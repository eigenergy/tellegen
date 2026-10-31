//! The generation request: what the caller asks for, with every default
//! resolved by `serde` so the echo in the report is complete. The request JSON
//! is hashed for provenance; the Overpass document never travels inside it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::SynthError;

/// A region to generate in: a GeoJSON `Polygon` or `MultiPolygon` geometry
/// (longitude, latitude pairs; the first ring of each polygon is the outer
/// ring, later rings are holes) and a label for reports and bus names.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// GeoJSON geometry object (`{"type":"Polygon","coordinates":[...]}`).
    pub geometry: serde_json::Value,
    #[serde(default = "default_label")]
    pub label: String,
}

fn default_label() -> String {
    "Selected region".to_owned()
}

impl Region {
    /// A rectangular region. `east` may exceed 180 for a box that crosses the
    /// antimeridian; the ring is then split into two polygons at the seam so it
    /// stays valid GeoJSON.
    pub fn bbox(west: f64, south: f64, east: f64, north: f64, label: &str) -> Self {
        let ring = |w: f64, e: f64| {
            serde_json::json!([[w, south], [e, south], [e, north], [w, north], [w, south]])
        };
        let geometry = if east > 180.0 {
            serde_json::json!({
                "type": "MultiPolygon",
                "coordinates": [[ring(west, 180.0)], [ring(-180.0, east - 360.0)]]
            })
        } else {
            serde_json::json!({ "type": "Polygon", "coordinates": [ring(west, east)] })
        };
        Self {
            geometry,
            label: label.to_owned(),
        }
    }
}

/// Where the grid comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Deterministic generation from the region, its population and the seed.
    #[default]
    Synthetic,
    /// A grid mapped from an Overpass (OpenStreetMap) document passed beside the
    /// request; electrical parameters are modelled, not surveyed.
    Osm,
}

/// Bus count per unit area when `target_bus_count` is not given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Low,
    Medium,
    #[default]
    High,
}

/// How closely the electrical model follows the region's data: `High` uses
/// population weighted loads and tight parameter spreads; `Low` spreads load
/// with a floor plus a random weight and widens the parameter spreads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    Low,
    #[default]
    High,
}

/// Generator fuel families carried by the report and the fuel mix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Fuel {
    Gas,
    Coal,
    Nuclear,
    Hydro,
    Wind,
    Solar,
    Storage,
    Other,
}

impl Fuel {
    pub const ALL: [Fuel; 8] = [
        Fuel::Gas,
        Fuel::Coal,
        Fuel::Nuclear,
        Fuel::Hydro,
        Fuel::Wind,
        Fuel::Solar,
        Fuel::Storage,
        Fuel::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Fuel::Gas => "gas",
            Fuel::Coal => "coal",
            Fuel::Nuclear => "nuclear",
            Fuel::Hydro => "hydro",
            Fuel::Wind => "wind",
            Fuel::Solar => "solar",
            Fuel::Storage => "storage",
            Fuel::Other => "other",
        }
    }
}

/// Peak demand of the region.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DemandSpec {
    /// Peak MW from the population of the places inside the region. Each place
    /// draws its country's average load per head of the places table (the
    /// national demand table, [`crate::demand::NationalDemand`]) times
    /// `peak_factor`, so a whole country reproduces its national peak; a place
    /// whose country has no entry draws the world average per person. With
    /// `per_capita_kw`, every person of every place draws that average load
    /// instead.
    Population {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per_capita_kw: Option<f64>,
        #[serde(default = "default_peak_factor")]
        peak_factor: f64,
    },
    /// A stated peak demand in MW.
    TotalMw { mw: f64 },
}

fn default_peak_factor() -> f64 {
    crate::demand::DEFAULT_PEAK_FACTOR
}

impl Default for DemandSpec {
    fn default() -> Self {
        DemandSpec::Population {
            per_capita_kw: None,
            peak_factor: default_peak_factor(),
        }
    }
}

/// What to do with landmasses that the topology cannot join.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FarIslandPolicy {
    /// Drop the buses of a landmass farther than `max_interconnect_km` from
    /// the connected system (the group of landmasses whose places hold the
    /// most people) and say so in a warning. The default: the result is one
    /// electrical island the engine can solve as a whole.
    #[default]
    Cut,
    /// Keep the landmass as its own electrical island with its own reference
    /// bus and generation. Validation runs per island.
    Keep,
}

/// Island handling for synthetic generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IslandOptions {
    /// Landmasses whose nearest buses are closer than this are joined by an
    /// interconnect corridor (submarine or overhead), like Sicily or the Danish
    /// islands. source: typical AC/HVDC interconnector lengths, ENTSO-E grid map.
    #[serde(default = "default_max_interconnect_km")]
    pub max_interconnect_km: f64,
    #[serde(default)]
    pub beyond: FarIslandPolicy,
}

fn default_max_interconnect_km() -> f64 {
    250.0
}

impl Default for IslandOptions {
    fn default() -> Self {
        Self {
            max_interconnect_km: default_max_interconnect_km(),
            beyond: FarIslandPolicy::default(),
        }
    }
}

/// Island handling for the OSM path, where real data leaves isolated
/// substations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OsmIslands {
    /// Join every component with the shortest same-voltage tie lines, tagged
    /// `interconnect`.
    #[default]
    Connect,
    /// Drop islands without generation and report them.
    Cut,
    /// Leave islands alone; validation runs per island.
    Keep,
}

/// Thresholds for mapping an Overpass document onto a grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OsmOptions {
    /// Lines and substations below this nominal voltage are distribution and
    /// are dropped.
    #[serde(default = "default_osm_voltage_floor_kv")]
    pub voltage_floor_kv: f64,
    /// Substation and converter elements within this radius collapse into one site.
    #[serde(default = "default_osm_cluster_radius_m")]
    pub cluster_radius_m: f64,
    /// A line end within this distance of a site snaps to it.
    #[serde(default = "default_osm_snap_km")]
    pub snap_km: f64,
    /// A DC line end within this distance of a converter snaps to it.
    #[serde(default = "default_osm_hvdc_snap_km")]
    pub hvdc_snap_km: f64,
    /// A plant within this distance of a site connects there.
    #[serde(default = "default_osm_plant_snap_km")]
    pub plant_snap_km: f64,
    /// Infrastructure this far outside the region polygon is still kept.
    #[serde(default = "default_osm_clip_buffer_km")]
    pub clip_buffer_km: f64,
    #[serde(default)]
    pub islands: OsmIslands,
    /// Fewer buses than this is `OsmInsufficient`.
    #[serde(default = "default_osm_min_buses")]
    pub min_buses: usize,
    /// Fewer branches than this is `OsmInsufficient`.
    #[serde(default = "default_osm_min_branches")]
    pub min_branches: usize,
}

fn default_osm_voltage_floor_kv() -> f64 {
    110.0
}
fn default_osm_cluster_radius_m() -> f64 {
    500.0
}
fn default_osm_snap_km() -> f64 {
    2.0
}
fn default_osm_hvdc_snap_km() -> f64 {
    25.0
}
fn default_osm_plant_snap_km() -> f64 {
    10.0
}
fn default_osm_clip_buffer_km() -> f64 {
    5.0
}
fn default_osm_min_buses() -> usize {
    3
}
fn default_osm_min_branches() -> usize {
    2
}

impl Default for OsmOptions {
    fn default() -> Self {
        Self {
            voltage_floor_kv: default_osm_voltage_floor_kv(),
            cluster_radius_m: default_osm_cluster_radius_m(),
            snap_km: default_osm_snap_km(),
            hvdc_snap_km: default_osm_hvdc_snap_km(),
            plant_snap_km: default_osm_plant_snap_km(),
            clip_buffer_km: default_osm_clip_buffer_km(),
            islands: OsmIslands::default(),
            min_buses: default_osm_min_buses(),
            min_branches: default_osm_min_branches(),
        }
    }
}

/// Reference ranges the calibration objective measures against. Each range is
/// `[low, high]` of the published or measured statistic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationTargets {
    /// Identifier of the reference set (`reference_stats.json` entry).
    pub set_id: String,
    /// Where the numbers come from.
    pub source: String,
    /// Mean nodal degree range.
    pub avg_degree: [f64; 2],
    /// Fraction of buses at the highest voltage level.
    pub hv_share: [f64; 2],
    /// Fraction of branches at or above the loading target in the DC OPF.
    pub binding_fraction: [f64; 2],
    /// 90th percentile branch loading floor (under-utilisation guard).
    pub loading_p90_min: f64,
    /// Branch loading the calibration keeps the worst branch under.
    pub loading_target: f64,
}

impl Default for CalibrationTargets {
    fn default() -> Self {
        Self::published()
    }
}

impl CalibrationTargets {
    /// Published structural ranges for North American transmission systems.
    /// source: Birchfield, Xu, Gegner, Shetye, Overbye, "Grid Structural
    /// Characteristics as Validation Criteria for Synthetic Networks", IEEE
    /// Trans. Power Systems 32(4), 2017, Table II (degree distribution, ratio
    /// of buses per voltage level) and Section IV (loading of binding lines in
    /// the base case OPF). The binding fraction range is the ACTIVSg base case
    /// behaviour: a few percent of lines at their limit, never none.
    pub fn published() -> Self {
        Self {
            set_id: "birchfield-2017-published".to_owned(),
            source: "Birchfield et al., IEEE TPWRS 32(4) 2017, Table II".to_owned(),
            avg_degree: [2.2, 3.0],
            hv_share: [0.08, 0.35],
            binding_fraction: [0.0, 0.08],
            loading_p90_min: 0.25,
            loading_target: 0.95,
        }
    }
}

/// OPF-in-the-loop calibration controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationOptions {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Each iteration is one declared parameter step with an exact DC OPF re-solve.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Allow the loop to scale total load (off: load is data, not a knob).
    #[serde(default)]
    pub allow_load_scale: bool,
    /// Override the reference targets.
    #[serde(default)]
    pub targets: Option<CalibrationTargets>,
}

fn default_true() -> bool {
    true
}

fn default_max_iterations() -> u32 {
    6
}

impl Default for CalibrationOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_iterations: default_max_iterations(),
            allow_load_scale: false,
            targets: None,
        }
    }
}

/// Validation controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ValidationOptions {
    #[serde(default = "default_true")]
    pub run_dc_opf: bool,
    #[serde(default = "default_true")]
    pub run_ac_pf: bool,
    /// Planning voltage band, per unit.
    #[serde(default = "default_planning_band")]
    pub voltage_band: [f64; 2],
    /// Emergency voltage band, per unit.
    #[serde(default = "default_emergency_band")]
    pub emergency_band: [f64; 2],
}

fn default_planning_band() -> [f64; 2] {
    [0.95, 1.05]
}

fn default_emergency_band() -> [f64; 2] {
    [0.9, 1.1]
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            run_dc_opf: true,
            run_ac_pf: true,
            voltage_band: default_planning_band(),
            emergency_band: default_emergency_band(),
        }
    }
}

/// Caps that keep a request inside what a browser session can afford.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Bus cap. A synthetic grid's count is held at it (`bus_count_capped`);
    /// an OpenStreetMap grid drops its lowest voltage levels until it fits
    /// (`osm_bus_count_capped`) and is refused when its highest level alone
    /// exceeds it.
    #[serde(default = "default_max_bus_count")]
    pub max_bus_count: u32,
    /// Spherical area cap of the region, km²; positive and finite.
    #[serde(default = "default_max_area_km2")]
    pub max_area_km2: f64,
    /// Upper bound of `branch_factor`; finite and at least 1.
    #[serde(default = "default_max_branch_factor")]
    pub max_branch_factor: f64,
    /// Maximum raw Overpass response size, in bytes.
    #[serde(default = "default_max_osm_document_bytes")]
    pub max_osm_document_bytes: usize,
    /// Maximum Overpass elements accepted before topology construction.
    #[serde(default = "default_max_osm_elements")]
    pub max_osm_elements: usize,
}

fn default_max_bus_count() -> u32 {
    3000
}

fn default_max_area_km2() -> f64 {
    4.0e7
}

fn default_max_branch_factor() -> f64 {
    4.0
}

fn default_max_osm_document_bytes() -> usize {
    crate::osm::MAX_OSM_DOCUMENT_BYTES
}

fn default_max_osm_elements() -> usize {
    crate::osm::MAX_OSM_ELEMENTS
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bus_count: default_max_bus_count(),
            max_area_km2: default_max_area_km2(),
            max_branch_factor: default_max_branch_factor(),
            max_osm_document_bytes: default_max_osm_document_bytes(),
            max_osm_elements: default_max_osm_elements(),
        }
    }
}

/// Why a caller reran an OSM request as a synthetic one. Written by the
/// caller, never read by the generator, excluded from the request hash, and
/// echoed in the report's provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FallbackFrom {
    pub source: Source,
    pub reason: FallbackReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buses: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branches: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    OsmInsufficient,
    OverpassFailed,
}

/// The one generation request. Unknown keys are refused here and in every
/// nested option, so a misspelt option fails instead of taking its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SynthRequest {
    pub region: Region,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default)]
    pub source: Source,
    #[serde(default)]
    pub density: Density,
    #[serde(default)]
    pub fidelity: Fidelity,
    /// Nominal voltage levels, kV, highest first after resolution. Each level
    /// keeps its own kV for the per-unit base and takes the per-km parameters
    /// of the nearest line class. Empty (the default) takes the national
    /// ladder of the region's dominant country by demand
    /// ([`crate::voltage::VoltageLadders`]); a stated list is used as given,
    /// and the corridor plan may then add 765 kV above it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voltage_levels_kv: Vec<f64>,
    /// Exact bus count; otherwise `density` × area decides.
    #[serde(default)]
    pub target_bus_count: Option<u32>,
    /// Branches per bus, `1.0..=limits.max_branch_factor`; otherwise by density.
    #[serde(default)]
    pub branch_factor: Option<f64>,
    #[serde(default)]
    pub demand: DemandSpec,
    /// Load power factor; `q = p · tan(acos pf)`.
    #[serde(default = "default_power_factor")]
    pub power_factor: f64,
    /// Installed capacity ≥ (1 + margin) × peak load.
    #[serde(default = "default_reserve_margin")]
    pub reserve_margin: f64,
    /// Number of generating units; otherwise 16% of the buses (at least 3).
    #[serde(default)]
    pub generator_count: Option<u32>,
    /// Capacity shares by fuel; normalised. Empty means the default mix.
    #[serde(default)]
    pub fuel_mix: BTreeMap<Fuel, f64>,
    #[serde(default)]
    pub islands: IslandOptions,
    #[serde(default)]
    pub osm: Option<OsmOptions>,
    #[serde(default)]
    pub calibration: CalibrationOptions,
    #[serde(default)]
    pub validation: ValidationOptions,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_from: Option<FallbackFrom>,
}

fn default_seed() -> u64 {
    2026
}

fn default_power_factor() -> f64 {
    0.95
}

fn default_reserve_margin() -> f64 {
    0.18
}

/// Default capacity mix when the request states none. source: EIA Electric
/// Power Annual 2023, Table 4.3 (U.S. net summer capacity shares: natural gas
/// 43%, coal 15%, nuclear 8%, hydro 6%, wind 12%, solar 8%, storage 2%),
/// rounded and renormalised without the residual "other" category.
pub fn default_fuel_mix() -> BTreeMap<Fuel, f64> {
    let mut m = BTreeMap::new();
    m.insert(Fuel::Gas, 0.42);
    m.insert(Fuel::Coal, 0.14);
    m.insert(Fuel::Nuclear, 0.08);
    m.insert(Fuel::Hydro, 0.07);
    m.insert(Fuel::Wind, 0.13);
    m.insert(Fuel::Solar, 0.12);
    m.insert(Fuel::Storage, 0.04);
    m
}

impl SynthRequest {
    /// A request with every default, for a region.
    pub fn new(region: Region) -> Self {
        serde_json::from_value(serde_json::json!({ "region": region }))
            .expect("a region alone is a complete request")
    }

    /// Reject what no stage could use. Context dependent checks (generator
    /// count against the resolved bus count, area against the cap) run in the
    /// context stage.
    pub fn validate(&self) -> Result<(), SynthError> {
        let bad = |m: String| Err(SynthError::InvalidRequest(m));
        // The upper bounds below sit far outside any real system; they keep a
        // huge but finite number from overflowing later arithmetic.
        if self.limits.max_osm_document_bytes == 0 {
            return bad("limits.max_osm_document_bytes must be at least 1".into());
        }
        if self.limits.max_osm_elements == 0 {
            return bad("limits.max_osm_elements must be at least 1".into());
        }
        if self.limits.max_bus_count == 0 {
            return bad("limits.max_bus_count must be at least 1".into());
        }
        // A NaN cap compares false against every area and would switch the
        // area check off.
        if !(self.limits.max_area_km2.is_finite() && self.limits.max_area_km2 > 0.0) {
            return bad("limits.max_area_km2 must be a positive finite number".into());
        }
        // `branch_factor` must lie in 1..=max, which is empty below 1.
        if !(self.limits.max_branch_factor.is_finite()
            && self.limits.max_branch_factor >= 1.0)
        {
            return bad("limits.max_branch_factor must be a finite number of at least 1".into());
        }
        // OSM controls feed spatial clustering and clipping. Reject invalid
        // numbers before they reach geometry operations.
        let osm = self.osm.as_ref().cloned().unwrap_or_default();
        for (name, value, allow_zero) in [
            ("osm.voltage_floor_kv", osm.voltage_floor_kv, false),
            ("osm.cluster_radius_m", osm.cluster_radius_m, false),
            ("osm.snap_km", osm.snap_km, false),
            ("osm.hvdc_snap_km", osm.hvdc_snap_km, false),
            ("osm.plant_snap_km", osm.plant_snap_km, false),
            ("osm.clip_buffer_km", osm.clip_buffer_km, true),
        ] {
            let valid =
                value.is_finite() && if allow_zero { value >= 0.0 } else { value > 0.0 };
            if !valid {
                let constraint = if allow_zero { "non-negative" } else { "positive" };
                return bad(format!("{name} must be a {constraint} finite number"));
            }
        }
        if !self
            .voltage_levels_kv
            .iter()
            .all(|v| v.is_finite() && (1.0..=1500.0).contains(v))
        {
            return bad("voltage levels must be between 1 and 1500 kV".into());
        }
        if let Some(n) = self.target_bus_count {
            if n < 1 || n > self.limits.max_bus_count {
                return bad(format!(
                    "target_bus_count must be between 1 and {}",
                    self.limits.max_bus_count
                ));
            }
        }
        if let Some(f) = self.branch_factor {
            if !f.is_finite() || !(1.0..=self.limits.max_branch_factor).contains(&f) {
                return bad(format!(
                    "branch_factor must be between 1 and {}",
                    self.limits.max_branch_factor
                ));
            }
        }
        match &self.demand {
            DemandSpec::Population {
                per_capita_kw,
                peak_factor,
            } => {
                if per_capita_kw.is_some_and(|kw| !(kw.is_finite() && kw > 0.0 && kw <= 100.0)) {
                    return bad("demand.per_capita_kw must be positive and at most 100".into());
                }
                if !(peak_factor.is_finite() && (1.0..=10.0).contains(peak_factor)) {
                    return bad("demand.peak_factor must be between 1 and 10".into());
                }
            }
            DemandSpec::TotalMw { mw } => {
                if !(mw.is_finite() && (0.0..=1.0e7).contains(mw)) {
                    return bad("demand.mw must be between 0 and 1e7".into());
                }
            }
        }
        if !(self.power_factor.is_finite() && self.power_factor > 0.0 && self.power_factor <= 1.0) {
            return bad("power_factor must be in (0, 1]".into());
        }
        if !(self.reserve_margin.is_finite() && (0.0..=10.0).contains(&self.reserve_margin)) {
            return bad("reserve_margin must be between 0 and 10".into());
        }
        if let Some(g) = self.generator_count {
            if let Some(n) = self.target_bus_count {
                if g > n {
                    return bad(format!(
                        "generator_count must be no greater than generated bus count ({n})"
                    ));
                }
            }
        }
        if self.fuel_mix.values().any(|w| !w.is_finite() || *w < 0.0) {
            return bad("fuel_mix shares must be non-negative finite numbers".into());
        }
        if !(self.islands.max_interconnect_km.is_finite()
            && self.islands.max_interconnect_km >= 0.0)
        {
            return bad("islands.max_interconnect_km must be non-negative".into());
        }
        for (name, band) in [
            ("validation.voltage_band", self.validation.voltage_band),
            ("validation.emergency_band", self.validation.emergency_band),
        ] {
            if !(band[0].is_finite() && band[1].is_finite() && 0.0 < band[0] && band[0] < band[1]) {
                return bad(format!("{name} must be an increasing positive pair"));
            }
        }
        crate::geom::region::validate_geometry(&self.region.geometry)
            .map_err(SynthError::InvalidRequest)?;
        Ok(())
    }

    /// The fuel mix with shares normalised to one (the default mix when empty
    /// or all zero).
    pub fn resolved_fuel_mix(&self) -> BTreeMap<Fuel, f64> {
        let total: f64 = self.fuel_mix.values().sum();
        let source = if self.fuel_mix.is_empty() || total <= 0.0 {
            default_fuel_mix()
        } else {
            self.fuel_mix.clone()
        };
        let total: f64 = source.values().sum();
        source
            .into_iter()
            .filter(|(_, w)| *w > 0.0)
            .map(|(f, w)| (f, w / total))
            .collect()
    }

    /// Voltage levels sorted highest first, duplicates removed.
    pub fn resolved_voltage_levels(&self) -> Vec<f64> {
        let mut levels = self.voltage_levels_kv.clone();
        levels.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        levels.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        levels
    }

    /// SHA-256 of the canonical request JSON (keys sorted at every level,
    /// `fallback_from` removed), as lowercase hex. Stable across field order
    /// and whitespace in the caller's JSON.
    pub fn sha256(&self) -> String {
        let mut value = serde_json::to_value(self).expect("request serializes");
        if let Some(obj) = value.as_object_mut() {
            obj.remove("fallback_from");
        }
        let canonical = canonical_json(&value);
        let digest = Sha256::digest(canonical.as_bytes());
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// JSON text with object keys sorted at every level. `serde_json::Value`
/// already orders object keys, so this is a format with no whitespace and
/// deterministic number spelling.
pub fn canonical_json(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).expect("string"),
                        canonical_json(&map[k])
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", parts.join(","))
        }
        other => serde_json::to_string(other).expect("scalar"),
    }
}
