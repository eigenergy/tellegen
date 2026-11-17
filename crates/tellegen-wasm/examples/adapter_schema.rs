//! Export browser response schemas from the serialized Rust records.
use sha2::Digest;
fn main() {
    let mut schema = tellegen_wasm::output_schema();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = serde_json::Map::new();
    for file in [
        "crates/tellegen-wasm/src/lib.rs",
        "crates/tellegen-wasm/src/outputs.rs",
        "crates/tellegen-wasm/src/dist.rs",
        "crates/tellegen-wasm/src/geo.rs",
        "crates/tellegen-wasm/src/synth.rs",
        "crates/tellegen-wasm/src/collection.rs",
        "crates/tellegen-wasm/examples/adapter_schema.rs",
        "crates/tellegen/src/api.rs",
        "crates/tellegen/src/study.rs",
        "crates/tellegen/src/equipment.rs",
        "crates/tellegen/src/substations.rs",
        "crates/tellegen/src/schematic.rs",
        "crates/tellegen/src/tx_schematic.rs",
        "crates/tellegen/src/dc_grid.rs",
        "crates/tellegen/src/ac_dc.rs",
        "crates/tellegen/src/regulation.rs",
        "crates/tellegen/src/mc_pf/energized.rs",
        "crates/tellegen-synth/src/report.rs",
        "crates/tellegen-synth/src/synchronous.rs",
    ] {
        let bytes = std::fs::read(root.join(file)).expect("browser response source");
        let hash: String = sha2::Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        sources.insert(file.into(), hash.into());
    }
    schema["x-rust-source-files"] = sources.into();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("browser schema JSON")
    );
}
