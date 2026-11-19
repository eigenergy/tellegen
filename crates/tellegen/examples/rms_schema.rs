//! Generate the JSON Schema of the RMS records the browser engine exchanges
//! (`packages/engine/src/generated/rms.schema.json`):
//! `cargo run -p tellegen --example rms_schema --features schema,rms-experimental`.

use schemars::JsonSchema;
use sha2::{Digest, Sha256};
use std::fmt::Write;

#[derive(JsonSchema)]
#[allow(dead_code)]
struct RmsSchema {
    request: tellegen::rms::RmsRequest,
    options: tellegen::rms::RmsOptions,
    inspection: tellegen::rms::RmsInspection,
    criteria: tellegen::rms::ScreeningCriteria,
    screening: tellegen::rms::RmsScreening,
    generator: tellegen::rms::RmsGenerator,
    event: tellegen::rms::RmsEvent,
}

fn main() {
    let sources = [
        include_str!("../src/rms/mod.rs"),
        // The request and result types live here since the split of mod.rs.
        include_str!("../src/rms/api.rs"),
        include_str!("../src/rms/screen.rs"),
    ];
    let mut hash = Sha256::new();
    for source in sources {
        hash.update(source.as_bytes());
    }
    let mut schema = serde_json::to_value(schemars::schema_for!(RmsSchema)).expect("RMS schema");
    let mut hex = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut hex, "{byte:02x}").expect("write to String");
    }
    schema["x-rust-source-sha256"] = hex.into();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("RMS schema JSON")
    );
}
