//! Network models built from a powerio `BalancedNetwork`: the [`DcNetwork`] B-theta model
//! and the [`AcNetwork`] pi-model admittance form. Both normalize exactly once through
//! `BalancedNetwork::to_normalized` + `IndexedNetwork` (per unit, radians, filtered, densely
//! reindexed, reference inferred), then build a `powerio-prob` problem instance
//! (`DcOpfInstance` / `AcOpfInstance`) as the shared owner of case interpretation —
//! per unit generator PQ bounds, nodal withdrawal, branch phase terms, reference coverage —
//! then layer on the solver preparation each formulation needs.
//!
//! Two pieces of solver policy stay here as passes:
//! [`flatten_gen_costs`] rewrites every generator's cost to a plain quadratic before
//! the instance is built (the piecewise fit, the missing cost rule, and the leading
//! artifact strip), and [`normalize_angle_bounds`] applies Tellegen's tighter defaults.
//! The DC model consumes the complete `DcOpfInstance`, including affine phase terms,
//! source rows, and synthesized limits. The AC model consumes the instance's complete
//! pi model, including terminal charging and lowered three-winding transformers.
//!
//! The two formulations split into [`mod@dc`] and [`mod@ac`].

use std::collections::HashSet;

use powerio::{BalancedNetwork, GenCost, IndexedNetwork, NormalizeOptions};

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
/// `BalancedNetwork` pre-pass: the piecewise least squares fit, the leading rounding
/// artifact strip, and the rule treating a missing cost as free all run here, so the
/// powerio-prob builders — whose `GenCost::quadratic()` /
/// `quadratic_with_constant()` return `None` for piecewise, cubic-and-higher, or
/// absent rows — accept every generator and read back exactly these coefficients.
/// Run on the normalized network (per unit) so the fit sees the same points Tellegen
/// fit before this migration.
pub(super) fn flatten_gen_costs(net: &mut BalancedNetwork) -> Result<GenCostColumns, String> {
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

/// Exact 60 degree angle-difference pad used by every Tellegen formulation.
pub(super) const DEFAULT_ANGLE_BOUND_PAD: f64 = std::f64::consts::PI / 3.0;

/// Default angle-difference bounds (radians in, radians out). A `>= pi/2` half-window
/// (the MATPOWER "unconstrained" +-360 degree default, or the zero/zero "unset" case)
/// collapses to the documented +-60 degree MATPOWER/PowerModels convention. Shared by
/// the DC OPF (which carries these) and the AC model (the AC OPF angle-difference limits
/// and the conic angle constraints).
pub(super) fn normalize_angle_bounds(mut amin: f64, mut amax: f64) -> (f64, f64) {
    if amin <= -std::f64::consts::FRAC_PI_2 {
        amin = -DEFAULT_ANGLE_BOUND_PAD;
    }
    if amax >= std::f64::consts::FRAC_PI_2 {
        amax = DEFAULT_ANGLE_BOUND_PAD;
    }
    if amin == 0.0 && amax == 0.0 {
        return (-DEFAULT_ANGLE_BOUND_PAD, DEFAULT_ANGLE_BOUND_PAD);
    }
    (amin, amax)
}

/// Row provenance needed by Tellegen's solver models. Positions are in the
/// star-lowered [`IndexedNetwork`] view; a `None` row is a synthetic 3-winding
/// star element with no row in the source network.
#[derive(Clone, Debug)]
pub(super) struct ModelSourceRows {
    pub(super) buses: Vec<Option<usize>>,
    pub(super) branches: Vec<Option<usize>>,
    pub(super) generators: Vec<Option<usize>>,
    /// Normalized active transformer position -> caller transformer row. Used
    /// to preserve raw lowering ordinals when normalization drops an earlier
    /// transformer whose winding referenced an isolated bus.
    pub(super) transformers_3w: Vec<Option<usize>>,
}

/// A computation-ready network and the map back to the network supplied by the
/// caller. PowerIO's normalized marker is authoritative: a normalized network
/// is already per unit/radians and must not be scaled a second time.
pub(super) struct ModelInput {
    pub(super) network: BalancedNetwork,
    pub(super) source_rows: ModelSourceRows,
}

/// Normalize a raw network exactly once, or clone an already-normalized network,
/// while preserving source-row identity across PowerIO's 3-winding star lowering.
pub(super) fn normalize_for_model(source: &BalancedNetwork) -> Result<ModelInput, String> {
    source.validate().map_err(|error| error.to_string())?;
    validate_canonical_identity(source)?;
    reject_unsupported_active_elements(source)?;
    let (network, source_rows) = if source.is_normalized() {
        source.check_base_mva().map_err(|error| error.to_string())?;
        let network = source.clone();
        let (n_buses, n_branches) = {
            let view = IndexedNetwork::new(&network);
            (view.n(), view.branches().len())
        };
        let mut buses = (0..network.buses.len()).map(Some).collect::<Vec<_>>();
        let mut branches = (0..network.branches.len()).map(Some).collect::<Vec<_>>();
        buses.resize(n_buses, None);
        branches.resize(n_branches, None);
        let generators = (0..network.generators.len()).map(Some).collect();
        let transformers_3w = network
            .transformers_3w
            .iter()
            .enumerate()
            .filter(|(_, transformer)| transformer.in_service)
            .map(|(row, _)| Some(row))
            .collect();
        (
            network,
            ModelSourceRows {
                buses,
                branches,
                generators,
                transformers_3w,
            },
        )
    } else {
        let (normalized, rows) = source
            .to_normalized_with_source_rows(&NormalizeOptions::default())
            .map_err(|error| error.to_string())?;
        (
            normalized.network,
            ModelSourceRows {
                buses: rows.buses,
                branches: rows.branches,
                generators: rows.generators,
                transformers_3w: rows.transformers_3w,
            },
        )
    };

    Ok(ModelInput {
        network,
        source_rows,
    })
}

/// Require edit-axis UIDs to be unique and distinguishable from numeric IDs.
pub(crate) fn validate_canonical_identity(network: &BalancedNetwork) -> Result<(), String> {
    validate_unique_uids("bus", network.buses.iter().map(|bus| bus.uid.as_deref()))?;
    validate_unique_uids(
        "branch",
        network.branches.iter().map(|branch| branch.uid.as_deref()),
    )
}

pub(super) fn validate_unique_uids<'a>(
    family: &str,
    uids: impl IntoIterator<Item = Option<&'a str>>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for uid in uids.into_iter().flatten() {
        let digits = uid
            .strip_prefix('+')
            .or_else(|| uid.strip_prefix('-'))
            .unwrap_or(uid);
        if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!(
                "{family} uid \"{uid}\" is ambiguous with a numeric element id"
            ));
        }
        if !seen.insert(uid) {
            return Err(format!("duplicate {family} uid \"{uid}\""));
        }
    }
    Ok(())
}

/// Tellegen does not yet model these active element families. Refuse them at
/// model construction instead of returning a feasible-looking solve that omitted
/// their topology or injections. Inactive/open records remain lossless metadata.
fn reject_unsupported_active_elements(network: &BalancedNetwork) -> Result<(), String> {
    let closed_switches = network
        .switches
        .iter()
        .filter(|switch| switch.closed)
        .count();
    let active_storage = network
        .storage
        .iter()
        .filter(|storage| storage.in_service)
        .count();
    let active_hvdc = network.hvdc.iter().filter(|link| link.in_service).count();
    let mut unsupported = Vec::new();
    if closed_switches > 0 {
        unsupported.push(format!("{closed_switches} closed switch(es)"));
    }
    if active_storage > 0 {
        unsupported.push(format!("{active_storage} in-service storage unit(s)"));
    }
    if active_hvdc > 0 {
        unsupported.push(format!("{active_hvdc} in-service HVDC link(s)"));
    }
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "network contains active elements this solver does not support: {}",
            unsupported.join(", ")
        ))
    }
}

/// Compose a problem instance's source rows (indices into the lowered normalized
/// view) with normalization provenance (indices into the caller's source tables).
pub(super) fn project_source_rows(
    view_rows: &[usize],
    provenance: &[Option<usize>],
    family: &str,
) -> Result<Vec<Option<usize>>, String> {
    view_rows
        .iter()
        .map(|&row| {
            provenance.get(row).copied().ok_or_else(|| {
                format!(
                    "{family} source-row projection {row} outside provenance length {}",
                    provenance.len()
                )
            })
        })
        .collect()
}

/// Stable one-based element ids for selected lowered-view positions. Real rows
/// retain their source position. Synthetic rows are allocated after the complete
/// source table, and their ordinal is computed before instance filtering so a
/// skipped zero-impedance winding cannot renumber the following winding.
pub(super) fn ids_for_view_rows(
    view_rows: &[usize],
    provenance: &[Option<usize>],
    source_len: usize,
    family: &str,
) -> Result<Vec<usize>, String> {
    let mut synthetic = source_len
        .checked_add(1)
        .ok_or_else(|| format!("{family} synthetic id space exhausted"))?;
    let all_ids = provenance
        .iter()
        .map(|row| match row {
            Some(row) => {
                if *row >= source_len {
                    return Err(format!(
                        "{family} source row {row} outside source length {source_len}"
                    ));
                }
                Ok(row + 1)
            }
            None => {
                let id = synthetic;
                synthetic = synthetic
                    .checked_add(1)
                    .ok_or_else(|| format!("{family} synthetic id space exhausted"))?;
                Ok(id)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    view_rows
        .iter()
        .map(|&row| {
            all_ids.get(row).copied().ok_or_else(|| {
                format!(
                    "{family} view row {row} outside provenance length {}",
                    provenance.len()
                )
            })
        })
        .collect()
}

fn active_transformer_ordinals(source: &BalancedNetwork) -> Vec<Option<usize>> {
    let mut next = 0usize;
    source
        .transformers_3w
        .iter()
        .map(|transformer| {
            transformer.in_service.then(|| {
                let ordinal = next;
                next += 1;
                ordinal
            })
        })
        .collect()
}

fn source_transformer_ordinal(
    normalized_transformer: usize,
    transformer_source_rows: &[Option<usize>],
    source_ordinals: &[Option<usize>],
    family: &str,
) -> Result<usize, String> {
    let source_row = transformer_source_rows
        .get(normalized_transformer)
        .copied()
        .flatten()
        .ok_or_else(|| {
            format!(
                "{family} synthetic row references unknown normalized transformer {normalized_transformer}"
            )
        })?;
    source_ordinals
        .get(source_row)
        .copied()
        .flatten()
        .ok_or_else(|| {
            format!("{family} synthetic row references inactive source transformer {source_row}")
        })
}

/// Stable bus ids for a lowered normalized view. Source rows recover the exact
/// id carried by the caller. Each synthetic star uses its transformer's ordinal
/// among all active source transformers, matching PowerIO's lowering of the
/// canonical network even when normalization dropped an earlier transformer.
pub(super) fn bus_ids_for_source_rows(
    source_rows: &[Option<usize>],
    transformer_source_rows: &[Option<usize>],
    source: &BalancedNetwork,
) -> Result<Vec<usize>, String> {
    let synthetic_base = source
        .buses
        .iter()
        .map(|bus| bus.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "bus synthetic id space exhausted".to_owned())?;
    let source_ordinals = active_transformer_ordinals(source);
    let mut normalized_transformer = 0usize;
    source_rows
        .iter()
        .map(|row| match row {
            Some(row) => source.buses.get(*row).map(|bus| bus.id.0).ok_or_else(|| {
                format!(
                    "bus source row {row} outside source length {}",
                    source.buses.len()
                )
            }),
            None => {
                let ordinal = source_transformer_ordinal(
                    normalized_transformer,
                    transformer_source_rows,
                    &source_ordinals,
                    "bus",
                )?;
                normalized_transformer += 1;
                synthetic_base
                    .checked_add(ordinal)
                    .ok_or_else(|| "bus synthetic id space exhausted".to_owned())
            }
        })
        .collect()
}

/// Stable one-based branch ids for a lowered normalized view. Synthetic winding
/// ids retain the source transformer's raw active ordinal and winding ordinal,
/// so filtering an earlier transformer or a zero-impedance winding cannot
/// renumber later display/solution rows.
pub(super) fn branch_ids_for_view_rows(
    view_rows: &[usize],
    provenance: &[Option<usize>],
    transformer_source_rows: &[Option<usize>],
    source: &BalancedNetwork,
) -> Result<Vec<usize>, String> {
    let synthetic_base = source
        .branches
        .len()
        .checked_add(1)
        .ok_or_else(|| "branch synthetic id space exhausted".to_owned())?;
    let source_ordinals = active_transformer_ordinals(source);
    let mut synthetic_branch = 0usize;
    let all_ids = provenance
        .iter()
        .map(|row| match row {
            Some(row) => {
                if *row >= source.branches.len() {
                    return Err(format!(
                        "branch source row {row} outside source length {}",
                        source.branches.len()
                    ));
                }
                Ok(row + 1)
            }
            None => {
                let normalized_transformer = synthetic_branch / 3;
                let winding = synthetic_branch % 3;
                synthetic_branch += 1;
                let source_ordinal = source_transformer_ordinal(
                    normalized_transformer,
                    transformer_source_rows,
                    &source_ordinals,
                    "branch",
                )?;
                let offset = source_ordinal
                    .checked_mul(3)
                    .and_then(|offset| offset.checked_add(winding))
                    .ok_or_else(|| "branch synthetic id space exhausted".to_owned())?;
                synthetic_base
                    .checked_add(offset)
                    .ok_or_else(|| "branch synthetic id space exhausted".to_owned())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    view_rows
        .iter()
        .map(|&row| {
            all_ids.get(row).copied().ok_or_else(|| {
                format!(
                    "branch view row {row} outside provenance length {}",
                    provenance.len()
                )
            })
        })
        .collect()
}

pub(super) fn uids_for_source_rows<T>(
    source_rows: &[Option<usize>],
    source: &[T],
    uid: impl Fn(&T) -> &Option<String>,
    family: &str,
) -> Result<Vec<Option<String>>, String> {
    source_rows
        .iter()
        .map(|row| match row {
            Some(row) => source
                .get(*row)
                .map(|element| uid(element).clone())
                .ok_or_else(|| {
                    format!(
                        "{family} source row {row} outside source length {}",
                        source.len()
                    )
                }),
            None => Ok(None),
        })
        .collect()
}

/// Dense sizes and source metadata recovered for the AC problem instance.
#[cfg(feature = "sensitivity")]
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

/// Resolve the AC instance's dense columns back through normalization and
/// 3-winding lowering to the caller's source tables.
#[cfg(feature = "sensitivity")]
pub(super) fn reconstruct_ids(
    raw: &BalancedNetwork,
    bus_ids: &[powerio::BusId],
    branch_view_rows: &[usize],
    generator_view_rows: &[usize],
    source_rows: &ModelSourceRows,
) -> Result<Ids, String> {
    let n = bus_ids.len();
    if source_rows.buses.len() != n {
        return Err(format!(
            "bus provenance length {} != problem bus count {n}",
            source_rows.buses.len()
        ));
    }
    let m = branch_view_rows.len();
    let k = generator_view_rows.len();
    if k == 0 {
        return Err("network has no in-service generators".into());
    }
    let bus_ids = bus_ids_for_source_rows(&source_rows.buses, &source_rows.transformers_3w, raw)?;
    let bus_uids = uids_for_source_rows(&source_rows.buses, &raw.buses, |bus| &bus.uid, "bus")?;
    let branch_source_rows =
        project_source_rows(branch_view_rows, &source_rows.branches, "branch")?;
    let branch_ids = branch_ids_for_view_rows(
        branch_view_rows,
        &source_rows.branches,
        &source_rows.transformers_3w,
        raw,
    )?;
    let branch_uids = uids_for_source_rows(
        &branch_source_rows,
        &raw.branches,
        |branch| &branch.uid,
        "branch",
    )?;
    let gen_ids = ids_for_view_rows(
        generator_view_rows,
        &source_rows.generators,
        raw.generators.len(),
        "generator",
    )?;

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

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    fn case3() -> BalancedNetwork {
        powerio::parse_str(crate::model::CASE3, "matpower")
            .expect("parse case3")
            .network
    }

    fn assert_rejected_by_every_model(net: &BalancedNetwork, family: &str) {
        let dc = crate::model::DcNetwork::from_network(net)
            .err()
            .expect("active unsupported element must reject DC construction");
        assert!(dc.contains(family), "unexpected DC error: {dc}");

        #[cfg(feature = "sensitivity")]
        {
            let ac = crate::model::AcNetwork::from_network(net)
                .expect_err("active unsupported element must reject AC construction");
            assert!(ac.contains(family), "unexpected AC error: {ac}");
        }
    }

    #[test]
    fn active_unmodeled_element_families_fail_closed() {
        let mut switched = case3();
        switched.switches.push(powerio::Switch::new(
            powerio::BusId(1),
            powerio::BusId(2),
            true,
        ));
        assert_rejected_by_every_model(&switched, "closed switch");

        let mut storage = case3();
        storage
            .storage
            .push(powerio::Storage::new(powerio::BusId(2)));
        assert_rejected_by_every_model(&storage, "in-service storage");

        let mut hvdc = case3();
        hvdc.hvdc
            .push(powerio::Hvdc::new(powerio::BusId(1), powerio::BusId(2)));
        assert_rejected_by_every_model(&hvdc, "in-service HVDC");
    }

    #[test]
    fn inactive_unmodeled_elements_do_not_reject() {
        let mut net = case3();
        net.switches.push(powerio::Switch::new(
            powerio::BusId(1),
            powerio::BusId(2),
            false,
        ));
        let mut storage = powerio::Storage::new(powerio::BusId(2));
        storage.in_service = false;
        net.storage.push(storage);
        let mut hvdc = powerio::Hvdc::new(powerio::BusId(1), powerio::BusId(2));
        hvdc.in_service = false;
        net.hvdc.push(hvdc);

        crate::model::DcNetwork::from_network(&net).expect("inactive metadata is safe for DC");
        #[cfg(feature = "sensitivity")]
        crate::model::AcNetwork::from_network(&net).expect("inactive metadata is safe for AC");
    }

    #[test]
    fn programmatic_structural_errors_reject_before_indexing() {
        let mut duplicate = case3();
        duplicate.buses[1].id = duplicate.buses[0].id;
        let error = crate::model::DcNetwork::from_network(&duplicate)
            .err()
            .expect("duplicate ids must reject");
        assert!(error.to_lowercase().contains("duplicate"), "{error}");

        let mut dangling = case3();
        dangling.branches[0].to = powerio::BusId(999_999);
        let error = crate::model::DcNetwork::from_network(&dangling)
            .err()
            .expect("dangling branch must reject");
        assert!(
            error.contains("999999") || error.to_lowercase().contains("unknown"),
            "{error}"
        );
    }
}
