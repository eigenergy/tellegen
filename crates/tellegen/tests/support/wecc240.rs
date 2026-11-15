// The WECC 240 case that ships in `tests/data/rms_examples/wecc240`, for the
// tests that read it. The published crate leaves that folder out (`exclude` in
// Cargo.toml), so a test on it is skipped there, with a line on stderr;
// `TELLEGEN_REQUIRE_CASE_DATA=1` turns the skip into a failure.

/// The folder of the shipped WECC 240 case, or `None` when it is not here.
#[allow(dead_code)]
fn shipped_wecc240() -> Option<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/rms_examples/wecc240");
    if dir.join("case.raw").is_file() && dir.join("case.dyr").is_file() {
        return Some(dir);
    }
    assert!(
        std::env::var_os("TELLEGEN_REQUIRE_CASE_DATA").is_none(),
        "the WECC 240 case data is required (TELLEGEN_REQUIRE_CASE_DATA) but not found"
    );
    eprintln!("skipped: the WECC 240 case data is not here (it is not in the published crate)");
    None
}

/// The shipped WECC 240 network, or `None` when its data is not here.
#[allow(dead_code)]
fn shipped_wecc240_network() -> Option<powerio::BalancedNetwork> {
    let dir = shipped_wecc240()?;
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    Some(net.clone())
}
