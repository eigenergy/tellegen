//! Generate the synthetic generation schema from the Rust records, for the
//! `@tellegen/engine/synth` TypeScript types.

use schemars::JsonSchema;
use sha2::{Digest, Sha256};
use std::fmt::Write;

#[derive(JsonSchema)]
#[allow(dead_code)]
struct SynthSchema {
    request: tellegen_synth::SynthRequest,
    report: tellegen_synth::GenerationReport,
    step: tellegen_synth::StepOutcome,
    preview: tellegen_synth::Preview,
    error: tellegen_synth::ErrorPayload,
}

fn main() {
    let sources = [
        include_str!("../src/request.rs"),
        include_str!("../src/report.rs"),
        include_str!("../src/error.rs"),
    ];
    let mut hash = Sha256::new();
    for source in sources {
        hash.update(source.as_bytes());
    }
    let mut schema =
        serde_json::to_value(schemars::schema_for!(SynthSchema)).expect("synth schema");
    let digest = hash.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("write to String");
    }
    schema["x-rust-source-sha256"] = hex.into();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("synth schema JSON")
    );
}
