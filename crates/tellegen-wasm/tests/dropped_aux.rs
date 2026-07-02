//! The browser file drop path on a real user supplied PowerWorld `.aux` export.
//! This is the demo flow: clear the bundled ACTIVSg500 case, drop its `.aux`,
//! and get the same network back, solved and differentiable, entirely in wasm.
//!
//! The test runs the exact Rust the wasm build runs: `powerio::parse_str` for
//! the parse, `tellegen::geo::network_coords` + `spread_stacks` for the map view
//! that `ingest_case` builds, and `solve_dc_json` for the browser solve,
//! sensitivity, and demand-delta paths. Skips when the staged TAMU data is
//! absent (CI serves the embedded fallback and ships no `.aux`).

use std::path::PathBuf;

use serde_json::Value;
use tellegen::geo::{network_coords, spread_stacks};
use tellegen_wasm::solve_dc_json;

fn aux_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/ACTIVSg500/ACTIVSg500.aux")
}

#[test]
fn dropped_aux_parses_solves_and_differentiates() {
    let path = aux_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipping dropped_aux: {} not staged", path.display());
        return;
    };

    // 1. Parse. The `.aux` alone must yield a complete network: the drop guard in
    //    +page.svelte rejects an aux with no branches or generators.
    let net = powerio::parse_str(&text, "aux")
        .expect("parse ACTIVSg500.aux")
        .network;
    let n_bus = net.buses.len();
    assert!(n_bus >= 500, "buses {n_bus}");
    assert!(!net.branches.is_empty(), "aux carried no branches");
    assert!(
        net.generators.iter().any(|g| g.in_service),
        "aux carried no in-service generators"
    );

    // 2. Coordinates. The aux carries substation coords, so the map view is
    //    non-empty and covers the buses (the same network_coords + spread_stacks
    //    ingest_case builds the view from). spread_stacks must not panic.
    let mut coords = network_coords(&net);
    assert!(!coords.is_empty(), "aux produced no coordinates");
    let covered = net
        .buses
        .iter()
        .filter(|b| coords.contains_key(&b.id.0))
        .count();
    assert!(
        covered as f64 >= 0.99 * n_bus as f64,
        "only {covered}/{n_bus} buses have coordinates"
    );
    spread_stacks(&mut coords);

    let net_json = net.to_json().expect("network to_json");

    // 3. Base solve (the browser solveDc path): finite LMPs for every bus, plus a
    //    convergence trace for the solve-card sparkline.
    let base: Value = serde_json::from_str(&solve_dc_json(&net_json, "").expect("base solve"))
        .expect("base solve JSON");
    assert_eq!(base["lmp"].as_array().unwrap().len(), n_bus);
    assert!(base["lmp"]
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["usd_per_mwh"].as_f64().unwrap().is_finite()));
    assert!(
        base["objective"].as_f64().unwrap().is_finite()
            && base["objective"].as_f64().unwrap() > 0.0
    );
    assert!(
        !base["iterations"].as_array().unwrap().is_empty(),
        "no convergence trace"
    );

    // 4. Demand update: a delta at a real bus shifts the operating point.
    let bus = net.buses[0].id.0;
    let bumped: Value = serde_json::from_str(
        &solve_dc_json(&net_json, &format!(r#"{{"deltas":{{"{bus}":50.0}}}}"#))
            .expect("demand-delta solve"),
    )
    .expect("bumped solve JSON");
    assert!(
        (bumped["objective"].as_f64().unwrap() - base["objective"].as_f64().unwrap()).abs() > 1e-6,
        "demand delta had no effect on the objective"
    );

    // 5. Sensitivity: the dLMP/dd column for a selected bus (browser sens wasm).
    if cfg!(feature = "sensitivity") {
        let sens: Value = serde_json::from_str(
            &solve_dc_json(&net_json, &format!(r#"{{"sens_bus":{bus}}}"#))
                .expect("sensitivity solve"),
        )
        .expect("sens solve JSON");
        let col = &sens["dlmp_dd"];
        assert!(!col.is_null(), "dlmp_dd column present");
        assert_eq!(col["bus"].as_u64().unwrap() as usize, bus);
        let values = col["values"].as_array().unwrap();
        assert_eq!(values.len(), n_bus);
        assert!(values
            .iter()
            .all(|v| v["value"].as_f64().unwrap().is_finite()));
    }
}
