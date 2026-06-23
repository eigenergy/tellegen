//! `tellegen` — a thin CLI over the engine's stateless JSON contract, for
//! reproducible solves, parity checks, and scripting. The network is read as
//! powerio `Network` JSON on stdin; the solve request (see [`tellegen::solve_json`])
//! is the first argument, or the word `capabilities` to print the support matrix.
//!
//! ```text
//! tellegen capabilities
//! tellegen < case.json                       # base-case DC OPF ({} request)
//! tellegen '{"formulation":"socwr"}' < case.json
//! ```
//!
//! This is the stateless face of the engine; interactive, build-once workflows use
//! the `Session` API (see the core crate). Phase 5 will grow this into the full
//! parity/repro harness (M5).

use std::io::Read;
use std::process::ExitCode;

const USAGE: &str =
    "usage: tellegen [REQUEST_JSON | capabilities | --help]   (network JSON on stdin)\n\
     \n\
     REQUEST_JSON  a solve request; default '{}' is a base-case DC OPF.\n\
     capabilities  print the formulation/operand/parameter capability matrix.";

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();

    match arg.as_str() {
        "capabilities" => {
            println!("{}", tellegen::capabilities_json());
            return ExitCode::SUCCESS;
        }
        "-h" | "--help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let request = if arg.is_empty() { "{}" } else { arg.as_str() };

    let mut network_json = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut network_json) {
        eprintln!("tellegen: failed to read network JSON from stdin: {e}");
        return ExitCode::FAILURE;
    }
    if network_json.trim().is_empty() {
        eprintln!("tellegen: no network JSON on stdin (try: tellegen < case.json)\n\n{USAGE}");
        return ExitCode::FAILURE;
    }

    match tellegen::solve_json(&network_json, request) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tellegen: solve failed: {e}");
            ExitCode::FAILURE
        }
    }
}
