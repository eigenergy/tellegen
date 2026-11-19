//! Export the Rust records returned by the RMS browser adapter.
use sha2::Digest;

fn main() {
    let mut schema = tellegen_wasm::rms::output_schema();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = serde_json::Map::new();
    for file in [
        "crates/tellegen-wasm/examples/rms_adapter_schema.rs",
        "crates/tellegen-wasm/src/rms.rs",
        "crates/tellegen/src/rms/mod.rs",
        "crates/tellegen/src/rms/api.rs",
        "crates/tellegen/src/rms/screen.rs",
        "crates/tellegen/src/rms/linear.rs",
        "crates/tellegen/src/rms/library.rs",
    ] {
        let bytes = std::fs::read(root.join(file)).expect("RMS result source");
        let hash: String = sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        sources.insert(file.into(), hash.into());
    }
    schema["x-rust-source-files"] = sources.into();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("RMS schema JSON")
    );
}
