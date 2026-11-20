//! Export browser response schemas from the serialized Rust records.
fn main() {
    let schema = tellegen_wasm::output_schema();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("browser schema JSON")
    );
}
