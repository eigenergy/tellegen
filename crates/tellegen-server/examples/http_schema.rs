//! Generate schemas from the records serialized by the HTTP server.
fn main() {
    use sha2::Digest;
    let mut schema = tellegen_server::http_result_schema();
    schema["x-server-source-sha256"] = sha2::Sha256::digest(include_bytes!("../src/lib.rs"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .into();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tellegen");
    let mut sources = serde_json::Map::new();
    for file in [
        "src/api.rs",
        "src/solve.rs",
        "src/sens/interface.rs",
        "src/sens/mod.rs",
        "src/dc_formula.rs",
    ] {
        let bytes = std::fs::read(root.join(file)).expect("shared result source");
        sources.insert(
            file.into(),
            sha2::Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                .into(),
        );
    }
    schema["x-engine-source-files"] = sources.into();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
