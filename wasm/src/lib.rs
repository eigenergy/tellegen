use wasm_bindgen::prelude::*;

/// Parse a case file (MATPOWER, PSS/E RAW, PowerModels or egret JSON) and
/// return the network as a JSON string. Runs entirely in the browser.
#[wasm_bindgen]
pub fn parse_case(text: &str, format: &str) -> Result<String, JsError> {
    let net = powerio::parse_str(text, format).map_err(|e| JsError::new(&e.to_string()))?;
    powerio::to_json(&net).map_err(|e| JsError::new(&e.to_string()))
}

/// Quick stats without full serialization, for a drop preview.
#[wasm_bindgen]
pub fn case_summary(text: &str, format: &str) -> Result<String, JsError> {
    let net = powerio::parse_str(text, format).map_err(|e| JsError::new(&e.to_string()))?;
    let summary = serde_json::json!({
        "name": net.name,
        "base_mva": net.base_mva,
        "n_bus": net.buses.len(),
        "n_branch": net.branches.len(),
        "n_gen": net.generators.len(),
    });
    Ok(summary.to_string())
}
