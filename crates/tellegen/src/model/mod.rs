//! Private solver workspaces for PowerIO problem instances. Every path consumes
//! PowerIO's dense preparation, including its identities and row maps. A caller
//! that passes a raw [`BalancedNetwork`] has it prepared as given, so the
//! preparation's bus ids and source rows index the caller's own tables; only
//! the one based branch numbering of Tellegen's older numeric solve response is
//! derived here, from the preparation's branch sources.
//!
//! PowerIO compiles the declared generator cost into quadratic columns or exact
//! convex piecewise linear breakpoints. Tellegen keeps that distinction in its
//! private workspace. The angle difference window is PowerIO's PowerModels pad.
//! The DC model consumes the complete `DcOpfInstance`, including affine phase terms,
//! source rows, and synthesized limits. The AC model consumes the instance's complete
//! pi model, including terminal charging and lowered three-winding transformers.
//!
//! The two formulations split into [`mod@dc`] and [`mod@ac`].

use std::collections::HashSet;

use powerio::{BalancedNetwork, BusType};
use powerio_matrix::{AnalysisBranchSource, PiecewiseLinearCost};
use powerio_tx::HvdcTreatment;

#[cfg(feature = "sensitivity")]
mod ac;
#[cfg(feature = "acopf")]
mod acopf;
#[cfg(test)]
mod cases;
mod dc;
pub(crate) mod islands;
#[cfg(feature = "sensitivity")]
pub(crate) mod load_voltage;
#[cfg(all(test, feature = "sensitivity"))]
mod powerio_parity;

#[cfg(feature = "sensitivity")]
pub(crate) use ac::AcNetwork;
#[cfg(all(feature = "acopf", feature = "sensitivity"))]
pub(crate) use acopf::AcOpfDualPoint;
#[cfg(feature = "acopf")]
pub(crate) use acopf::{solve_ac_opf_cancellable, solve_joint_ac_dc, AcOpfSolved};
pub(crate) use dc::{csr_row, DcNetwork, LinearRow};

#[cfg(all(test, feature = "sensitivity"))]
pub(crate) use cases::parse_case3_ac;
#[cfg(test)]
pub(crate) use cases::{parse_case3, CASE3, CASE3_TWIN, CASE_ISLANDS};
#[cfg(all(test, feature = "sensitivity"))]
pub(crate) use cases::{parse_case9_ac, CASE9};

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
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
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

/// How a model treats the network's in-service HVDC lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HvdcLines {
    /// The model has no term for them, so an in-service line rejects the network.
    Reject,
    /// Each line's stated transfer enters the bus balance as fixed injections at
    /// its two terminal buses, PowerIO's `HvdcTreatment::FixedInjection`.
    FixedInjection,
}

/// PowerIO's account of the HVDC lines a model carries as fixed injections: what
/// they inject, and each in-service line left out because a terminal bus is
/// isolated. A network with no HVDC line reports nothing.
pub(super) fn hvdc_diagnostics(network: &BalancedNetwork) -> Vec<crate::api::SolveDiagnostic> {
    let injections = network.calc_hvdc_injections();
    // `to_diagnostics` states the lines with an inactive terminal last, one per
    // row in `inactive_terminal` order.
    let mut inactive = injections.inactive_terminal.iter();
    injections
        .to_diagnostics(HvdcTreatment::FixedInjection)
        .into_iter()
        .map(|diagnostic| {
            let buses = if diagnostic.code() == "BUILD.HVDC.TERMINAL_INACTIVE" {
                inactive
                    .next()
                    .map(|&row| {
                        let line = &network.hvdc()[row];
                        vec![line.from.0, line.to.0]
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            crate::api::SolveDiagnostic {
                code: diagnostic.code().to_owned(),
                message: diagnostic.message().to_owned(),
                buses,
            }
        })
        .collect()
}

/// PowerIO's statement of each bus coded PV that the instance specifies as PQ
/// because no in service generator is there; `buses` names that bus.
pub(super) fn voltage_control_diagnostics(
    instance: &powerio::AcPfInstance,
) -> Vec<crate::api::SolveDiagnostic> {
    let network = instance.network();
    // PowerIO reports the buses in table order; pair each with its id.
    let freed = network
        .buses()
        .iter()
        .zip(instance.specifications())
        .filter(|(bus, specification)| {
            bus.kind == BusType::Pv
                && matches!(specification, powerio::AcBusSpecification::Pq { .. })
                && !network
                    .generators()
                    .iter()
                    .any(|generator| generator.in_service && generator.bus == bus.id)
        })
        .map(|(bus, _)| bus.id.0);
    instance
        .voltage_control_diagnostics()
        .into_iter()
        .zip(freed)
        .map(|(diagnostic, bus)| crate::api::SolveDiagnostic {
            code: diagnostic.code().to_owned(),
            message: diagnostic.message().to_owned(),
            buses: vec![bus],
        })
        .collect()
}

/// Validate a caller's network for a model PowerIO prepares straight from it.
/// PowerIO's normalized marker is authoritative: a network that declares itself
/// normalized is already per unit and radians, and the preparation reads it so.
pub(super) fn validate_model_network(
    source: &BalancedNetwork,
    hvdc: HvdcLines,
) -> Result<(), String> {
    source.validate().map_err(|error| error.to_string())?;
    validate_canonical_identity(source)?;
    reject_unsupported_active_elements(source, hvdc)?;
    if source.is_normalized() {
        source.check_base_mva().map_err(|error| error.to_string())?;
        reject_unfiltered_normalized_elements(source)?;
    }
    Ok(())
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
/// filtering the marker claims lives in the normalization pass this path skips (and
/// before PowerIO 0.11.4 `IndexedNetwork` summed every load and shunt with no
/// in-service filter). Taking the marker at face value would serve an out-of-service
/// load and price it into every LMP while reporting `optimal`. Fail closed instead: a
/// network that really is normalized carries none of these.
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

/// Reactive-only SVC controls are outside the supported AC formulations.
/// DC models omit reactive quantities and report that approximation separately.
#[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
pub(super) fn reject_active_svcs(network: &BalancedNetwork) -> Result<(), String> {
    let count = network
        .static_var_compensators()
        .iter()
        .filter(|s| s.in_service)
        .count();
    if count == 0 {
        Ok(())
    } else {
        Err(format!("network contains {count} in-service static VAR compensator(s); AC SVC controls are not supported"))
    }
}

pub(super) fn dc_diagnostics(network: &BalancedNetwork) -> Vec<crate::api::SolveDiagnostic> {
    let mut diagnostics = hvdc_diagnostics(network);
    let buses: std::collections::BTreeSet<_> = network
        .static_var_compensators()
        .iter()
        .filter(|s| s.in_service)
        .map(|s| s.bus.0)
        .collect();
    if !buses.is_empty() {
        diagnostics.push(crate::api::SolveDiagnostic {
            code: "dc_svc_reactive_omitted".into(),
            message: "DC formulation omits static VAR compensator reactive controls, as it omits all reactive quantities".into(),
            buses: buses.into_iter().collect(),
        });
    }
    diagnostics
}

pub(super) fn reject_unsupported_active_elements(
    network: &BalancedNetwork,
    hvdc: HvdcLines,
) -> Result<(), String> {
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
    if let Some(detailed) = network.detailed_connectivity() {
        let covered: std::collections::HashSet<_> = network
            .hvdc()
            .iter()
            .filter(|h| h.in_service)
            .flat_map(|h| [h.converter1.as_ref(), h.converter2.as_ref()])
            .flatten()
            .map(|c| &c.component)
            .collect();
        let unmapped = detailed
            .voltage_source_converters
            .iter()
            .filter(|c| {
                c.dc_terminal1.connected != Some(false)
                    && c.dc_terminal2.connected != Some(false)
                    && !covered.contains(&c.component)
            })
            .count()
            + detailed
                .line_commutated_converters
                .iter()
                .filter(|c| {
                    c.dc_terminal1.connected != Some(false)
                        && c.dc_terminal2.connected != Some(false)
                        && !covered.contains(&c.component)
                })
                .count();
        if unmapped > 0 {
            unsupported.push(format!("{unmapped} detailed DC converter(s) without an explicit reduced injection equivalent; use a qualified DC-grid calculation"));
        }
    }
    if closed_switches > 0 {
        unsupported.push(format!("{closed_switches} closed switch(es)"));
    }
    if active_storage > 0 {
        unsupported.push(format!("{active_storage} in-service storage unit(s)"));
    }
    if active_hvdc > 0 && hvdc == HvdcLines::Reject {
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

/// One based ids for Tellegen's numeric solve response: a column with a
/// source row keeps that row's position, and a column with none (a lowered
/// three winding transformer winding) takes the next id after the source table.
pub(super) fn source_position_ids(
    source_rows: &[Option<usize>],
    source_len: usize,
    family: &str,
) -> Result<Vec<usize>, String> {
    let mut next_synthetic = source_len
        .checked_add(1)
        .ok_or_else(|| format!("{family} synthetic id space exhausted"))?;
    source_rows
        .iter()
        .map(|source_row| match source_row {
            Some(row) => {
                if *row >= source_len {
                    return Err(format!(
                        "{family} source row {row} outside source length {source_len}"
                    ));
                }
                row.checked_add(1)
                    .ok_or_else(|| format!("{family} id space exhausted"))
            }
            None => {
                let id = next_synthetic;
                next_synthetic = next_synthetic
                    .checked_add(1)
                    .ok_or_else(|| format!("{family} synthetic id space exhausted"))?;
                Ok(id)
            }
        })
        .collect()
}

/// One based branch ids for a model prepared from a caller's raw network:
/// PowerIO's row in its star-lowered analysis network, plus one. A source
/// branch keeps its row position and a lowered winding keeps the row PowerIO's
/// lowering gave it.
pub(super) fn network_branch_ids(analysis_rows: &[usize]) -> Result<Vec<usize>, String> {
    analysis_rows
        .iter()
        .map(|row| {
            row.checked_add(1)
                .ok_or_else(|| "branch id space exhausted".to_owned())
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

        // The DC model carries an HVDC line as fixed injections. The AC model
        // built from a network has no HVDC term; its power flow instance does.
        let mut hvdc = case3();
        hvdc.hvdc_mut()
            .push(powerio::Hvdc::new(powerio::BusId(1), powerio::BusId(2)));
        crate::model::DcNetwork::from_network(&hvdc).expect("DC carries HVDC lines");
        #[cfg(feature = "sensitivity")]
        {
            let ac = crate::model::AcNetwork::from_network(&hvdc)
                .expect_err("the AC network model has no HVDC term");
            assert!(ac.contains("in-service HVDC"), "unexpected AC error: {ac}");
        }
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
    /// infeasible. PowerIO's pad never inverts one.
    #[test]
    fn angle_bounds_never_invert() {
        for (amin, amax) in [(-180, -70), (100, 360)] {
            let mut net = case3();
            net.branches_mut()[0].angmin = f64::from(amin);
            net.branches_mut()[0].angmax = f64::from(amax);
            let dc = crate::model::DcNetwork::from_network(&net).expect("build");
            let (lo, hi) = (dc.angmin[0], dc.angmax[0]);
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
