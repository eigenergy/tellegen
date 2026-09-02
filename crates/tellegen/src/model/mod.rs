//! Private solver workspaces for PowerIO problem instances. The typed instance
//! paths consume PowerIO's dense preparation, including its identities and row
//! maps. Legacy callers that pass a raw [`BalancedNetwork`] still normalize it
//! once before constructing a typed instance; only that compatibility path
//! reconstructs provenance for Tellegen's older numeric solve response.
//!
//! PowerIO compiles the declared generator cost into quadratic columns or exact
//! convex piecewise linear breakpoints. Tellegen keeps that distinction in its
//! private workspace. [`normalize_angle_bounds`] is the remaining formulation
//! pass: it applies Tellegen's tighter default angle window.
//! The DC model consumes the complete `DcOpfInstance`, including affine phase terms,
//! source rows, and synthesized limits. The AC model consumes the instance's complete
//! pi model, including terminal charging and lowered three-winding transformers.
//!
//! The two formulations split into [`mod@dc`] and [`mod@ac`].

use std::collections::HashSet;

use powerio::{BalancedNetwork, BusType};
use powerio_matrix::{AnalysisBranchSource, PiecewiseLinearCost};
use powerio_tx::{IndexedNetwork, NormalizeOptions};

#[cfg(feature = "sensitivity")]
mod ac;
#[cfg(test)]
mod cases;
mod dc;

#[cfg(feature = "sensitivity")]
pub(crate) use ac::AcNetwork;
pub(crate) use dc::DcNetwork;

#[cfg(all(test, feature = "conic"))]
pub(crate) use cases::parse_case3_ac;
#[cfg(all(test, feature = "sensitivity"))]
pub(crate) use cases::parse_case9_ac;
#[cfg(test)]
pub(crate) use cases::{parse_case3, CASE3};

/// One convex piecewise linear objective term as the line equations used by
/// the solver epigraph. PowerIO has already validated the breakpoint order and
/// convexity; this private form precomputes the slopes and intercepts once.
#[derive(Clone, Debug)]
pub(crate) struct PiecewiseCost {
    pub(crate) slopes: Vec<f64>,
    pub(crate) intercepts: Vec<f64>,
}

impl PiecewiseCost {
    pub(crate) fn from_prepared(cost: PiecewiseLinearCost) -> Self {
        debug_assert_eq!(cost.power.len(), cost.value.len());
        debug_assert!(cost.power.len() >= 2);
        let mut slopes = Vec::with_capacity(cost.power.len() - 1);
        let mut intercepts = Vec::with_capacity(cost.power.len() - 1);
        for segment in 0..cost.power.len() - 1 {
            let slope = (cost.value[segment + 1] - cost.value[segment])
                / (cost.power[segment + 1] - cost.power[segment]);
            slopes.push(slope);
            intercepts.push(cost.value[segment] - slope * cost.power[segment]);
        }
        Self { slopes, intercepts }
    }

    pub(crate) fn evaluate(&self, power: f64) -> f64 {
        self.slopes
            .iter()
            .zip(&self.intercepts)
            .map(|(&slope, &intercept)| slope * power + intercept)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    pub(crate) fn segment_count(&self) -> usize {
        self.slopes.len()
    }

    pub(crate) fn maximum_slope(&self) -> f64 {
        self.slopes.last().copied().unwrap_or(0.0)
    }
}

/// Parse in-memory MATPOWER text into the balanced network through the PowerIO
/// module route: an in-memory `Source`, the automatic parse to
/// `PioModule<PioValue>`, and typed narrowing. The module records are
/// dropped here because these callers want only the value; a caller that
/// needs diagnostics, history, or same format writing keeps the module.
#[cfg_attr(not(test), allow(dead_code))] // the engine's tests and fixtures parse in memory
pub(crate) fn parse_matpower(text: &str) -> Result<powerio::BalancedNetwork, String> {
    let source = powerio::Source::from_memory("case.m", text.as_bytes().to_vec())
        .map_err(|e| e.to_string())?;
    let module = powerio::parse(source, None).map_err(|e| e.to_string())?;
    Ok(crate::ir::balanced_module(module)?.into_value())
}

/// Source branch row for each analysis branch column: `None` for a lowered
/// three winding transformer winding, which has no row in the branch table.
pub(super) fn branch_source_rows(sources: &[AnalysisBranchSource]) -> Vec<Option<usize>> {
    sources
        .iter()
        .map(|source| match source {
            AnalysisBranchSource::Branch { row } => Some(*row),
            _ => None,
        })
        .collect()
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
    // The two clamps above move each end independently, so a window lying wholly
    // on one side of zero and reaching past pi/2 comes back inverted: (-180, -70)
    // degrees collapses to (-60, -70), an empty interval that makes a solvable
    // case infeasible. powerio's `clamp_angle_bounds` repairs this the same way.
    if amin > amax {
        return (-DEFAULT_ANGLE_BOUND_PAD, DEFAULT_ANGLE_BOUND_PAD);
    }
    (amin, amax)
}

/// Row provenance for the legacy raw network construction path. Positions are
/// in the star lowered [`IndexedNetwork`] view; a `None` row is a synthetic
/// three winding star element with no row in the source network. Typed PowerIO
/// instances use the preparation's row maps instead.
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
        reject_unfiltered_normalized_elements(source)?;
        let network = source.clone();
        let (n_buses, n_branches) = {
            let view = IndexedNetwork::new(&network);
            (view.n(), view.branches().len())
        };
        let mut buses = (0..network.buses().len()).map(Some).collect::<Vec<_>>();
        let mut branches = (0..network.branches().len()).map(Some).collect::<Vec<_>>();
        buses.resize(n_buses, None);
        branches.resize(n_branches, None);
        let generators = (0..network.generators().len()).map(Some).collect();
        let transformers_3w = network
            .transformers_3w()
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
/// PowerIO gives a bus with no source identity its bus number as the uid, so
/// a numeric bus uid is accepted when it is the bus's own number: both key
/// spellings then name the same bus.
pub(crate) fn validate_canonical_identity(network: &BalancedNetwork) -> Result<(), String> {
    validate_unique_uids(
        "bus",
        network
            .buses()
            .iter()
            .map(|bus| (bus.uid.as_deref(), Some(bus.id.0))),
    )?;
    validate_unique_uids(
        "branch",
        network
            .branches()
            .iter()
            .map(|branch| (branch.uid.as_deref(), None)),
    )
}

/// Each item is a record's uid and, for a family addressed by number, the
/// record's own numeric id.
pub(super) fn validate_unique_uids<'a>(
    family: &str,
    uids: impl IntoIterator<Item = (Option<&'a str>, Option<usize>)>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for (uid, own_id) in uids {
        let Some(uid) = uid else {
            continue;
        };
        let digits = uid
            .strip_prefix('+')
            .or_else(|| uid.strip_prefix('-'))
            .unwrap_or(uid);
        let numeric = !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit());
        let own_number = own_id.is_some_and(|id| uid == id.to_string());
        if numeric && !own_number {
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
/// Guard the already-normalized fast path. `is_normalized()` reads a self-declared
/// `source_format` marker that any hand-written model JSON can set, but the element
/// filtering the marker claims lives in the normalization pass this path skips, and
/// `IndexedNetwork` sums every load and shunt with no in-service filter. Taking the
/// marker at face value would serve an out-of-service load and price it into every
/// LMP while reporting `optimal`. Fail closed instead: a network that really is
/// normalized carries none of these.
fn reject_unfiltered_normalized_elements(network: &BalancedNetwork) -> Result<(), String> {
    let idle_loads = network
        .loads()
        .iter()
        .filter(|load| !load.in_service)
        .count();
    let idle_shunts = network
        .shunts()
        .iter()
        .filter(|shunt| !shunt.in_service)
        .count();
    let isolated_buses = network
        .buses()
        .iter()
        .filter(|bus| bus.kind == BusType::Isolated)
        .count();
    let mut carried = Vec::new();
    if idle_loads > 0 {
        carried.push(format!("{idle_loads} out-of-service load(s)"));
    }
    if idle_shunts > 0 {
        carried.push(format!("{idle_shunts} out-of-service shunt(s)"));
    }
    if isolated_buses > 0 {
        carried.push(format!("{isolated_buses} isolated bus(es)"));
    }
    if carried.is_empty() {
        return Ok(());
    }
    Err(format!(
        "network declares itself normalized but still carries {}",
        carried.join(", ")
    ))
}

fn reject_unsupported_active_elements(network: &BalancedNetwork) -> Result<(), String> {
    let closed_switches = network
        .switches()
        .iter()
        .filter(|switch| switch.closed)
        .count();
    let active_storage = network
        .storage()
        .iter()
        .filter(|storage| storage.in_service)
        .count();
    let active_hvdc = network.hvdc().iter().filter(|link| link.in_service).count();
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
        .transformers_3w()
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
        .buses()
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
            Some(row) => source.buses().get(*row).map(|bus| bus.id.0).ok_or_else(|| {
                format!(
                    "bus source row {row} outside source length {}",
                    source.buses().len()
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
        .branches()
        .len()
        .checked_add(1)
        .ok_or_else(|| "branch synthetic id space exhausted".to_owned())?;
    let source_ordinals = active_transformer_ordinals(source);
    let mut synthetic_branch = 0usize;
    let all_ids = provenance
        .iter()
        .map(|row| match row {
            Some(row) => {
                if *row >= source.branches().len() {
                    return Err(format!(
                        "branch source row {row} outside source length {}",
                        source.branches().len()
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
    #[cfg(feature = "conic")]
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
    let bus_uids = uids_for_source_rows(&source_rows.buses, raw.buses(), |bus| &bus.uid, "bus")?;
    #[cfg(feature = "conic")]
    let branch_source_rows =
        project_source_rows(branch_view_rows, &source_rows.branches, "branch")?;
    let branch_ids = branch_ids_for_view_rows(
        branch_view_rows,
        &source_rows.branches,
        &source_rows.transformers_3w,
        raw,
    )?;
    #[cfg(feature = "conic")]
    let branch_uids = uids_for_source_rows(
        &branch_source_rows,
        raw.branches(),
        |branch| &branch.uid,
        "branch",
    )?;
    let gen_ids = ids_for_view_rows(
        generator_view_rows,
        &source_rows.generators,
        raw.generators().len(),
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
        #[cfg(feature = "conic")]
        branch_uids,
    })
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    fn case3() -> BalancedNetwork {
        crate::model::parse_matpower(crate::model::CASE3).expect("parse case3")
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
        switched.switches_mut().push(powerio::Switch::new(
            powerio::BusId(1),
            powerio::BusId(2),
            true,
        ));
        assert_rejected_by_every_model(&switched, "closed switch");

        let mut storage = case3();
        storage
            .storage_mut()
            .push(powerio::Storage::new(powerio::BusId(2)));
        assert_rejected_by_every_model(&storage, "in-service storage");

        let mut hvdc = case3();
        hvdc.hvdc_mut()
            .push(powerio::Hvdc::new(powerio::BusId(1), powerio::BusId(2)));
        assert_rejected_by_every_model(&hvdc, "in-service HVDC");
    }

    #[test]
    fn inactive_unmodeled_elements_do_not_reject() {
        let mut net = case3();
        net.switches_mut().push(powerio::Switch::new(
            powerio::BusId(1),
            powerio::BusId(2),
            false,
        ));
        let mut storage = powerio::Storage::new(powerio::BusId(2));
        storage.in_service = false;
        net.storage_mut().push(storage);
        let mut hvdc = powerio::Hvdc::new(powerio::BusId(1), powerio::BusId(2));
        hvdc.in_service = false;
        net.hvdc_mut().push(hvdc);

        crate::model::DcNetwork::from_network(&net).expect("inactive metadata is safe for DC");
        #[cfg(feature = "sensitivity")]
        crate::model::AcNetwork::from_network(&net).expect("inactive metadata is safe for AC");
    }

    #[test]
    fn programmatic_structural_errors_reject_before_indexing() {
        let mut duplicate = case3();
        duplicate.buses_mut()[1].id = duplicate.buses()[0].id;
        let error = crate::model::DcNetwork::from_network(&duplicate)
            .err()
            .expect("duplicate ids must reject");
        assert!(error.to_lowercase().contains("duplicate"), "{error}");

        let mut dangling = case3();
        dangling.branches_mut()[0].to = powerio::BusId(999_999);
        let error = crate::model::DcNetwork::from_network(&dangling)
            .err()
            .expect("dangling branch must reject");
        assert!(
            error.contains("999999") || error.to_lowercase().contains("unknown"),
            "{error}"
        );
    }
    /// Each clamp moves one end, so a window wholly on one side of zero and
    /// reaching past pi/2 used to come back inverted and make a solvable case
    /// infeasible.
    #[test]
    fn angle_bounds_never_invert() {
        for (amin, amax) in [
            (-std::f64::consts::PI, (-70f64).to_radians()),
            (100f64.to_radians(), 360f64.to_radians()),
        ] {
            let (lo, hi) = super::normalize_angle_bounds(amin, amax);
            assert!(
                lo <= hi,
                "inverted window from ({amin}, {amax}): ({lo}, {hi})"
            );
            assert!(lo.is_finite() && hi.is_finite());
        }
    }

    /// `is_normalized()` is a self-declared marker, and the element filtering it
    /// implies lives in the pass the fast path skips. An out-of-service load that
    /// survives it is summed into the nodal demand and priced into every LMP.
    #[test]
    fn falsely_normalized_network_rejects() {
        let mut net = case3();
        *net.source_format_mut() = powerio::SourceFormat::Normalized;
        assert!(
            !net.loads().is_empty(),
            "case3 must carry a load to disable"
        );
        net.loads_mut()[0].in_service = false;
        let error = crate::model::DcNetwork::from_network(&net)
            .err()
            .expect("a normalized claim with unfiltered elements must reject");
        assert!(
            error.contains("normalized") && error.contains("out-of-service load"),
            "{error}"
        );
    }

    /// The readers copy VMIN/VMAX verbatim and normalization leaves them alone,
    /// so an inverted or non-finite band reaches the model. `f64::clamp` panics
    /// on both.
    #[cfg(feature = "sensitivity")]
    #[test]
    fn inverted_voltage_band_does_not_panic() {
        for (vmin, vmax) in [(1.1, 0.9), (f64::NAN, 1.1)] {
            let mut net = case3();
            for bus in net.buses_mut() {
                bus.vmin = vmin;
                bus.vmax = vmax;
            }
            // The assertion is that this returns at all: `f64::clamp` panicking
            // here would abort the test rather than produce an `Err`.
            let _ = crate::model::AcNetwork::from_network(&net);
        }
    }

    /// A non-finite generator bound used to make `0.0 * inf` = NaN, which
    /// `f64::max` ignores, collapsing the shed price to its floor and quietly
    /// shedding load that should have been served.
    #[test]
    fn non_finite_generator_bounds_keep_the_shed_price_finite() {
        let mut net = case3();
        assert!(!net.generators().is_empty());
        net.generators_mut()[0].pmax = f64::INFINITY;
        let dc = crate::model::DcNetwork::from_network(&net)
            .expect("a non-finite bound must not break construction");
        assert!(
            dc.c_shed.iter().all(|value| value.is_finite()),
            "shed price must stay finite"
        );
    }
}
