//! Checks of how Tellegen's models map PowerIO's operators (issue #102).
//!
//! Every matrix the models read is PowerIO's: the AC model's bus admittance
//! matrix is its preparation's `calc_admittance_matrix`, its branch blocks and
//! flows are `AcBranchData`'s, and the DC model's incidence, Laplacian, and
//! phase shift injection are the DC preparation's. Nothing here re-derives
//! them. What Tellegen owns is the mapping: which preparation it builds, the
//! dense bus and branch order its vectors follow, and the jumper-charging edit
//! it applies before PowerIO stamps the matrix. The checks below hold that
//! mapping to PowerIO's other entry points (`DcOperators`, the AC power
//! balance) and to a hand-written three-bus matrix.

use std::collections::BTreeMap;

use num_complex::Complex64;
use powerio::BalancedNetwork;
use powerio_matrix::{
    build_dc_opf_preparation, DcOperators, DcOpfAssemblyOptions, SynthesizedLimitWindow, Units,
};
use powerio_prob::{DcOpfInstance, DcPfInstance};
use powerio_tx::{AngleDifferenceBounds, BranchSusceptanceFormula, HvdcTreatment};

use super::islands::{energize, ReferencePolicy};
use super::{AcNetwork, DcNetwork};

/// The network each model reads after energizing, built the way
/// [`DcNetwork::from_network`] builds it: PowerIO prepares it as given.
fn analysis_network(raw: &BalancedNetwork) -> BalancedNetwork {
    energize(raw, ReferencePolicy::GeneratorHosted)
        .network(raw)
        .clone()
}

/// The model's admittance matrix as `(row, col) -> G + jB`, every stored entry.
fn ybus_entries(model: &AcNetwork) -> BTreeMap<(usize, usize), Complex64> {
    let ybus = model.ybus();
    let mut entries: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for (&g, (r, c)) in ybus.g.iter() {
        entries.entry((r, c)).or_default().re = g;
    }
    for (&b, (r, c)) in ybus.b.iter() {
        entries.entry((r, c)).or_default().im = b;
    }
    entries
}

/// The model's admittance matrix and branch columns must describe the same
/// network in the same dense order: at an arbitrary voltage, PowerIO's bus
/// injection `V conj(Y V)` over the matrix equals the sum of PowerIO's branch
/// terminal flows leaving each bus plus the bus shunt. This holds the dense
/// bus map, the branch endpoints, and the jumper-charging edit (which must
/// reach both the matrix and the flows) to each other. Returns the largest
/// mismatch relative to the bus's largest term.
fn assert_ac_mapping(name: &str, model: &AcNetwork) -> f64 {
    let n = model.n;
    let vm: Vec<f64> = (0..n)
        .map(|i| 1.0 + 0.05 * ((i * 7 % 11) as f64 / 10.0 - 0.5))
        .collect();
    let va: Vec<f64> = (0..n)
        .map(|i| 0.3 * ((i * 5 % 13) as f64 / 12.0 - 0.5))
        .collect();
    let (p, q) = powerio_matrix::calc_bus_power_injection(model.ybus(), &vm, &va)
        .expect("bus power injection");
    let flows = model.branches.calc_branch_flows(&vm, &va);
    let mut p_sum: Vec<f64> = (0..n).map(|i| model.gs[i] * vm[i] * vm[i]).collect();
    let mut q_sum: Vec<f64> = (0..n).map(|i| -model.bs[i] * vm[i] * vm[i]).collect();
    let mut scale: Vec<f64> = p_sum
        .iter()
        .zip(&q_sum)
        .map(|(a, b)| a.abs().max(b.abs()))
        .collect();
    for e in 0..model.m {
        let (f, t) = (model.branches.from_bus[e], model.branches.to_bus[e]);
        p_sum[f] += flows.pf[e];
        q_sum[f] += flows.qf[e];
        p_sum[t] += flows.pt[e];
        q_sum[t] += flows.qt[e];
        for (bus, values) in [
            (f, [flows.pf[e], flows.qf[e]]),
            (t, [flows.pt[e], flows.qt[e]]),
        ] {
            scale[bus] = values.iter().fold(scale[bus], |acc, v| acc.max(v.abs()));
        }
    }
    let mut worst = 0.0_f64;
    for i in 0..n {
        let tolerance = 1e-9 * scale[i].max(1.0);
        for (what, injected, summed) in [("P", p[i], p_sum[i]), ("Q", q[i], q_sum[i])] {
            let error = (injected - summed).abs();
            assert!(
                error <= tolerance,
                "{name}: bus {i} {what} injection {injected:e} vs branch flows + shunt {summed:e}"
            );
            worst = worst.max(error / scale[i].max(1.0));
        }
    }
    worst
}

/// Check the DC model's index mapping: the incidence, Laplacian, and nodal
/// phase shift the model stores (PowerIO's preparation operators, in dense
/// order) must equal what PowerIO's `DcOperators` reports for the same network
/// in analysis order, once branch columns map through the preparation's
/// analysis rows and buses through `bus_analysis_rows`. `DcOperators` states
/// PowerModels signs, so its bus susceptance matrix is `-L`. Incidence entries
/// are exactly `+-1`; the weighted entries agree to `1e-12` relative.
fn assert_dc_operator_mapping(name: &str, raw: &BalancedNetwork) -> usize {
    let dc = DcNetwork::from_network(raw).expect("dc model");

    let norm = analysis_network(raw);
    let formula = BranchSusceptanceFormula::SeriesSusceptance;
    let opf = DcOpfInstance::from_network(norm.clone())
        .expect("dc opf instance")
        .with_branch_susceptance_formula(formula);
    let assembly = DcOpfAssemblyOptions::default()
        .with_units(Units::PerUnit)
        .with_skip_zero_impedance(false)
        .with_synthesize_unrated_limits(true)
        .with_hvdc_treatment(HvdcTreatment::FixedInjection)
        .with_angle_difference_bounds(AngleDifferenceBounds::PowerModelsPad)
        .with_synthesized_limit_window(SynthesizedLimitWindow::SelectedBounds);
    let prep = build_dc_opf_preparation(&opf, &assembly).expect("dc preparation");
    assert_eq!(prep.bus_analysis_rows, dc.bus_analysis_rows, "{name}");

    // `DcOperators` keeps an in-service branch onto a bus typed isolated,
    // which the preparation (like PowerIO normalization) leaves out with the
    // bus; take such branches out of service for the reference.
    let mut operator_network = norm;
    let isolated: std::collections::BTreeSet<powerio::BusId> = operator_network
        .buses()
        .iter()
        .filter(|bus| bus.kind == powerio::BusType::Isolated)
        .map(|bus| bus.id)
        .collect();
    for branch in operator_network.branches_mut() {
        if isolated.contains(&branch.from) || isolated.contains(&branch.to) {
            branch.in_service = false;
        }
    }
    let pf = DcPfInstance::from_network(operator_network)
        .expect("dc pf instance")
        .with_branch_susceptance_formula(formula);
    let operators = DcOperators::build(&pf).expect("dc operators");
    let a = operators.calc_incidence_matrix(); // m x n, analysis rows/cols

    // The index maps: operator row -> analysis branch row -> dense branch, and
    // analysis bus row -> dense bus.
    let dense_branch: BTreeMap<usize, usize> = prep
        .branches
        .analysis_rows
        .iter()
        .enumerate()
        .map(|(dense, &row)| (row, dense))
        .collect();
    let mut dense_bus = vec![None; a.cols()];
    for (dense, &row) in dc.bus_analysis_rows.iter().enumerate() {
        dense_bus[row] = Some(dense);
    }
    let bus =
        |row: usize| dense_bus[row].unwrap_or_else(|| panic!("{name}: bus row {row} dropped"));
    assert_eq!(
        operators.branch_rows().len(),
        dc.m,
        "{name}: branch column count"
    );
    let mut reference: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for (value, (k, b)) in a.iter() {
        let row = operators.branch_rows()[k];
        let e = *dense_branch
            .get(&row)
            .unwrap_or_else(|| panic!("{name}: operator branch row {row} has no model column"));
        reference.insert((bus(b), e), *value);
    }
    let stored: BTreeMap<(usize, usize), f64> = (0..dc.n)
        .flat_map(|i| crate::model::csr_row(&dc.incidence, i).map(move |(e, v)| ((i, e), v)))
        .collect();
    assert_eq!(stored, reference, "{name}: incidence mapping");

    let close = |what: &str, value: f64, expected: f64| {
        let scale = value.abs().max(expected.abs());
        assert!(
            (value - expected).abs() <= 1e-12 * scale,
            "{name}: {what} model {value:e} vs DcOperators {expected:e}"
        );
    };
    let susceptance = operators.calc_bus_susceptance_matrix();
    let mut expected: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for (value, (r, c)) in susceptance.iter() {
        if *value != 0.0 {
            expected.insert((bus(r), bus(c)), -value);
        }
    }
    let laplacian: BTreeMap<(usize, usize), f64> = (0..dc.n)
        .flat_map(|i| crate::model::csr_row(&dc.laplacian, i).map(move |(j, v)| ((i, j), v)))
        .filter(|(_, v)| *v != 0.0)
        .collect();
    assert_eq!(
        laplacian.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "{name}: Laplacian pattern"
    );
    for ((key, value), want) in laplacian.iter().zip(expected.values()) {
        close(&format!("L{key:?}"), *value, *want);
    }
    // `DcOperators` keeps a de-energized bus on its axis, typed isolated
    // with nothing connected; the preparation leaves it out.
    let phase = operators.calc_bus_phase_shift_injection();
    for (row, want) in phase.iter().enumerate() {
        match dense_bus[row] {
            Some(dense) => close("p_shift", dc.prep.p_shift[dense], *want),
            None => assert_eq!(*want, 0.0, "{name}: dropped bus row {row} shifts phase"),
        }
    }
    stored.len()
}

/// Hand-built stress case: non-contiguous bus numbers, bus shunts, off-nominal
/// taps with and without phase shift, an out-of-service branch, a parallel
/// pair, a declared isolated bus, and an unsupplied island that is dropped.
/// The isolated bus 99 and the island bus 50 sit in the middle of the bus
/// table, so a preparation over the unnormalized network (the SOCWR path)
/// carries buses through a map that is neither the identity nor a prefix.
const STRESS: &str = "\
function mpc = stress
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 10 3 0   0  0   0   1 1    0    230 1 1.1 0.9;
 99 4 0   0  0   0   1 1    0    115 1 1.1 0.9;
 20 1 90  30 4.5 19  1 1    0    230 1 1.1 0.9;
 50 1 15  5  1   3   1 1    0    115 1 1.1 0.9;
 35 2 0   0  0   -12 1 1    0    230 1 1.1 0.9;
 7  1 40  10 0   0   1 1    0    115 1 1.1 0.9;
 51 1 5   1  0   0   1 1    0    115 1 1.1 0.9;
];
mpc.gen = [
 10 0  0 300 -300 1.02 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 35 60 0 300 -300 1.01 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 10 20 0.01  0.1   0.02 250 250 250 0     0    1 -360 360;
 10 35 0.012 0.09  0.03 250 250 250 0     0    1 -360 360;
 20 35 0.01  0.1   0.01 250 250 250 0     0    1 -360 360;
 20 35 0.02  0.11  0.01 250 250 250 0     0    1 -360 360;
 35 7  0.002 0.05  0    250 250 250 0.975 0    1 -360 360;
 20 7  0.001 0.04  0    250 250 250 1.05  -7.5 1 -360 360;
 10 7  0.003 0.06  0.01 250 250 250 1.02  12   0 -360 360;
 7  99 0.01  0.1   0    250 250 250 0     0    1 -360 360;
 50 51 0.01  0.1   0    250 250 250 0     0    1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 1.2 0;
];
";

fn embedded_cases() -> Vec<(&'static str, String)> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut cases = vec![
        ("CASE3", super::CASE3.to_owned()),
        ("CASE3_TWIN", super::CASE3_TWIN.to_owned()),
        ("CASE_ISLANDS", super::CASE_ISLANDS.to_owned()),
        ("CASE9", super::CASE9.to_owned()),
        ("STRESS", STRESS.to_owned()),
    ];
    for (name, path) in [
        (
            "pglib_opf_case200_activ",
            "crates/tellegen-server/fixtures/pglib/pglib_opf_case200_activ.m",
        ),
        (
            "pglib_opf_case500_goc",
            "crates/tellegen-server/fixtures/pglib/pglib_opf_case500_goc.m",
        ),
        ("europe3809", "apps/web/static/examples/europe3809.m"),
        ("docs_case3", "docs/evidence/studies/case3.m"),
    ] {
        let text = std::fs::read_to_string(format!("{root}/{path}"))
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        cases.push((name, text));
    }
    cases
}

#[test]
fn ac_operators_map_to_the_dense_order_on_the_repository_cases() {
    for (name, text) in embedded_cases() {
        let raw = super::parse_matpower(&text).expect("parse");
        let model = AcNetwork::from_network_pf(&raw).expect("ac model");
        let worst = assert_ac_mapping(name, &model);
        eprintln!("ac mapping {name}: worst relative mismatch {worst:e}");
    }
}

/// The SOCWR workspace reads a typed instance without normalizing it: the
/// unsupplied island and the declared isolated bus stay in the network, and
/// the preparation leaves them out. STRESS places them mid-table, so the
/// preparation carries analysis rows `[0, 2, 4, 5]`, a map that is not a
/// prefix. Its admittance matrix must equal the power flow model's, which
/// PowerIO prepares from the same energized network; entries agree to `1e-12`
/// relative.
#[cfg(feature = "conic")]
#[test]
fn ybus_maps_buses_dropped_mid_table_to_the_dense_columns() {
    let raw = super::parse_matpower(STRESS).expect("parse");
    let network = energize(&raw, ReferencePolicy::Stated)
        .network(&raw)
        .clone();
    let instance = powerio_prob::AcOpfInstance::from_network(network).expect("instance");
    let mapped = AcNetwork::from_instance(&instance).expect("SOCWR model");
    let model = AcNetwork::from_network_pf(&raw).expect("ac model");
    assert_eq!(model.bus_ids, vec![10, 20, 35, 7]);
    assert_eq!(mapped.bus_ids, model.bus_ids);
    assert_ac_mapping("STRESS instance", &mapped);
    let (mapped, reference) = (ybus_entries(&mapped), ybus_entries(&model));
    assert_eq!(
        mapped.keys().collect::<Vec<_>>(),
        reference.keys().collect::<Vec<_>>(),
        "sparsity pattern"
    );
    for (((r, c), value), expected) in mapped.iter().zip(reference.values()) {
        let scale = value.norm().max(expected.norm());
        assert!(
            (value - expected).norm() <= 1e-12 * scale,
            "({r}, {c}): mapped {value:e} vs power flow {expected:e}"
        );
    }
}

/// A 3-bus network whose admittance matrix is written out by hand: a lossless
/// line with charging in parallel with a lossy line (1-2), a phase-shifting
/// transformer with an off-nominal tap (2-3), an out-of-service line (1-3), and
/// a bus shunt (bus 3), with bus numbers that are not dense.
#[test]
fn ybus_matches_a_hand_written_three_bus_matrix() {
    let text = "\
function mpc = analytic3
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 4 3 0  0  0 0  1 1 0 230 1 1.1 0.9;
 7 1 50 10 0 0  1 1 0 230 1 1.1 0.9;
 9 1 30 5  5 10 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 4 0 0 300 -300 1 100 1 250 0 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 4 7 0    0.1  0.02 250 250 250 0 0  1 -360 360;
 4 7 0.03 0.04 0    250 250 250 0 0  1 -360 360;
 7 9 0    0.25 0    250 250 250 2 30 1 -360 360;
 4 9 0.01 0.1  0    250 250 250 0 0  0 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.1 1 0;
];
";
    let raw = super::parse_matpower(text).expect("parse");
    let model = AcNetwork::from_network_pf(&raw).expect("ac model");
    assert_eq!(model.bus_ids, vec![4, 7, 9]);
    let c = Complex64::new;
    // Line 4-7: y = 1/(0.1j) = -10j, charging 0.01j per end. Line 4-7 (lossy):
    // y = 1/(0.03 + 0.04j) = 12 - 16j. Transformer 7-9: y = 1/(0.25j) = -4j,
    // t = 2 e^{j pi/6}: Yff = y/4 = -j, Ytt = -4j, Yft = -y/conj(t) = 2j e^{j pi/6},
    // Ytf = -y/t = 2j e^{-j pi/6}. Bus 9 shunt (5 + 10j) MVA / 100.
    let shift = std::f64::consts::FRAC_PI_6;
    let expected = [
        ((0, 0), c(12.0, -10.0 + 0.01 - 16.0)),
        ((0, 1), c(-12.0, 10.0 + 16.0)),
        ((1, 0), c(-12.0, 10.0 + 16.0)),
        ((1, 1), c(12.0, -10.0 + 0.01 - 16.0 - 1.0)),
        ((1, 2), c(0.0, 2.0) * Complex64::from_polar(1.0, shift)),
        ((2, 1), c(0.0, 2.0) * Complex64::from_polar(1.0, -shift)),
        ((2, 2), c(0.05, -4.0 + 0.1)),
    ];
    let ybus = ybus_entries(&model);
    assert_eq!(
        ybus.keys().copied().collect::<Vec<_>>(),
        expected.iter().map(|&(rc, _)| rc).collect::<Vec<_>>(),
        "sparsity pattern"
    );
    for ((&(r, c), value), &(_, want)) in ybus.iter().zip(&expected) {
        assert!(
            (value - want).norm() <= 1e-12 * want.norm(),
            "({r}, {c}): {value} != {want}"
        );
    }
    assert_ac_mapping("analytic3", &model);
}

/// A near-ideal jumper keeps its series admittance in Y_bus but not its terminal
/// charging, and the branch flows read the same edited columns.
#[test]
fn ybus_drops_jumper_charging_like_the_pi_columns() {
    let text = super::CASE3.replace(
        "1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
        "1 3 1e-7 1e-6 0.5 250 250 250 0 0 1 -360 360;",
    );
    let raw = super::parse_matpower(&text).expect("parse");
    let model = AcNetwork::from_network_pf(&raw).expect("ac model");
    assert_eq!((model.branches.b_fr[1], model.branches.b_to[1]), (0.0, 0.0));
    // Bus 1's diagonal carries no jumper charging: the 1-2 line's series and
    // the jumper's series susceptance only.
    let ybus = ybus_entries(&model);
    let series: f64 = model.branches.b[0] + model.branches.b[1];
    assert!((ybus[&(0, 0)].im - series).abs() <= 1e-9 * series.abs());
    assert_ac_mapping("jumper", &model);
}

#[test]
fn dc_operators_map_to_powerio_on_the_repository_cases() {
    for (name, text) in embedded_cases() {
        let raw = super::parse_matpower(&text).expect("parse");
        let nnz = assert_dc_operator_mapping(name, &raw);
        eprintln!("dc operator mapping {name}: {nnz} incidence entries");
    }
}

/// The full PGLib corpus at `$PGLIB_OPF_PATH` (default `~/Datasets/pglib-opf`).
/// Ignored by default: the corpus is not vendored. Run with
/// `cargo test --release -p tellegen powerio_parity -- --ignored --nocapture`.
#[test]
#[ignore = "needs the PGLib corpus on disk"]
fn operator_mapping_on_the_pglib_corpus() {
    let root = std::env::var("PGLIB_OPF_PATH").unwrap_or_else(|_| {
        format!(
            "{}/Datasets/pglib-opf",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let Ok(dir) = std::fs::read_dir(&root) else {
        eprintln!("no PGLib corpus at {root}; skipping");
        return;
    };
    let mut files: Vec<_> = dir
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "m"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no cases under {root}");
    for path in files {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("read case");
        let raw = super::parse_matpower(&text).expect("parse");
        let model = AcNetwork::from_network_pf(&raw).expect("ac model");
        let worst = assert_ac_mapping(&name, &model);
        let nnz = assert_dc_operator_mapping(&name, &raw);
        eprintln!("mapping {name}: ac worst {worst:e}; incidence {nnz} entries");
        #[cfg(feature = "conic")]
        {
            let instance = powerio_prob::AcOpfInstance::from_network(raw).expect("instance");
            match AcNetwork::from_instance(&instance) {
                Ok(model) => {
                    let worst = assert_ac_mapping(&name, &model);
                    eprintln!("instance mapping {name}: ac worst {worst:e}");
                }
                Err(error) => eprintln!("{name}: no SOCWR workspace ({error})"),
            }
        }
    }
}
