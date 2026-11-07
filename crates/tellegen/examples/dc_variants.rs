//! Export all five conventions for the independent PowerModels/AC harness.
use powerio::{PioValue, Source};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: dc_variants CASE.m")?;
    let module = powerio::parse(Source::open(&path)?)?;
    let PioValue::BalancedNetwork(_) = module.value() else {
        return Err("balanced case required".into());
    };
    let text = tellegen::ir::serialize_module(&module)?;
    let mut results = serde_json::Map::new();
    for formula in [
        "series_susceptance",
        "series_susceptance_no_phase_shift",
        "tap_adjusted_reactance",
        "tap_adjusted_series_susceptance",
        "reactance_only",
    ] {
        let mut solves = serde_json::Map::new();
        for formulation in ["dcpf", "dcopf"] {
            let request = json!({"formulation":formulation,"branch_susceptance_formula":formula});
            let result = match tellegen::solve_module_json(&text, &request.to_string()) {
                Ok(response) => serde_json::from_str(&response)?,
                Err(error) => json!({"error":error}),
            };
            solves.insert(formulation.into(), result);
        }
        results.insert(formula.into(), solves.into());
    }
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}
