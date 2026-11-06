//! Emit the build's settings contract and portable request/response schema.
fn main() {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(
        concat!(
            include_str!("../src/settings.rs"),
            include_str!("../src/api.rs"),
            include_str!("../src/mc_pf/mod.rs")
        )
        .as_bytes(),
    );
    let value = serde_json::json!({
        "x-rust-source-sha256": hash.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "catalog": tellegen::settings::describe(),
        "schema": schemars::schema_for!(Contract)
    });
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}
#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct Contract {
    settings: tellegen::settings::SettingsRequest,
    resolved: tellegen::settings::ResolvedSettings,
    calculation: tellegen::settings::CalculationRequest,
    result: tellegen::settings::CalculationResult,
}
