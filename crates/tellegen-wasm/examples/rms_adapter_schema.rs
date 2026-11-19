//! Export the Rust records returned by the RMS browser adapter.
fn main() {
    let schema = tellegen_wasm::rms::output_schema();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("RMS schema JSON")
    );
}
