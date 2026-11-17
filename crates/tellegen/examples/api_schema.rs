//! Shared schemas for engine results and MCP arguments, from their Rust records.
use schemars::JsonSchema;
use sha2::{Digest, Sha256};
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
    mc_result: tellegen::mc_pf::McPfResult,
    mc_summary: tellegen::mc_pf::McPfSummary,
    mc_detail: tellegen::mc_pf::McPfDetail,
    mc_profile: tellegen::mc_pf::McPfProfile,
    mc_load_branches: Vec<tellegen::mc_pf::McLoadBranchState>,
    mc_study: tellegen::mc_pf::McStudySnapshot,
    calculation: tellegen::settings::CalculationResult,
    settings: tellegen::settings::ResolvedSettings,
    study_result: tellegen::study_ops::StudyOperationResult,
    study_summary: tellegen::document::StudySummary,
    study_bundle: tellegen::document::StudyBundle,
}
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        "src/api.rs",
        "src/dc_formula.rs",
        "src/error.rs",
        "src/sens/interface.rs",
        "src/solve.rs",
        "src/plan.rs",
        "src/study.rs",
        "src/mc_pf/mod.rs",
        "src/settings.rs",
        "src/study_ops.rs",
        "src/document.rs",
        "src/objective.rs",
        "src/preparation.rs",
        "src/exploration.rs",
        "examples/api_schema.rs",
    ];
    let mut hashes = serde_json::Map::new();
    for file in files {
        let bytes = std::fs::read(root.join(file)).expect("schema source file");
        hashes.insert(
            file.into(),
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                .into(),
        );
    }
    let mut schema = serde_json::to_value(schemars::schema_for!(ApiSchema)).expect("API schema");
    schema["x-rust-source-files"] = hashes.into();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("schema JSON")
    );
}
