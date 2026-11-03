use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn command(args: &[&str], input: &Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tellegen"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn cli_settings_and_calculation_match_native() {
    let request = json!({"calculation":"mc_ac_pf","overrides":{"damping":0.8}});
    let cli = command(&["settings", "resolve"], &request);
    let native =
        tellegen::settings::resolve(&serde_json::from_value(request.clone()).unwrap()).unwrap();
    assert_eq!(cli, serde_json::to_value(native).unwrap());
    let request = json!({"settings":request,"module_json":include_str!("../../../apps/web/tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json")});
    let cli = command(&["calculate"], &request);
    let native =
        tellegen::settings::calculate(&serde_json::from_value(request).unwrap(), None).unwrap();
    assert_eq!(cli, serde_json::to_value(native).unwrap());
}
