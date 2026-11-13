//! Controllers as block graphs reproduce the compiled models in the time
//! domain.
//!
//! Every controller record of the single-machine cases built from WECC 240
//! (and of WECC 240 itself) that has a block template is replaced by its
//! template graph;
//! the run must match the compiled run. The graph puts time constants in the
//! mass matrix where some compiled models divide by them, so Newton takes a
//! slightly different path: the gate is the solver tolerance level, not
//! bitwise equality.

#![cfg(feature = "sensitivity")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::path::Path;

use tellegen::rms::{template, LoadModel, ModelOverride, RmsEvent, RmsRequest, RmsSimulation};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));

/// Largest difference of any channel the graph runs may show against the
/// compiled runs.
const GATE: f64 = 1e-7;

/// Open: on these cases the graph run departs from the compiled run (channels
/// up to 5e-5), and the cause is not found yet. Ruled out: the IEEEST filter
/// with A3 = A4 = 0 (A4 = 1e-3 leaves the gap) and its output limit (also
/// reached in `smib_genrou_gast_ieeest_sexs`, which agrees to 1e-13). The
/// compiled models are the ones validated against PowerSimulationsDynamics.jl.
/// These cases are held to [`OPEN_BOUND`] instead of [`GATE`]; one that starts
/// to agree fails the test, so the list stays current.
const OPEN_BOUND: f64 = 1e-4;

const OPEN: &[&str] = &[
    "smib_genrou_hygov_ieeest_sexs",
    "smib_genrou_ieeest_sexs_tgov1",
];

/// Template graphs for every linked controller record that has one.
fn overrides(net: &powerio::BalancedNetwork, dyr: &powerio::DynamicCase) -> Vec<ModelOverride> {
    let mut out = Vec::new();
    for m in powerio::link_machines(net, dyr).machines {
        for l in [&m.exciter, &m.governor, &m.stabilizer]
            .into_iter()
            .flatten()
        {
            if let Some(graph) = template(&l.model) {
                out.push(ModelOverride {
                    bus: m.bus,
                    machine_id: m.machine_id.clone(),
                    graph,
                });
            }
        }
    }
    out
}

fn worst(
    a: &std::collections::BTreeMap<String, Vec<f64>>,
    b: &std::collections::BTreeMap<String, Vec<f64>>,
) -> (f64, String) {
    let mut w = (0.0_f64, String::new());
    for (k, va) in a {
        let vb = b.get(k).unwrap_or_else(|| panic!("channel {k} missing"));
        assert_eq!(va.len(), vb.len(), "{k}");
        for (x, y) in va.iter().zip(vb) {
            let d = (x - y).abs();
            if d > w.0 || d.is_nan() {
                w = (d, k.clone());
            }
        }
    }
    w
}

#[test]
fn rms_templates_match_compiled_models() {
    let mut compared = 0;
    let mut failures = Vec::new();
    let only = std::env::var("RMS_GRAPH_CASES").ok();
    let t_end: Option<f64> = std::env::var("RMS_GRAPH_TEND")
        .ok()
        .and_then(|v| v.parse().ok());
    for case in wecc240_smib_cases() {
        let id = case.id.as_str();
        if only
            .as_ref()
            .is_some_and(|o| !o.split(',').any(|x| x == id))
        {
            continue;
        }
        let (network, dyr) = smib_study(&case);
        let net = &network;
        let models = overrides(net, &dyr);
        if models.is_empty() {
            continue;
        }
        let lm = LoadModel::ConstantImpedance;
        // Trip of one of the two lines to the infinite bus.
        let event = RmsEvent::BranchTrip {
            time: 1.0,
            from: 101,
            to: 102,
            circuit: "2".into(),
        };
        let request = RmsRequest {
            t_end: t_end.unwrap_or(3.0),
            dt: 0.0025,
            output_step: 0.005,
            load_model: lm,
            events: vec![event],
            small_signal: false,
            record: None,
        };
        let compiled = RmsSimulation::new(net, &dyr, lm)
            .unwrap()
            .run(&request)
            .unwrap();
        let graph = match RmsSimulation::with_models(net, &dyr, &models, lm)
            .and_then(|s| s.run(&request))
        {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{id}: {e}"));
                continue;
            }
        };
        let (d, k) = worst(&compiled.channels, &graph.channels);
        let names: Vec<&str> = models.iter().map(|m| m.graph.name.as_str()).collect();
        println!(
            "{id:<32} {} graph(s) {:?}: max |Δ| {d:.1e} ({k})",
            models.len(),
            names
        );
        compared += 1;
        let agrees = d <= GATE;
        if OPEN.contains(&id) {
            if agrees {
                failures.push(format!("{id} agrees now: take it off OPEN"));
            } else if !(d <= OPEN_BOUND) {
                failures.push(format!(
                    "{id}: max |Δ| {d:.2e} at {k} beyond {OPEN_BOUND:e}"
                ));
            }
        } else if !agrees {
            failures.push(format!("{id}: max |Δ| {d:.2e} at {k}"));
        }
    }
    assert!(compared > 0);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// WECC 240 (103 units: SEXS on all, TGOV1 on many) through a fault that
/// drives exciters to their ceiling: the limits act as in the compiled models.
#[test]
fn rms_templates_match_compiled_models_wecc240_fault() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240");
    let module = powerio::parse(powerio::Source::open(base.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(base.join("case.dyr")).unwrap()).unwrap();
    let models = overrides(net, &dyr);
    let request = RmsRequest {
        t_end: 3.0,
        dt: 0.005,
        output_step: 0.01,
        load_model: LoadModel::ConstantImpedance,
        events: vec![
            RmsEvent::BusFault {
                time: 0.5,
                bus: 1032,
                r: 0.0,
                x: 0.01,
            },
            RmsEvent::ClearFault {
                time: 0.6,
                bus: 1032,
            },
        ],
        small_signal: false,
        record: None,
    };
    let lm = LoadModel::ConstantImpedance;
    let a = RmsSimulation::new(net, &dyr, lm)
        .unwrap()
        .run(&request)
        .unwrap();
    let b = RmsSimulation::with_models(net, &dyr, &models, lm)
        .unwrap()
        .run(&request)
        .unwrap();
    let (d, k) = worst(&a.channels, &b.channels);
    let at_ceiling = a
        .channels
        .iter()
        .filter(|(k, _)| k.starts_with("efd:"))
        .filter(|(_, v)| v.iter().any(|x| *x > v[0] * 1.5))
        .count();
    let signals = b.channels.keys().filter(|k| k.starts_with("sig:")).count();
    println!(
        "RMS wecc240 fault: {} graph(s), {at_ceiling} exciters forced, max |Δ| {d:.1e} ({k}), {signals} block signals recorded",
        models.len()
    );
    // The diagram signal of each exciter's output block is its field voltage.
    let efd = &b.channels["efd:1032#C"];
    let vf = &b.channels["sig:exciter.Vf:1032#C"];
    assert!(efd.iter().zip(vf).all(|(a, b)| (a - b).abs() < 1e-12));
    assert!(at_ceiling > 0, "the fault does not force any exciter");
    assert!(d <= GATE, "{d:e} at {k}");
}
