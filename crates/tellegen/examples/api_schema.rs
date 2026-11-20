//! Shared schemas for engine results and MCP arguments, from their Rust records.
use schemars::JsonSchema;
#[derive(JsonSchema)]
#[allow(dead_code)]
struct ApiSchema {
    solve: tellegen::SolveOutput,
    commit: tellegen::study::StudyCommit<'static>,
    edits: tellegen::Edits,
    sensitivity: tellegen::SensRequest,
    plan_spec: tellegen::CapacityPlanSpec,
    plan: tellegen::CapacityPlanOutcome,
    preview: tellegen::study::Preview,
    mc_metadata: tellegen::mc_pf::McPfResultMetadata<'static>,
    mc_strings: Vec<String>,
    mc_terminal_ids: Vec<(String, String)>,
    mc_result: tellegen::mc_pf::McPfResult,
    mc_summary: tellegen::mc_pf::McPfSummary,
    mc_detail: tellegen::mc_pf::McPfDetail,
    mc_profile: tellegen::mc_pf::McPfProfile,
    mc_load_branches: Vec<tellegen::mc_pf::McLoadBranchState>,
    mc_study: tellegen::mc_pf::McStudySnapshot,
    calculation: tellegen::settings::CalculationResult,
    settings: tellegen::settings::ResolvedSettings,
    settings_catalog: tellegen::settings::SettingsCatalog,
    study_result: tellegen::study_ops::StudyOperationResult,
    study_summary: tellegen::document::StudySummary,
    study_bundle: tellegen::document::StudyBundle,
}
fn main() {
    // Through a JSON value so the keys print in sorted order.
    let schema = schemars::schema_for!(ApiSchema).to_value();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("schema JSON")
    );
}
