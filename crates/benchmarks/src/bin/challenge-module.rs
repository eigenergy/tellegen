//! Convert one named MATPOWER source into a stored PowerIO module for the
//! challenge evidence runner. This binary is part of the non-shipping
//! validation harness, not a second Tellegen input format.

use std::io::Write;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("challenge-module: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let source_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: challenge-module <source-path> <public-source-name>")?;
    let public_name = args
        .next()
        .ok_or("usage: challenge-module <source-path> <public-source-name>")?;
    if args.next().is_some() {
        return Err("usage: challenge-module <source-path> <public-source-name>".into());
    }
    if public_name.trim().is_empty() || std::path::Path::new(&public_name).is_absolute() {
        return Err("public source name must be a nonempty relative path".into());
    }

    let bytes = std::fs::read(&source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let source = powerio::Source::from_bytes(public_name, bytes).map_err(|e| e.to_string())?;
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
    let module: powerio::PioModule<powerio::BalancedNetwork> = powerio::try_into_typed(module)
        .map_err(|mismatch| {
            format!(
                "source produced {}, not balanced_network",
                mismatch.actual()
            )
        })?;
    let dynamic = module.map_value(powerio::PioValue::BalancedNetwork);
    let encoded = powerio::stored::write_module(&dynamic).map_err(|e| e.to_string())?;
    std::io::stdout()
        .write_all(encoded.as_bytes())
        .map_err(|error| format!("cannot write module: {error}"))?;
    Ok(())
}
