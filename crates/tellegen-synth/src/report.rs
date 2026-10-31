//! What a generation run reports: the resolved request, the region context,
//! per stage records, structural statistics, the engine's DC OPF and AC power
//! flow verdicts, the calibration trace, per element annotations and
//! provenance. Nothing here is adjusted after the fact: a solve that failed
//! says so.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::request::{FallbackFrom, Fuel, SynthRequest};

/// The schema token written into every report.
pub const REPORT_SCHEMA: &str = "tellegen.synth-report/1";

/// Generation stages, in order. `Calibrate` repeats once per iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Context,
    SiteBuses,
    Topology,
    Electrify,
    DcOpf,
    Calibrate,
    AcPf,
    Finalize,
    Done,
}

impl Stage {
    pub const ORDER: [Stage; 9] = [
        Stage::Context,
        Stage::SiteBuses,
        Stage::Topology,
        Stage::Electrify,
        Stage::DcOpf,
        Stage::Calibrate,
        Stage::AcPf,
        Stage::Finalize,
        Stage::Done,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Context => "context",
            Stage::SiteBuses => "site_buses",
            Stage::Topology => "topology",
            Stage::Electrify => "electrify",
            Stage::DcOpf => "dc_opf",
            Stage::Calibrate => "calibrate",
            Stage::AcPf => "ac_pf",
            Stage::Finalize => "finalize",
            Stage::Done => "done",
        }
    }

    /// Nominal progress fraction reached when this stage completes.
    pub fn progress(self) -> f64 {
        match self {
            Stage::Context => 0.05,
            Stage::SiteBuses => 0.15,
            Stage::Topology => 0.30,
            Stage::Electrify => 0.45,
            Stage::DcOpf => 0.60,
            Stage::Calibrate => 0.80,
            Stage::AcPf => 0.92,
            Stage::Finalize => 0.98,
            Stage::Done => 1.0,
        }
    }
}

/// A non-fatal condition the caller should show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Warning {
    pub code: String,
    pub message: String,
}

impl Warning {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

/// The outcome of one `step()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct StepOutcome {
    /// The stage that just ran.
    pub stage: Stage,
    /// Calibration iteration (1-based) when `stage == Calibrate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    /// The stage the next `step()` runs; `None` after `Done`.
    pub next: Option<Stage>,
    /// Progress in `[0, 1]`.
    pub progress: f64,
    pub done: bool,
    /// Warnings raised by this stage.
    pub warnings: Vec<Warning>,
    /// True when `preview()` would return something new.
    pub preview_changed: bool,
    /// Stage wall time when the host supplied a clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<f64>,
}

/// What a bus is for, as the map shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BusKind {
    Load = 0,
    Substation = 1,
    Generator = 2,
    Converter = 3,
}

impl BusKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BusKind::Load => "load",
            BusKind::Substation => "substation",
            BusKind::Generator => "generator",
            BusKind::Converter => "converter",
        }
    }
}

/// Where an element came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ElementSource {
    Synthetic,
    Osm,
    /// Generation added so an island or the system meets its reserve margin.
    Makeup,
    /// A tie added to join components.
    Interconnect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PreviewBus {
    pub id: usize,
    pub lon: f64,
    pub lat: f64,
    pub base_kv: f64,
    pub kind: BusKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PreviewBranch {
    pub from: usize,
    pub to: usize,
    pub base_kv: f64,
    /// Longitude/latitude polyline.
    pub route: Vec<[f64; 2]>,
}

/// The map preview: valid from `SiteBuses` on, branches from `Topology` on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Preview {
    pub stage: Stage,
    pub buses: Vec<PreviewBus>,
    pub branches: Vec<PreviewBranch>,
}

/// The preview as flat typed arrays for binary map attributes. Bus arrays are
/// in bus index order (id = index + 1); branch positions are the concatenated
/// routes with `branch_start_indices` marking where each route starts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewArrays {
    pub bus_positions: Vec<f64>,
    pub bus_kinds: Vec<u8>,
    pub bus_kv: Vec<f32>,
    pub branch_positions: Vec<f64>,
    pub branch_start_indices: Vec<u32>,
    pub branch_kv: Vec<f32>,
}

/// A longitude/latitude box; `east` may exceed 180 for a seam-crossing frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BBox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// How the demand figure was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DemandBasis {
    /// Each place draws its country's peak per head of the places table, from
    /// the national demand table (`RegionContext::demand_by_country`,
    /// `Provenance::demand_table`).
    National,
    /// Population of the places inside the region × the stated per-capita
    /// draw × peak factor.
    Population,
    /// The request stated the peak MW.
    TotalMw,
    /// No place fell inside the region: a per-bus default was used.
    DefaultPerBus,
}

/// Where a region's voltage levels came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VoltageBasis {
    /// The request stated the levels.
    Request,
    /// The national ladder of the region's dominant country
    /// (`RegionContext::voltage_country`, `Provenance::voltage_table`).
    Country,
    /// The dominant country has no ladder in the table: the ENTSO-E members'
    /// ladder for a member, else IEC 60038's standard levels (warning
    /// `voltage_fallback`).
    Fallback,
}

/// One country's part of a region's demand under the national basis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CountryDemand {
    /// Country code of the places table (Natural Earth `adm0_a3`).
    pub code: String,
    /// Year of the national figure; absent when the country has no entry and
    /// its places drew the world average.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    pub places: usize,
    /// Sum of `pop_max` over those places.
    pub population: f64,
    /// Peak demand of those places, MW.
    pub peak_mw: f64,
    /// The country's national peak (annual average load × peak factor), MW;
    /// `peak_mw` equals it when the region holds every place and at least 95%
    /// of the area of the country. Absent for the world fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub national_peak_mw: Option<f64>,
    /// Population of the country's listed places over its national
    /// population, at most 1: the share of its peak that follows its places.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listed_share: Option<f64>,
    /// Share of the country's area the region covers (each part of the region
    /// counted to the country of its nearest place); absent without an area.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area_share: Option<f64>,
    /// Of `peak_mw`, the part of the country's unlisted demand the region
    /// takes: in proportion to the area it covers, never beyond its share of
    /// the country's listed population, MW.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlisted_peak_mw: Option<f64>,
}

/// The region as the generator resolved it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RegionContext {
    pub label: String,
    /// Spherical area of the region polygons, km².
    pub area_km2: f64,
    pub center_lon: f64,
    pub center_lat: f64,
    /// Continuous longitude frame of the region.
    pub frame: BBox,
    pub crosses_antimeridian: bool,
    /// Polygons with usable area.
    pub landmasses: usize,
    /// Sum of `pop_max` over the populated places inside the region.
    pub population_in_region: f64,
    pub places_in_region: usize,
    /// Places counted in the region although just outside its polygons:
    /// within 15 km of its edge, of a country whose place population it
    /// holds at least half of, and moved onto that edge (harbours and
    /// estuaries the polygons leave out).
    pub places_snapped: usize,
    pub target_bus_count: u32,
    pub peak_demand_mw: f64,
    pub demand_basis: DemandBasis,
    /// The countries behind the demand, largest peak first (national basis
    /// only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demand_by_country: Vec<CountryDemand>,
    pub fuel_mix: BTreeMap<Fuel, f64>,
    /// Branches per bus the topology aims at.
    pub branch_factor: f64,
    /// Nominal levels, highest first; a level the corridor plan added above
    /// them (`PlanningReport::added_level_kv`) comes first once it ran.
    pub voltage_levels_kv: Vec<f64>,
    /// The line class each level takes its per-km parameters from.
    pub voltage_classes_kv: Vec<f64>,
    /// Where the levels came from.
    pub voltage_basis: VoltageBasis,
    /// The country whose ladder the levels are (table code), or whose missing
    /// entry the fallback stands in for; absent for stated levels or a region
    /// without places.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage_country: Option<String>,
    /// Higher AC levels the corridor plan may raise buses to, kV, highest
    /// first: the country's levels in service above its ladder, or 765 kV
    /// above stated levels. Empty when there is none, as in most of Europe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotion_levels_kv: Vec<f64>,
}

/// One completed stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct StageRecord {
    pub stage: Stage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<f64>,
    /// Element counts and other integers the stage produced.
    pub counts: BTreeMap<String, u64>,
}

/// Summary quantiles of a sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Quantiles {
    pub min: f64,
    pub p10: f64,
    pub p50: f64,
    pub mean: f64,
    pub p90: f64,
    pub max: f64,
}

/// One statistic beside its reference range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReferenceComparison {
    pub metric: String,
    pub value: f64,
    pub reference_range: [f64; 2],
    pub within: bool,
}

/// Structural statistics of the generated graph (and of a reference case when
/// measured with the CLI).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TopologyStats {
    pub n_buses: usize,
    pub n_branches: usize,
    pub n_generators: usize,
    pub n_loads: usize,
    pub n_transformers: usize,
    pub n_hvdc: usize,
    pub connected_components: usize,
    /// Second smallest Laplacian eigenvalue, by sparse power iteration; `None`
    /// above the size cap or for an empty graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub algebraic_connectivity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub algebraic_connectivity_converged: Option<bool>,
    /// `degree_histogram[d]` = number of buses with degree `d`.
    pub degree_histogram: Vec<u32>,
    pub avg_degree: f64,
    pub max_degree: usize,
    pub clustering_coefficient: f64,
    pub branch_bus_ratio: f64,
    pub line_length_km: Quantiles,
    pub total_line_length_km: f64,
    /// Total corridor length over the Euclidean minimum spanning tree length.
    pub mst_length_ratio: f64,
    /// Fraction of distinct corridors that are Delaunay edges of the bus points.
    pub delaunay_edge_fraction: f64,
    pub parallel_corridors: usize,
    pub parallel_branches: usize,
    /// Buses per nominal voltage (key = kV).
    pub voltage_counts: BTreeMap<String, u32>,
    /// Fraction of buses at the highest voltage level.
    pub hv_share: f64,
    pub total_load_mw: f64,
    pub total_generation_capacity_mw: f64,
    pub gen_load_margin: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_line_loading: Option<f64>,
    #[serde(default)]
    pub reference: Vec<ReferenceComparison>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SolveOutcome {
    Optimal,
    Infeasible,
    Skipped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LmpSummary {
    pub min: f64,
    pub mean: f64,
    pub max: f64,
    pub spread: f64,
}

/// One electrical island's verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct IslandOutcome {
    pub index: usize,
    pub buses: usize,
    pub branches: usize,
    pub generators: usize,
    pub load_mw: f64,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// The corridor and reactive plan the electrify stage made before any
/// validation ran: what the corridors were sized for and what was installed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PlanningReport {
    /// The planning case: `economic_dispatch_at_peak`, the least-cost
    /// dispatch of the generators serving peak load on a lossless network
    /// with no limits.
    pub dispatch: String,
    /// Share of its rating no circuit may exceed in the planning case.
    pub utilisation_target: f64,
    /// Capacity and stiffening rounds the corridor plan ran (each one DC
    /// power flow).
    pub rounds: u32,
    /// False when the plan stopped at its round or circuit cap.
    pub converged: bool,
    /// Buses raised to the next voltage level because an incident corridor
    /// would have needed more than two circuits at the lower one.
    pub promoted_buses: usize,
    /// The highest level the plan added above the modelled ones (one of
    /// `RegionContext::promotion_levels_kv`) because corridors at the highest
    /// modelled level needed more than two circuits; absent when the modelled
    /// levels sufficed or the region has no higher level in service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_level_kv: Option<f64>,
    /// Most parallel circuits on one corridor.
    pub max_parallel_circuits: u32,
    /// Parallel circuits one corridor may hold.
    pub circuit_cap: u32,
    /// Corridors at the cap whose planning flow still needs more circuits.
    pub capped_corridors: usize,
    /// Branches the requested branch factor asks for.
    pub branch_target: usize,
    /// Branches in the network.
    pub branches: usize,
    /// Circuits added beyond one per corridor because the planning flow
    /// needed them.
    pub capacity_circuits: usize,
    /// Transformer corridors whose bank the plan enlarged (up to the rating of
    /// the line it terminates) before adding circuits.
    pub upsized_transformers: usize,
    /// Circuits added to hold each island's angle spread within the St.
    /// Clair stability margin (44.4°).
    pub stiffening_circuits: usize,
    /// Widest island angle spread of the planning case (DC), degrees.
    pub angle_spread_deg: f64,
    /// Circuits added, heaviest planned loading first, to reach the target.
    pub filler_circuits: usize,
    /// Highest circuit loading in the planning case.
    pub max_planning_loading: f64,
    /// Corridors still above their rating in the planning case.
    pub overloaded_corridors: usize,
    pub capacitor_banks: usize,
    /// Total capacitor rating, MVAr at 1 pu.
    pub capacitor_mvar: f64,
    pub reactor_banks: usize,
    /// Total shunt reactor rating, MVAr at 1 pu.
    pub reactor_mvar: f64,
    /// Largest angle difference across one branch in the AC solution of the
    /// planning case at the planned voltage profile, degrees.
    pub max_angle_difference_deg: f64,
}

/// The DC OPF verdict from the tellegen engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DcOpfReport {
    pub status: SolveOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_loading: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loading_p90: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loading_mean: Option<f64>,
    /// Ten bins of width 0.1 from 0; the last bin holds loadings ≥ 0.9.
    pub loading_histogram: Vec<u32>,
    /// Branch uids with loading ≥ 0.999.
    pub binding_branches: Vec<String>,
    pub binding_fraction: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lmp: Option<LmpSummary>,
    pub dispatch_by_fuel_mw: BTreeMap<Fuel, f64>,
    /// 0 when optimal, absent when infeasible (nothing is invented).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unserved_mw: Option<f64>,
    pub islands: Vec<IslandOutcome>,
    /// HVDC rows replaced by a fixed transfer on the validation copy.
    pub hvdc_as_fixed_transfer: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solve_ms: Option<f64>,
    /// Per branch loading keyed by branch uid (the module's identity).
    #[serde(default)]
    pub branch_loading: BTreeMap<String, f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AcPfOutcome {
    Converged,
    NotConverged,
    Skipped,
}

/// The AC power flow verdict from the tellegen engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AcPfReport {
    pub status: AcPfOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newton_iterations: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_max: Option<f64>,
    pub buses_outside_planning_band: usize,
    pub buses_outside_emergency_band: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub losses_mw: Option<f64>,
    pub islands: Vec<IslandOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solve_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CalibrationStatus {
    /// Every objective term is within tolerance.
    Converged,
    /// The objective decreased but a term is still outside tolerance.
    Improved,
    /// No step improved the objective; `final_objective == initial_objective`.
    Unchanged,
    /// The DC OPF stayed infeasible through every step.
    Infeasible,
    /// Calibration was disabled or had no DC OPF to work with.
    Disabled,
}

/// One recorded calibration step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CalibrationStep {
    pub iteration: u32,
    /// The parameter changed (`rating_scale[230]`, `gen_capacity_scale[gas]`,
    /// `branch_rating_notch[branches:12]`, `load_scale`).
    pub parameter: String,
    pub before: f64,
    pub after: f64,
    pub reason: String,
    pub objective_before: f64,
    pub objective_after: f64,
    pub dc_status: SolveOutcome,
    pub accepted: bool,
}

/// The declared parameter set and its current values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CalibrationParameters {
    /// Rating multiplier per line class (key = class kV), bounds `[0.8, 1.5]`.
    pub rating_scale: BTreeMap<String, f64>,
    /// Capacity multiplier per fuel, bounds `[0.8, 1.5]`.
    pub gen_capacity_scale: BTreeMap<Fuel, f64>,
    /// Load multiplier; 1.0 unless `allow_load_scale`.
    pub load_scale: f64,
    /// Branches moved up one rating notch, by uid.
    pub branch_rating_notches: BTreeMap<String, u32>,
}

/// The calibration record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CalibrationReport {
    pub status: CalibrationStatus,
    pub iterations: u32,
    pub initial_objective: f64,
    pub final_objective: f64,
    pub terms_initial: BTreeMap<String, f64>,
    pub terms_final: BTreeMap<String, f64>,
    /// Objective weights, declared up front.
    pub weights: BTreeMap<String, f64>,
    pub parameters: CalibrationParameters,
    pub steps: Vec<CalibrationStep>,
    pub targets: crate::request::CalibrationTargets,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BusAnnotation {
    pub id: usize,
    pub uid: String,
    pub name: String,
    pub kind: BusKind,
    pub base_kv: f64,
    /// 0 = highest voltage level.
    pub tier: usize,
    pub confidence: f64,
    pub source: ElementSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub population_weight: f64,
    /// Index of the electrical island (0 = largest).
    pub island: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BranchAnnotation {
    /// 1-based branch position.
    pub id: usize,
    pub uid: String,
    pub class_kv: f64,
    pub length_km: f64,
    /// `internal`, `tie_line` (leaves the region), `interconnect`, `transformer`.
    pub boundary_role: String,
    pub is_transformer: bool,
    pub is_hvdc: bool,
    pub source: ElementSource,
    /// Parallel circuits in this branch's corridor, this branch included;
    /// every branch of a corridor carries the same count. It is the corridor
    /// plan's count for a synthetic corridor and for the transformers and
    /// connections OpenStreetMap mode adds, and the summed `circuits` tags
    /// (else `cables / 3`) of the joined ways for a mapped OpenStreetMap
    /// corridor. An HVDC link is one row for all of its circuits.
    pub circuits: u32,
    /// 1-based position of this branch among its corridor's circuits
    /// (`1..=circuits`); 1 for an HVDC link.
    pub circuit_index: u32,
    /// `thermal`, `loadability` (St. Clair), `transformer`, `calibrated`.
    pub rating_basis: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub osm_ids: Vec<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CostCoefficients {
    pub c2: f64,
    pub c1: f64,
    pub c0: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GeneratorAnnotation {
    pub uid: String,
    pub bus: usize,
    pub fuel: Fuel,
    pub pmax_mw: f64,
    pub cost: CostCoefficients,
    pub makeup: bool,
    pub slack: bool,
    /// A synchronous condenser the plan placed for voltage support in a
    /// remote pocket: no active power, reactive range only.
    #[serde(default)]
    pub condenser: bool,
}

/// Per element annotations keyed by the same ids and uids the ingest view uses.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ElementAnnotations {
    pub buses: Vec<BusAnnotation>,
    pub branches: Vec<BranchAnnotation>,
    pub generators: Vec<GeneratorAnnotation>,
}

/// The populated places table behind the run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PlacesProvenance {
    pub count: usize,
    /// SHA-256 of the table as loaded.
    pub sha256: String,
    /// The dataset the table was derived from (the bundled table names
    /// Natural Earth's release); absent for a bare array of rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// SHA-256 of that dataset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_sha256: Option<String>,
}

/// The voltage ladder table behind a country or fallback voltage basis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VoltageTableProvenance {
    pub schema: String,
    pub version: String,
    /// SHA-256 of the bundled table.
    pub sha256: String,
}

/// The national demand table behind a national demand basis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DemandTableProvenance {
    pub schema: String,
    /// The year most entries report.
    pub year: u32,
    pub source: String,
    pub licence: String,
    /// SHA-256 of the bundled table.
    pub sha256: String,
    /// SHA-256 of the dataset the table was derived from.
    pub source_sha256: String,
    /// Peak demand as a multiple of the annual average load.
    pub peak_factor: f64,
    /// SHA-256 of the bundled country area table the unlisted demand spreads
    /// over.
    pub areas_sha256: String,
    /// SHA-256 of the admin-0 file the areas were derived from.
    pub areas_source_sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct OsmProvenance {
    pub generator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_osm_base: Option<String>,
    /// The Overpass answer's `osm3s.copyright` notice: the licence (ODbL) the
    /// data was served under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// Licence identifier inferred from the Overpass attribution notice (when recognizable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    pub document_sha256: String,
    pub elements: usize,
    pub counts: BTreeMap<String, u64>,
}

/// Everything needed to reproduce or audit the run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Provenance {
    pub generator: String,
    pub seed: u64,
    pub request_sha256: String,
    pub tellegen_synth_version: String,
    pub tellegen_version: String,
    pub powerio_version: String,
    pub parameter_table_version: String,
    /// Log-normal spread applied to per-branch reactance.
    pub spread_sigma: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub places: Option<PlacesProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demand_table: Option<DemandTableProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage_table: Option<VoltageTableProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osm: Option<OsmProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_from: Option<FallbackFrom>,
}

/// The full report. Blocks are `None` until their stage has run, so a report
/// read mid-session is partial but never invented.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GenerationReport {
    pub schema: String,
    pub request: SynthRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<RegionContext>,
    pub stages: Vec<StageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<TopologyStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning: Option<PlanningReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dc_opf: Option<DcOpfReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ac_pf: Option<AcPfReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<CalibrationReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elements: Option<ElementAnnotations>,
    pub provenance: Provenance,
    pub warnings: Vec<Warning>,
}
