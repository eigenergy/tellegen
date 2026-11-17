//! Browser response records shared by serialization and schema generation.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) type Object = BTreeMap<String, Value>;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum Model {
    Multiconductor,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum CoordinateSpace {
    Geographic,
    Projected,
    Diagram,
    Unknown,
    None,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum DistributionCoordinates {
    Geographic,
    Planar,
    Synthetic,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum BalancedCoordinates {
    File,
    SyntheticPending,
    Synthetic,
    Manual,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum JsonKind {
    Module,
    Transmission,
    Distribution,
    Ambiguous,
    Unknown,
}

impl TryFrom<&str> for JsonKind {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_value(Value::String(value.into())).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
impl PartialEq<&str> for JsonKind {
    fn eq(&self, other: &&str) -> bool {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(|s| s == *other))
            .unwrap_or(false)
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct GeoReport {
    pub matched_buses: usize,
    pub matched_branches: usize,
    pub unmatched_features: usize,
    pub notes: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct ParsedGeo {
    pub layer: String,
    pub diagnostics: Vec<Object>,
    pub n_points: usize,
    pub n_routes: usize,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct Attachment {
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminals: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct VoltageLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpn_min: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpn_max: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpp_min: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpp_max: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpp_pairs: Option<Vec<[String; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_min_by_terminal: Option<BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_max_by_terminal: Option<BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpos_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpos_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vneg_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vzero_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vn_max: Option<f64>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct GraphBus {
    pub id: String,
    pub terminals: Vec<String>,
    pub grounded: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present")]
    pub neutral_terminal: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xy: Option<[f64; 2]>,
    pub load_kw: f64,
    pub gen_kw: f64,
    pub has_source: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_attachments: Option<BTreeMap<String, Vec<Attachment>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage_limits: Option<VoltageLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal_voltage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct EdgeRating {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i_max: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s_rating_by_bus: Option<BTreeMap<String, f64>>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct Winding {
    pub bus: String,
    pub connection: String,
    pub terminals: Vec<String>,
    pub v_ref: f64,
    pub s_rating: f64,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct GraphEdge {
    pub kind: String,
    pub id: String,
    pub from: String,
    pub to: String,
    pub conductors: Vec<[String; 2]>,
    pub closed: bool,
    pub n_phases: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<EdgeRating>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windings: Option<Vec<Winding>>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct DistributionGraph {
    pub buses: Vec<GraphBus>,
    pub edges: Vec<GraphEdge>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct DistributionIngest {
    pub name: Option<String>,
    pub model: Model,
    pub n_bus: usize,
    pub n_edge: usize,
    pub n_line: usize,
    pub n_switch: usize,
    pub n_transformer: usize,
    pub n_load: usize,
    pub n_generator: usize,
    pub n_ibr: usize,
    pub n_source: usize,
    pub n_shunt: usize,
    pub n_capacitor: usize,
    pub load_kw: f64,
    pub gen_kw: f64,
    pub base_frequency: f64,
    pub has_coords: bool,
    pub placed_buses: usize,
    pub coords_space: CoordinateSpace,
    pub coords_kind: DistributionCoordinates,
    pub diagnostics: Vec<Object>,
    pub graph: DistributionGraph,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_retained: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_pf_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present")]
    pub mc_pf_unavailable_reason: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_pf_supported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_pf_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_energized_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg(feature = "mc-pf")]
    pub mc_excluded_islands: Option<Vec<tellegen::McExcludedIsland>>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct BalancedIngest<'a> {
    pub name: &'a str,
    pub base_mva: f64,
    pub n_bus: usize,
    pub n_branch: usize,
    pub n_analysis_bus: usize,
    pub n_analysis_branch: usize,
    pub n_gen: usize,
    pub load_mw: f64,
    pub gen_mw: f64,
    pub has_coords: bool,
    pub coords_kind: BalancedCoordinates,
    pub topology: super::Topology<'a>,
    pub diagnostics: Vec<Object>,
    pub warnings: Vec<String>,
    pub view: Option<super::View<'a>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formulation: Option<tellegen::Problem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coords_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<GeoReport>,
}

/// PowerIO owns diagnostic detail keys; require records while retaining every field.
pub(crate) fn diagnostic_objects(
    diagnostics: &[powerio::Diagnostic],
) -> Result<Vec<Object>, String> {
    diagnostics
        .iter()
        .map(|d| {
            serde_json::to_value(d)
                .map_err(|e| e.to_string())
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        })
        .collect()
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg(feature = "schema")]
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub(crate) enum IngestShape<'a> {
    Balanced(WithModuleText<BalancedIngest<'a>>),
    Distribution(DistributionIngest),
}

#[cfg(all(feature = "schema", feature = "conic", feature = "synth"))]
pub(crate) fn schema() -> Value {
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Responses<'a> {
        #[cfg(feature = "powerio-collections")]
        collection_supported: bool,
        #[cfg(feature = "powerio-collections")]
        collection_inspection: super::collection::Inspection,
        #[cfg(feature = "powerio-collections")]
        collection_import: super::collection::Imported,
        classifier: super::JsonClassificationOutput<'a>,
        ingest_balanced: WithModuleText<BalancedIngest<'a>>,
        ingest_balanced_view: BalancedIngest<'a>,
        ingest_distribution: DistributionIngest,
        json_drop: super::JsonDropOutput<'a>,
        parsed_geo: ParsedGeo,
        geo_report: GeoReport,
        mc_layout: tellegen::schematic::SchematicLayout,
        tx_layout: tellegen::tx_schematic::TxLayout,
        dc_grid: tellegen::dc_grid::DcGridSolution,
        ac_dc: tellegen::ac_dc::AcDcSolution,
        regulated: tellegen::regulation::RegulatedSolution,
        capabilities: Vec<tellegen::ProblemCaps>,
        exported_case: tellegen::study::ExportedCase,
        memory_stats: super::MemoryStats,
        synth_places: super::synth::PlacesInfo,
        synth_queries: Vec<String>,
        synth_generated: Generated<'a>,
        energized: tellegen::mc_pf::McEnergizedOutput,
        energized_retained: RetainedEnergized,
    }
    serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .for_serialize()
            .into_generator()
            .into_root_schema_for::<Responses<'static>>(),
    )
    .expect("browser output schema")
}

#[cfg(feature = "mc-pf")]
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct RetainedEnergized {
    pub input_retained: bool,
    pub network_scope: Option<tellegen::McNetworkScope>,
    pub excluded_islands: Vec<tellegen::McExcludedIsland>,
}

#[cfg(feature = "synth")]
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct Generated<'a> {
    pub module_json: String,
    pub report: tellegen_synth::GenerationReportOutput<'a>,
    #[cfg_attr(
        feature = "schema",
        schemars(with = "WithModuleText<BalancedIngest<'a>>")
    )]
    pub ingest: Value,
}

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct WithModuleText<T> {
    #[serde(flatten)]
    pub payload: T,
    pub module_json: String,
}
impl WithModuleText<Value> {
    pub(crate) fn into_value(mut self) -> Result<Value, String> {
        self.payload
            .as_object_mut()
            .ok_or("ingest payload is not an object")?
            .insert("module_json".into(), Value::String(self.module_json));
        Ok(self.payload)
    }
}
