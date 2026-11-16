//! Native validation runner for the complete Texas7k BMOPF conversion.
//! Uses Tellegen's explicit island selection; retains the full input module.

use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: texas7k-energized-runner <BMOPF JSON> <options JSON>".into());
    }
    let text = fs::read_to_string(&args[1])?;
    tellegen::validate_bmopf_json(&text)?;
    let source = powerio::Source::from_memory("input.bmopf.json", text.into_bytes())?;
    let module = powerio::parse_with_options(
        source,
        &powerio::ParseOptions::default().format("bmopf-json")?,
    )?;
    let serialized = tellegen::ir::serialize_module(&module)?;
    let preparation: serde_json::Value =
        serde_json::from_str(&tellegen::prepare_mc_energized_network_json(&serialized)?)?;
    let options = serde_json::from_str(&fs::read_to_string(&args[2])?)?;
    let selected = preparation["module_json"]
        .as_str()
        .ok_or("missing full module")?;
    if let Some(path) = env::var_os("TEXAS7K_MODULE_OUTPUT") {
        fs::write(path, selected)?;
    }
    println!("{}", tellegen::solve_mc_module_json(selected, &options)?);
    Ok(())
}
