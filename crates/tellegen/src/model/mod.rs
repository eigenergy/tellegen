//! Network models built from a powerio `Network`: the [`DcNetwork`] B-theta model
//! and the [`AcNetwork`] pi-model admittance form. Both normalize through
//! `Network::to_normalized` + `IndexedNetwork` (per unit, radians, filtered, densely
//! reindexed, reference inferred), build a `powerio-prob` problem instance
//! (`DcOpfInstance` / `AcOpfInstance`) as the shared owner of case interpretation —
//! per-unit generator PQ bounds, nodal demand, reference coverage — then layer on the
//! solver-prep each formulation needs.
//!
//! Two pieces of solver policy the instance builders don't own stay here as passes:
//! [`flatten_gen_costs`] rewrites every generator's cost to a plain quadratic before
//! the instance is built (the piecewise fit / missing-cost / leading-artifact rules),
//! and [`normalize_angle_bounds`] runs per branch afterward. Branch susceptance
//! (DC) and pi-model admittance (AC) are computed from the `IndexedNetwork` directly:
//! neither `DcConvention` reproduces tellegen's `-x/(r^2+x^2)`, and the dense branch
//! arrays keep every source branch — including a literal zero-impedance record the
//! instance would skip — so `problem/` and `sens/` stay index-aligned.
//!
//! The two formulations split into [`mod@dc`] and [`mod@ac`]; the dense-reindex /
//! id-reconstruction step they share lives here in [`reconstruct_ids`].

use std::collections::HashSet;

use powerio::network::{BusType, GenCost, Network};
use powerio::IndexedNetwork;

#[cfg(feature = "sensitivity")]
mod ac;
#[cfg(test)]
mod cases;
mod dc;

#[cfg(feature = "sensitivity")]
pub use ac::AcNetwork;
pub use dc::DcNetwork;

#[cfg(all(test, feature = "conic"))]
pub(crate) use cases::parse_case3_ac;
#[cfg(all(test, feature = "sensitivity"))]
pub(crate) use cases::parse_case9_ac;
#[cfg(test)]
pub(crate) use cases::{parse_case3, CASE3};

/// Below this squared impedance a branch is treated as open (zero admittance) — the
/// near-zero-impedance guard, shared by the DC and AC models.
pub(super) const MIN_Z_SQUARED: f64 = 1e-10;

/// A leading gen-cost polynomial coefficient at or below this magnitude is treated as a
/// rounding artifact and stripped, so a curve meant to be linear is not read as quadratic
/// because its quadratic term came in as e.g. `1e-17` rather than exactly `0.0`. Real
/// (per unit) cost coefficients sit far above this. Shared by the DC and AC cost readers.
pub(super) const LEADING_COST_COEFF_TOL: f64 = 1e-12;

/// Quadratic, linear, and constant cost coefficients `(cq, cl, cc)` for one
/// generator. MATPOWER model 2 rows are read directly after `to_normalized`
/// rescales them to per unit. Model 1 rows are piecewise linear costs; the
/// solver objective is quadratic, so those points are projected onto a
/// nonnegative quadratic least squares fit.
pub(super) fn quadratic_cost_coeffs(cost: Option<&GenCost>) -> Result<(f64, f64, f64), String> {
    let Some(c) = cost else {
        return Ok((0.0, 0.0, 0.0));
    };
    match c.model {
        1 => piecewise_quadratic_fit(c),
        2 => polynomial_quadratic_coeffs(c),
        _ => Err("only gen-cost models 1 and 2 are supported".into()),
    }
}

/// The quadratic, linear, and constant generation-cost coefficients as three
/// parallel columns in generator order (`cq[i]`/`cl[i]`/`cc[i]` for generator `i`) —
/// the layout `DcNetwork`/`AcNetwork` store, returned by [`flatten_gen_costs`].
pub(super) type GenCostColumns = (Vec<f64>, Vec<f64>, Vec<f64>);

/// Rewrite every generator's cost to a plain quadratic `[cq, cl, cc]` (MATPOWER
/// model 2, three coefficients) via [`quadratic_cost_coeffs`], returning the three
/// coefficient columns `(cq, cl, cc)` in generator order — the layout both
/// `DcNetwork` and `AcNetwork` store. This is tellegen's cost policy applied as a
/// `Network` pre-pass: the piecewise least squares fit, the leading
/// rounding-artifact strip, and the missing-cost-is-free rule all run here, so the
/// powerio-prob builders — whose `GenCost::quadratic()` /
/// `quadratic_with_constant()` return `None` for piecewise, cubic-and-higher, or
/// absent rows — accept every generator and read back exactly these coefficients.
/// The [`DcOpfInstance`](powerio_prob::DcOpfInstance) carries no constant term, so
/// the DC caller takes `cc` from here. Run on the normalized network (per unit) so
/// the fit sees the same points tellegen fit before this migration.
pub(super) fn flatten_gen_costs(net: &mut Network) -> Result<GenCostColumns, String> {
    let g = net.generators.len();
    let (mut cq, mut cl, mut cc) = (
        Vec::with_capacity(g),
        Vec::with_capacity(g),
        Vec::with_capacity(g),
    );
    for gen in &mut net.generators {
        let (q, l, c) = quadratic_cost_coeffs(gen.cost.as_ref())?;
        cq.push(q);
        cl.push(l);
        cc.push(c);
        gen.cost = Some(GenCost::new(2, 0.0, 0.0, vec![q, l, c]));
    }
    Ok((cq, cl, cc))
}

fn polynomial_quadratic_coeffs(cost: &GenCost) -> Result<(f64, f64, f64), String> {
    let mut v = cost.coeffs.clone();
    while v.len() > 1 && v[0].abs() <= LEADING_COST_COEFF_TOL {
        v.remove(0);
    }
    match v.len() {
        0 => Ok((0.0, 0.0, 0.0)),
        1 => Ok((0.0, 0.0, v[0])),
        2 => Ok((0.0, v[0], v[1])),
        3 => Ok((v[0], v[1], v[2])),
        _ => Err("only constant, linear, and quadratic gen costs are supported".into()),
    }
}

fn piecewise_quadratic_fit(cost: &GenCost) -> Result<(f64, f64, f64), String> {
    if cost.coeffs.len() != cost.ncost * 2 {
        return Err("piecewise gen costs must have paired breakpoints".into());
    }
    let mut points = Vec::with_capacity(cost.ncost);
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

/// Default angle-difference bounds (radians in, radians out). A `>= pi/2` half-window
/// (the MATPOWER "unconstrained"
/// +-360 degree default, or the zero/zero "unset" case) collapses to the +-60 degree
/// MATPOWER/PowerModels convention. Shared by the DC OPF (which carries these) and the
/// AC model (the AC OPF angle-difference limits and the conic angle constraints).
pub(super) fn normalize_angle_bounds(mut amin: f64, mut amax: f64) -> (f64, f64) {
    let pad = 60.0_f64.to_radians();
    if amin <= -std::f64::consts::FRAC_PI_2 {
        amin = -pad;
    }
    if amax >= std::f64::consts::FRAC_PI_2 {
        amax = pad;
    }
    if amin == 0.0 && amax == 0.0 {
        return (-pad, pad);
    }
    (amin, amax)
}

/// Dense sizes and the dense-index -> source-id maps recovered from a normalized
/// network. `to_normalized` keeps non-isolated buses (and in-service, attached
/// branches/generators) in source order and reassigns dense ids in that order, so
/// the k-th surviving raw element is dense index k. `bus_uids`/`branch_uids` carry
/// each surviving element's powerio row uid (`None` when the source network was
/// never stamped), aligned with `bus_ids`/`branch_ids`.
pub(super) struct Ids {
    n: usize,
    m: usize,
    k: usize,
    bus_ids: Vec<usize>,
    branch_ids: Vec<usize>,
    gen_ids: Vec<usize>,
    bus_uids: Vec<Option<String>>,
    branch_uids: Vec<Option<String>>,
}

/// Reconstruct the dense sizes and source-id maps for `view`, the shared first step
/// of both [`DcNetwork::from_network`] and [`AcNetwork::from_network`]. Errors if a
/// reconstructed id list does not match the normalized count (the dense-reindex
/// assumption broke) or the network has no in-service generators.
pub(super) fn reconstruct_ids(raw: &Network, view: &IndexedNetwork) -> Result<Ids, String> {
    let n = view.n();
    let surviving_buses: Vec<&powerio::network::Bus> = raw
        .buses
        .iter()
        .filter(|b| b.kind != BusType::Isolated)
        .collect();
    let bus_ids: Vec<usize> = surviving_buses.iter().map(|b| b.id.0).collect();
    let bus_uids: Vec<Option<String>> = surviving_buses.iter().map(|b| b.uid.clone()).collect();
    if bus_ids.len() != n {
        return Err(format!(
            "bus id reconstruction mismatch: {} non-isolated raw buses vs {} normalized",
            bus_ids.len(),
            n
        ));
    }
    let active: HashSet<usize> = bus_ids.iter().copied().collect();

    let m = view.branches().len();
    let surviving_branches: Vec<(usize, &powerio::network::Branch)> = raw
        .branches
        .iter()
        .enumerate()
        .filter(|(_, br)| br.in_service && active.contains(&br.from.0) && active.contains(&br.to.0))
        .collect();
    let branch_ids: Vec<usize> = surviving_branches.iter().map(|(i, _)| i + 1).collect();
    let branch_uids: Vec<Option<String>> = surviving_branches
        .iter()
        .map(|(_, br)| br.uid.clone())
        .collect();
    if branch_ids.len() != m {
        return Err(format!(
            "branch id reconstruction mismatch: {} active raw branches vs {} normalized",
            branch_ids.len(),
            m
        ));
    }

    let k = view.generators().len();
    if k == 0 {
        return Err("network has no in-service generators".into());
    }
    let gen_ids: Vec<usize> = raw
        .generators
        .iter()
        .enumerate()
        .filter(|(_, g)| g.in_service && active.contains(&g.bus.0))
        .map(|(i, _)| i + 1)
        .collect();
    if gen_ids.len() != k {
        return Err(format!(
            "generator id reconstruction mismatch: {} active raw generators vs {} normalized",
            gen_ids.len(),
            k
        ));
    }

    Ok(Ids {
        n,
        m,
        k,
        bus_ids,
        branch_ids,
        gen_ids,
        bus_uids,
        branch_uids,
    })
}

#[cfg(test)]
mod cost_fit_tests {
    use super::*;

    fn piecewise(points: &[(f64, f64)]) -> GenCost {
        GenCost::new(
            1,
            0.0,
            0.0,
            points.iter().flat_map(|&(x, y)| [x, y]).collect(),
        )
    }

    #[test]
    fn nonconvex_points_fall_back_to_a_least_squares_line() {
        // Concave points reject the quadratic (q < 0). The line must weigh the
        // interior breakpoint: the least squares slope over (0,0),(10,100),(200,200)
        // is 60000/76200, not the endpoints chord slope of 1.
        let (q, l, _) = quadratic_cost_coeffs(Some(&piecewise(&[
            (0.0, 0.0),
            (10.0, 100.0),
            (200.0, 200.0),
        ])))
        .unwrap();
        assert_eq!(q, 0.0);
        let expected = 60000.0 / 76200.0;
        assert!((l - expected).abs() < 1e-9, "expected {expected}, got {l}");
    }

    #[test]
    fn exact_quadratic_points_recover_the_curve() {
        // y = 2x^2 + 3x + 1 at x = 0, 1, 2 solves the normal equations exactly.
        let (q, l, c) =
            quadratic_cost_coeffs(Some(&piecewise(&[(0.0, 1.0), (1.0, 6.0), (2.0, 15.0)])))
                .unwrap();
        assert!((q - 2.0).abs() < 1e-9, "q {q}");
        assert!((l - 3.0).abs() < 1e-9, "l {l}");
        assert!((c - 1.0).abs() < 1e-9, "c {c}");
    }
}
