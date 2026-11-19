//! Generate the JSON Schema of the RMS records the browser engine exchanges
//! (`packages/engine/src/generated/rms.schema.json`):
//! `cargo run -p tellegen --example rms_schema --features schema,rms-experimental`.

use schemars::JsonSchema;

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
    // Through a JSON value so the keys print in sorted order.
    let schema = serde_json::to_value(schemars::schema_for!(RmsSchema)).expect("schema value");
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("RMS schema JSON")
    );
}
