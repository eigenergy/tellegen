//! Emit the build's settings contract and portable request/response schema.
fn main() {
    let value = serde_json::json!({
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
