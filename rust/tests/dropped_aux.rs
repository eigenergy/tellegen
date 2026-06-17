//! The browser file-drop path on a real user-supplied PowerWorld `.aux` export.
//! This is the demo flow: clear the bundled ACTIVSg2000 case, drop its `.aux`,
//! and get the same network back, solved and differentiable, entirely in wasm.
//!
//! The test runs the exact Rust the wasm build runs: `powerio::parse_str` for
//! the parse, `geo::network_coords` + `spread_stacks` for the map view that
//! `ingest_case` builds, and `dc::solve_network` for the browser solve,
//! sensitivity, and demand-delta paths. Skips when the staged TAMU data is
//! absent (CI serves the embedded fallback and ships no `.aux`).

use std::collections::HashMap;
use std::path::PathBuf;

use tellegen::dc::{solve_network, DcSolveRequest};
use tellegen::geo::{network_coords, spread_stacks};

fn aux_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/ACTIVSg2000/ACTIVSg2000.aux")
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
        .expect("parse ACTIVSg2000.aux")
        .network;
    assert!(net.buses.len() >= 2000, "buses {}", net.buses.len());
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
        covered as f64 >= 0.99 * net.buses.len() as f64,
        "only {covered}/{} buses have coordinates",
        net.buses.len()
    );
    spread_stacks(&mut coords);

    // 3. Base solve (the browser solveDc path): finite LMPs for every bus, plus a
    //    convergence trace for the solve-card sparkline.
    let base = solve_network(&net, &DcSolveRequest::default()).expect("base solve");
    assert_eq!(base.lmp.len(), net.buses.len());
    assert!(base.lmp.iter().all(|l| l.usd_per_mwh.is_finite()));
    assert!(base.objective.is_finite() && base.objective > 0.0);
    assert!(!base.iterations.is_empty(), "no convergence trace");

    // 4. Demand update: a delta at a real bus shifts the operating point.
    let bus = net.buses[0].id.0;
    let deltas: HashMap<usize, f64> = [(bus, 50.0)].into_iter().collect();
    let bumped = solve_network(
        &net,
        &DcSolveRequest {
            deltas,
            sens_bus: None,
        },
    )
    .expect("demand-delta solve");
    assert!(bumped.objective.is_finite());
    assert!(
        (bumped.objective - base.objective).abs() > 1e-6,
        "demand delta had no effect on the objective"
    );

    // 5. Sensitivity: the dLMP/dd column for a selected bus (browser sens wasm).
    if cfg!(feature = "sensitivity") {
        let sens = solve_network(
            &net,
            &DcSolveRequest {
                deltas: HashMap::new(),
                sens_bus: Some(bus),
            },
        )
        .expect("sensitivity solve");
        let col = sens.dlmp_dd.expect("dlmp_dd column present");
        assert_eq!(col.bus, bus);
        assert_eq!(col.values.len(), net.buses.len());
        assert!(col.values.iter().all(|v| v.value.is_finite()));
    }
}
