//! `solve-mc` and `solve-mc-bmopf` output views and solve profile.
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

const MODULE: &str = include_str!(
    "../../../apps/web/tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json"
);
const BMOPF: &str = include_str!(
    "../../../apps/web/tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.bmopf.json"
);

fn run(args: &[&str], input: &str) -> (bool, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tellegen"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn solve(args: &[&str], input: &str) -> Value {
    let (ok, stdout, stderr) = run(args, input);
    assert!(ok, "{args:?} failed: {stdout}{stderr}");
    serde_json::from_str(&stdout).unwrap()
}

const PROFILE_PHASES: [&str; 9] = [
    "parse_ms",
    "prepare_ms",
    "factor_ms",
    "load_evaluation_ms",
    "kcl_and_matvec_ms",
    "linear_solve_ms",
    "summary_ms",
    "output_ms",
    "serialize_ms",
];

#[test]
fn default_output_is_the_full_result() {
    for (command, input) in [("solve-mc", MODULE), ("solve-mc-bmopf", BMOPF)] {
        let plain = solve(&[command], input);
        assert_eq!(solve(&[command, r#"{"output":"full"}"#], input), plain);
        assert!(plain.get("terminals").is_some() && plain.get("profile").is_none());
        // The session path builds the same complete result.
        let mut profiled = solve(&[command, r#"{"profile":true}"#], input);
        let profile = profiled.as_object_mut().unwrap().remove("profile").unwrap();
        assert_eq!(profiled, plain, "{command}");
        assert_eq!(profile["timed"], true);
        for phase in PROFILE_PHASES {
            assert!(profile[phase].as_f64().unwrap() >= 0.0, "{command} {phase}");
        }
        assert!(profile["parse_ms"].as_f64().unwrap() > 0.0, "{command}");
        assert_eq!(profile["iterations"], plain["iterations"]);
    }
}

#[test]
fn summary_and_tables_are_compact_and_deterministic() {
    for (command, input) in [("solve-mc", MODULE), ("solve-mc-bmopf", BMOPF)] {
        let full = solve(&[command], input);
        let summary_args = [command, r#"{"output":"summary"}"#];
        let (_, first, _) = run(&summary_args, input);
        let (_, second, _) = run(&summary_args, input);
        assert_eq!(first, second, "{command} summary is not deterministic");
        let summary: Value = serde_json::from_str(&first).unwrap();
        for field in ["converged", "iterations", "physical_kcl_residual"] {
            assert_eq!(summary[field], full[field], "{command} {field}");
        }
        assert_eq!(
            summary["terminal_count"].as_u64().unwrap() as usize,
            full["terminals"].as_array().unwrap().len()
        );
        assert!(summary.get("terminals").is_none() && summary.get("profile").is_none());

        let tables_args = [command, r#"{"output":"tables","damping":1.0}"#];
        let (ok, first, stderr) = run(&tables_args, input);
        assert!(ok, "{command} tables failed: {stderr}");
        let (_, second, _) = run(&tables_args, input);
        assert_eq!(first, second, "{command} tables are not deterministic");
        let tables: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(tables["summary"], summary, "{command} tables summary");
        let columns = &tables["tables"];
        let terminals = full["terminals"].as_array().unwrap();
        let voltage = columns["terminal_voltage"].as_array().unwrap();
        assert_eq!(voltage.len(), 2 * terminals.len());
        // Row for row in the full result's order, complex values as [re, im].
        for (row, terminal) in [0, terminals.len() - 1].map(|row| (row, &terminals[row])) {
            assert_eq!(
                voltage[2 * row],
                terminal["voltage"]["re"],
                "{command} row {row}"
            );
            assert_eq!(
                voltage[2 * row + 1],
                terminal["voltage"]["im"],
                "{command} row {row}"
            );
        }
        assert_eq!(columns["iterations"], full["iterations"]);

        let profiled = solve(&[command, r#"{"output":"summary","profile":true}"#], input);
        assert!(profiled["profile"]["total_ms"].as_f64().unwrap() > 0.0);
    }
}

#[test]
fn view_keys_are_validated() {
    for options in [
        r#"{"output":"compact"}"#,
        r#"{"profile":"yes"}"#,
        r#"{"not_an_option":1}"#,
    ] {
        for (command, input) in [("solve-mc", MODULE), ("solve-mc-bmopf", BMOPF)] {
            let (ok, _, stderr) = run(&[command, options], input);
            assert!(!ok, "{command} accepted {options}");
            assert!(!stderr.is_empty());
        }
    }
}
