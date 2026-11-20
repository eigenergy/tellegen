//! Generate the synthetic generation schema from the Rust records, for the
//! `@tellegen/engine/synth` TypeScript types.

use schemars::JsonSchema;

#[derive(JsonSchema)]
#[allow(dead_code)]
struct SynthSchema {
    request: tellegen_synth::SynthRequest,
    report: tellegen_synth::GenerationReportOutput<'static>,
    step: tellegen_synth::StepOutcome,
    preview: tellegen_synth::Preview,
    error: tellegen_synth::ErrorPayload,
}

fn main() {
    // Through a JSON value so the keys print in sorted order.
    let schema = schemars::schema_for!(SynthSchema).to_value();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("synth schema JSON")
    );
}
