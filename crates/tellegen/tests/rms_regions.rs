//! The network split into regions (`TELLEGEN_REGIONS`, a JSON map of bus
//! number to region), on a short bus fault on the 240-bus example case that
//! ships with the code. The map is read once per process, so each test here
//! writes a map and runs this test binary again with it (the ignored test
//! `run_in_a_child_process`, which writes its channels to a file); the
//! parent compares them with a run of its own, without regions:
//!
//! - with the network still factorized as one sparse LU
//!   (`TELLEGEN_NET_ONE=1`) only the residual goes by regions, which is the
//!   serial residual bit for bit: the run is the same bit for bit;
//! - with the network factorized by regions (a separator and one block per
//!   region) the Newton systems are the same, solved in another order: the
//!   run agrees to rounding.
//!
//! Without the shipped case data (the published crate) every test here is
//! skipped.

#![cfg(feature = "rms")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::path::Path;

use tellegen::rms::{LoadModel, RmsEvent, RmsRequest, RmsResult, RmsSimulation};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/wecc240.rs"
));

/// Where the child writes its channels.
const OUT: &str = "TELLEGEN_REGIONS_TEST_OUT";

/// What the child reports: the channels as the bits of their values, and
/// the region statistics of its run.
#[derive(serde::Serialize, serde::Deserialize)]
struct ChildRun {
    channels: BTreeMap<String, Vec<u64>>,
    steps: usize,
    stats: String,
}

/// The shipped 240-bus case, or `None` without its data.
fn sim() -> Option<RmsSimulation> {
    let dir = shipped_wecc240()?;
    let net = shipped_wecc240_network()?;
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    Some(RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap())
}

/// A 0.1 s bus fault from 0.1 s, run to 0.4 s.
fn request() -> RmsRequest {
    RmsRequest {
        t_end: 0.4,
        dt: 0.005,
        output_step: 0.005,
        load_model: Some(LoadModel::ConstantImpedance),
        events: vec![
            RmsEvent::BusFault {
                time: 0.1,
                bus: 1101,
                r: 0.0,
                x: 0.05,
            },
            RmsEvent::ClearFault {
                time: 0.2,
                bus: 1101,
            },
        ],
        small_signal: false,
        record: None,
    }
}

/// Four regions by the thousands of the bus numbers (the case's areas).
fn region_of(bus: usize) -> u16 {
    match bus / 1000 {
        1 | 2 => 0,
        3 => 1,
        4 | 5 => 2,
        _ => 3,
    }
}

/// The run of the child with the region map and `env`, and the parent's own
/// run without regions; `None` without the case data.
fn child_and_own(env: &[(&str, &str)]) -> Option<(ChildRun, RmsResult)> {
    let own = sim()?.run(&request()).unwrap();
    let map: BTreeMap<String, u16> = own
        .buses
        .iter()
        .map(|&b| (b.to_string(), region_of(b)))
        .collect();
    let tmp = std::env::temp_dir();
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = format!("{}-{run}", std::process::id());
    let map_path = tmp.join(format!("tellegen-regions-{tag}.json"));
    let out_path = tmp.join(format!("tellegen-regions-run-{tag}.json"));
    std::fs::write(&map_path, serde_json::to_string(&map).unwrap()).unwrap();
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "run_in_a_child_process",
        "--ignored",
        "--nocapture",
    ])
    .env("TELLEGEN_REGIONS", &map_path)
    .env("TELLEGEN_THREADS", "3")
    .env(OUT, &out_path)
    .env_remove("TELLEGEN_NET_ONE");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let _ = std::fs::remove_file(&map_path);
    let text = std::fs::read_to_string(&out_path);
    let _ = std::fs::remove_file(&out_path);
    assert!(
        out.status.success(),
        "the child failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let child: ChildRun = serde_json::from_str(&text.expect("the child's channels")).unwrap();
    // The child's residual went by regions.
    let calls: u64 = child
        .stats
        .strip_prefix("residual by regions: ")
        .and_then(|s| s.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("region statistics: {}", child.stats));
    assert!(calls > 0, "{}", child.stats);
    assert_eq!(
        child.channels.keys().collect::<Vec<_>>(),
        own.channels.keys().collect::<Vec<_>>()
    );
    Some((child, own))
}

/// Run by the tests below with a region map; without one it does nothing.
#[test]
#[ignore = "run in a child process with a region map by the tests of this file"]
fn run_in_a_child_process() {
    let Some(path) = std::env::var_os(OUT) else {
        eprintln!("skipped: run by the other tests of this file");
        return;
    };
    let r = sim()
        .expect("the shipped case data")
        .run(&request())
        .unwrap();
    let run = ChildRun {
        channels: r
            .channels
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().map(|x| x.to_bits()).collect()))
            .collect(),
        steps: r.steps,
        stats: tellegen::rms::region_stats(),
    };
    std::fs::write(Path::new(&path), serde_json::to_string(&run).unwrap()).unwrap();
}

/// With regions and the network as one sparse LU, the run is the run
/// without regions bit for bit: the residual by regions is the serial
/// residual.
#[test]
fn the_residual_by_regions_gives_the_run_bit_for_bit() {
    let Some((child, own)) = child_and_own(&[("TELLEGEN_NET_ONE", "1")]) else {
        return;
    };
    for (k, v) in &own.channels {
        let bits: Vec<u64> = v.iter().map(|x| x.to_bits()).collect();
        assert!(bits == child.channels[k], "{k}");
    }
    assert_eq!(child.steps, own.steps);
}

/// With the network factorized by regions too, the run agrees with the run
/// without regions to rounding: within 1e-9 of each channel's largest
/// magnitude (at least 1), as the other refactorizations of the same Newton
/// systems (a few 1e-12 seen).
#[test]
fn the_network_lu_by_regions_gives_the_run_to_rounding() {
    let Some((child, own)) = child_and_own(&[]) else {
        return;
    };
    let mut worst = (0.0, String::new());
    for (k, v) in &own.channels {
        let scale = v.iter().fold(1.0_f64, |m, x| m.max(x.abs()));
        for (x, b) in v.iter().zip(&child.channels[k]) {
            let d = (x - f64::from_bits(*b)).abs() / scale;
            if !(d <= worst.0) {
                worst = (d, k.clone());
            }
        }
    }
    eprintln!("largest difference by regions: {:e} ({})", worst.0, worst.1);
    assert!(worst.0 <= 1e-9, "{:e} in {}", worst.0, worst.1);
    assert_eq!(child.steps, own.steps);
}
