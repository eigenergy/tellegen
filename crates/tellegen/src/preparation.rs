//! Explicit, reproducible model approximations over preserved source data.
use powerio::{BalancedNetwork, GenCost};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CostPreparation {
    #[default]
    Exact,
    ConvexQuadraticFit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CostApproximation {
    pub generator: usize,
    pub bus: usize,
    pub source_cost: GenCost,
    pub model_cost: GenCost,
    pub rms_breakpoint_error: f64,
    pub max_breakpoint_error: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ModelDetails {
    pub method: CostPreparation,
    pub source: String,
    pub model: String,
    pub quantity: String,
    pub units: String,
    pub approximations: Vec<CostApproximation>,
}

/// Fit piecewise costs by least squares in per-unit power; retain all source rows.
/// A nonconvex or singular quadratic fit uses a least-squares line instead.
pub fn prepare_costs(
    source: &BalancedNetwork,
    method: CostPreparation,
) -> Result<(BalancedNetwork, Option<ModelDetails>), String> {
    if method == CostPreparation::Exact {
        return Ok((source.clone(), None));
    }
    let scale = if source.is_normalized() {
        1.0
    } else {
        source.check_base_mva().map_err(|e| e.to_string())?;
        source.base_mva()
    };
    let mut network = source.clone();
    let mut approximations = Vec::new();
    for (row, generator) in network.generators_mut().iter_mut().enumerate() {
        let Some(cost) = generator.cost.as_ref().filter(|cost| cost.model == 1) else {
            continue;
        };
        let mut normalized = cost.clone();
        for pair in normalized.coeffs.chunks_exact_mut(2) {
            pair[0] /= scale;
        }
        let (q, l, c) = piecewise_quadratic_fit(&normalized)?;
        let coeffs = vec![q / scale.powi(2), l / scale, c];
        if !coeffs.iter().all(|v| v.is_finite()) {
            return Err("cost fitting produced non-finite coefficients".into());
        }
        let fitted = GenCost::new(2, cost.startup, cost.shutdown, coeffs);
        let errors: Vec<f64> = cost
            .coeffs
            .chunks_exact(2)
            .map(|p| ((q * (p[0] / scale) + l) * (p[0] / scale) + c - p[1]).abs())
            .collect();
        approximations.push(CostApproximation {
            generator: row + 1,
            bus: generator.bus.0,
            source_cost: cost.clone(),
            model_cost: fitted.clone(),
            rms_breakpoint_error: (errors.iter().map(|x| x * x).sum::<f64>()
                / errors.len().max(1) as f64)
                .sqrt(),
            max_breakpoint_error: errors.into_iter().fold(0.0, f64::max),
        });
        generator.cost = Some(fitted);
    }
    let source_ir = crate::ir::serialize_module(&powerio::PioModule::new(
        powerio::PioValue::BalancedNetwork(source.clone()),
    ))?;
    let model_ir = crate::ir::serialize_module(&powerio::PioModule::new(
        powerio::PioValue::BalancedNetwork(network.clone()),
    ))?;
    let details = ModelDetails {
        method,
        source: format!("sha256:{:x}", Sha256::digest(source_ir.as_bytes())),
        model: format!("sha256:{:x}", Sha256::digest(model_ir.as_bytes())),
        quantity: "generator cost".into(),
        units: "objective units".into(),
        approximations,
    };
    Ok((network, Some(details)))
}

fn piecewise_quadratic_fit(cost: &GenCost) -> Result<(f64, f64, f64), String> {
    // `ncost` is deserialized straight from model JSON and never clamped upstream,
    // so `ncost * 2` must not be allowed to wrap past the coefficient length and
    // then size an allocation. Take the capacity from the data, not the count.
    if cost.ncost.checked_mul(2) != Some(cost.coeffs.len()) {
        return Err("piecewise gen costs must have paired breakpoints".into());
    }
    let mut points = Vec::with_capacity(cost.coeffs.len() / 2);
    for pair in cost.coeffs.chunks_exact(2) {
        let x = pair[0];
        let y = pair[1];
        if !x.is_finite() || !y.is_finite() {
            return Err("piecewise gen costs must be finite".into());
        }
        points.push((x, y));
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| (a.0 - b.0).abs() <= f64::EPSILON);

    match points.len() {
        0 => Ok((0.0, 0.0, 0.0)),
        1 => Ok((0.0, 0.0, points[0].1)),
        2 => Ok(linear_fit(&points)),
        _ => Ok(quadratic_fit(&points).unwrap_or_else(|| linear_fit(&points))),
    }
}

/// Least squares line over every breakpoint. The quadratic fit falls back here
/// when its system is singular or nonconvex (`q < 0`), so interior points must
/// still weigh in: an endpoints chord would misprice everything between them.
fn linear_fit(points: &[(f64, f64)]) -> (f64, f64, f64) {
    let n = points.len() as f64;
    let (mut sx, mut sxx, mut sy, mut sxy) = (0.0, 0.0, 0.0, 0.0);
    for &(x, y) in points {
        sx += x;
        sxx += x * x;
        sy += y;
        sxy += x * y;
    }
    let det = n * sxx - sx * sx;
    if det.abs() <= f64::EPSILON * n * sxx.max(1.0) {
        // All breakpoints at one output level: a flat cost at their mean.
        return (0.0, 0.0, sy / n);
    }
    let slope = (n * sxy - sx * sy) / det;
    let intercept = (sy - slope * sx) / n;
    (0.0, slope, intercept)
}

fn quadratic_fit(points: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    let mut s0 = 0.0;
    let mut s1 = 0.0;
    let mut s2 = 0.0;
    let mut s3 = 0.0;
    let mut s4 = 0.0;
    let mut t0 = 0.0;
    let mut t1 = 0.0;
    let mut t2 = 0.0;
    for &(x, y) in points {
        let x2 = x * x;
        s0 += 1.0;
        s1 += x;
        s2 += x2;
        s3 += x2 * x;
        s4 += x2 * x2;
        t0 += y;
        t1 += x * y;
        t2 += x2 * y;
    }
    let [q, l, c] = solve_3x3([[s4, s3, s2], [s3, s2, s1], [s2, s1, s0]], [t2, t1, t0])?;
    if q.is_finite() && l.is_finite() && c.is_finite() && q >= 0.0 {
        Some((q, l, c))
    } else {
        None
    }
}

fn solve_3x3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for i in 0..3 {
        let mut pivot = i;
        for r in (i + 1)..3 {
            if a[r][i].abs() > a[pivot][i].abs() {
                pivot = r;
            }
        }
        if a[pivot][i].abs() <= 1e-12 {
            return None;
        }
        if pivot != i {
            a.swap(i, pivot);
            b.swap(i, pivot);
        }
        let pivot_row = a[i];
        for r in (i + 1)..3 {
            let factor = a[r][i] / pivot_row[i];
            for (elem, p) in a[r].iter_mut().zip(pivot_row).skip(i) {
                *elem -= factor * p;
            }
            b[r] -= factor * b[i];
        }
    }

    let mut x = [0.0; 3];
    for i in (0..3).rev() {
        let mut rhs = b[i];
        for (c, value) in x.iter().enumerate().skip(i + 1) {
            rhs -= a[i][c] * value;
        }
        x[i] = rhs / a[i][i];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_in_mw_and_retains_source_costs_and_errors() {
        let mut source = crate::model::parse_matpower(crate::model::CASE3).unwrap();
        let points = vec![0.0, 7.0, 50.0, 82.0, 100.0, 207.0, 150.0, 382.0];
        source.generators_mut()[0].cost = Some(GenCost::new(1, 9.0, 3.0, points));
        let (model, details) = prepare_costs(&source, CostPreparation::ConvexQuadraticFit).unwrap();
        let cost = model.generators()[0].cost.as_ref().unwrap();
        for (actual, expected) in cost.coeffs.iter().zip([0.01, 1.0, 7.0]) {
            assert!((actual - expected).abs() < 1e-8);
        }
        assert_eq!(source.generators()[0].cost.as_ref().unwrap().model, 1);
        let details = details.unwrap();
        assert_eq!(details.approximations.len(), 1);
        let row = &details.approximations[0];
        assert_eq!(
            row.source_cost,
            *source.generators()[0].cost.as_ref().unwrap()
        );
        assert_eq!(row.model_cost, *cost);
        assert!(row.max_breakpoint_error < 1e-8);
        assert_eq!(row.model_cost.startup, 9.0);
        assert_ne!(details.source, details.model);
    }

    #[test]
    fn concave_fit_uses_all_breakpoints_for_linear_least_squares() {
        let cost = GenCost::new(1, 0.0, 0.0, vec![0.0, 0.0, 1.0, 3.0, 2.0, 4.0]);
        let (quadratic, linear, constant) = piecewise_quadratic_fit(&cost).unwrap();
        assert_eq!(quadratic, 0.0);
        assert!((linear - 2.0).abs() < 1e-10);
        assert!((constant - 1.0 / 3.0).abs() < 1e-10);
        let mut malformed = cost.clone();
        malformed.ncost = usize::MAX;
        assert!(piecewise_quadratic_fit(&malformed).is_err());
        malformed = cost;
        malformed.coeffs[1] = f64::NAN;
        assert!(piecewise_quadratic_fit(&malformed).is_err());
    }
}
