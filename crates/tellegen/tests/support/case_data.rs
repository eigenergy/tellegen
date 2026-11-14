// Where the dynamic example cases are found. Included by the tests that read them.
//
// Shipped with the code (`tests/data`): WECC 240 (`rms_examples/wecc240`).
// Every other case is data and is kept outside this repository, see
// `tests/data/rms_examples/README.md`:
//   - `$TELLEGEN_CASE_DATA/rms_examples/<name>`, a checkout of a data repository, or
//   - `data/rms_examples/<name>` at the workspace root (`data/` is git-ignored).
// A test on such a case is skipped, with a line on stderr, when the data is not
// there. `TELLEGEN_REQUIRE_CASE_DATA=1` turns the skip into a failure.

/// The folders that hold dynamic example cases, shipped ones first.
#[allow(dead_code)]
fn rms_example_roots() -> Vec<std::path::PathBuf> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut roots = vec![manifest.join("tests/data/rms_examples")];
    if let Some(data) = std::env::var_os("TELLEGEN_CASE_DATA") {
        roots.push(std::path::PathBuf::from(data).join("rms_examples"));
    }
    roots.push(manifest.join("../../data/rms_examples"));
    roots
}

/// The folder of the case `name`, or `None` when its data is not on this machine.
#[allow(dead_code)]
fn rms_example(name: &str) -> Option<std::path::PathBuf> {
    let found = rms_example_roots()
        .into_iter()
        .map(|root| root.join(name))
        .find(|dir| dir.join("case.raw").is_file());
    if found.is_none() {
        assert!(
            std::env::var_os("TELLEGEN_REQUIRE_CASE_DATA").is_none(),
            "case data `{name}` is required (TELLEGEN_REQUIRE_CASE_DATA) but not found"
        );
        eprintln!("skipped: case data `{name}` is not on this machine (tests/data/rms_examples/README.md)");
    }
    found
}
