//! Cross-checks of the two assemblies issue #102 moved onto PowerIO.
//!
//! The AC model's bus admittance matrix is PowerIO's MATPOWER `makeYbus`
//! (`calc_admittance_matrix`). The model's own pi columns (`g`, `b`, the
//! terminal charging, `tap`, `shift`, `gs`, `bs`), which come from PowerIO's AC
//! preparation and which the branch flows and flow sensitivities read through
//! [`AcNetwork::branch_admittance`], must scatter to the same matrix. The
//! reference below is that scatter, retained from Tellegen's former local
//! assembly. It sums each diagonal in PowerIO's order (branches in table order,
//! then the bus shunt), so on the power flow's normalized network the two agree
//! bitwise everywhere on the whole PGLib corpus; summing the shunt first, as the
//! former assembly did, moved diagonals by up to 17 ulps (relative 6.2e-16) and
//! nothing else. The tolerance is the issue's `1e-12` relative, and off-diagonal
//! entries must agree bitwise.
//!
//! The DC KKT keeps its per-bus incidence lists ([`crate::sens`]'s
//! `incidence_by_bus`, a transpose of PowerIO's preparation endpoint columns):
//! `DcOperators::calc_incidence_matrix` returns the same entries, checked here,
//! but builds the full DC operator set to get them, about 27x the cost.
//! Tellegen's models index dense columns: the preparation's active buses
//! (`bus_analysis_rows`) and active branches (`branches.analysis_rows`). Those
//! two row maps are the only index translation the comparisons need.

use std::collections::BTreeMap;

use num_complex::Complex64;
use powerio::BalancedNetwork;
use powerio_matrix::{build_dc_opf_preparation, DcOperators, DcOpfAssemblyOptions, Units};
use powerio_prob::{DcOpfInstance, DcPfInstance};
use powerio_tx::{BranchSusceptanceFormula, HvdcTreatment};

use super::islands::{energize, ReferencePolicy};
use super::{normalize_angle_bounds, normalize_for_model, AcNetwork, DcNetwork, HvdcLines};

/// The network each model reads after energizing and normalizing, built the
/// way [`DcNetwork::from_network`] builds it.
fn analysis_network(raw: &BalancedNetwork, hvdc: HvdcLines) -> BalancedNetwork {
    let energized = energize(raw, ReferencePolicy::GeneratorHosted);
    normalize_for_model(energized.network(raw), hvdc)
        .expect("normalize")
        .network
}

/// The bus admittance matrix scattered from the model's own pi columns: every
/// branch's [`AcNetwork::branch_admittance`] stamp in branch order, then the bus
/// shunt on the diagonal, with a diagonal entry for every bus.
fn ybus_from_columns(model: &AcNetwork) -> BTreeMap<(usize, usize), Complex64> {
    let mut acc: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for i in 0..model.n {
        acc.insert((i, i), Complex64::new(0.0, 0.0));
    }
    for e in 0..model.m {
        let (yff, yft, ytf, ytt) = model.branch_admittance(e);
        let (f, t) = (model.br_from[e], model.br_to[e]);
        *acc.entry((f, f)).or_default() += yff;
        *acc.entry((f, t)).or_default() += yft;
        *acc.entry((t, f)).or_default() += ytf;
        *acc.entry((t, t)).or_default() += ytt;
    }
    for i in 0..model.n {
        *acc.get_mut(&(i, i)).expect("diagonal") += Complex64::new(model.gs[i], model.bs[i]);
    }
    acc
}

/// Distance in units in the last place between two finite doubles of the
/// same sign (or both zero); `u64::MAX` across a sign change.
fn ulps(a: f64, b: f64) -> u64 {
    if a == b {
        return 0;
    }
    if a.is_sign_negative() != b.is_sign_negative() {
        return u64::MAX;
    }
    a.to_bits().abs_diff(b.to_bits())
}

#[derive(Debug, Default)]
struct YbusParity {
    entries: usize,
    off_diagonal: usize,
    bitwise_off_diagonal: usize,
    diagonal: usize,
    bitwise_diagonal: usize,
    max_ulps: u64,
    max_relative: f64,
}

/// Compare the model's PowerIO-assembled `ybus()` with the scatter of its pi
/// columns, entry by entry: the same sparsity pattern, off-diagonal entries
/// bitwise equal, every entry within `1e-12` relative.
fn assert_ybus_parity(name: &str, raw: &BalancedNetwork) -> YbusParity {
    let model = AcNetwork::from_network_pf(raw).expect("ac model");
    assert_model_ybus_parity(name, &model)
}

fn assert_model_ybus_parity(name: &str, model: &AcNetwork) -> YbusParity {
    let assembled = model.ybus();
    let reference = ybus_from_columns(model);
    assert_eq!(
        assembled.len(),
        reference.len(),
        "{name}: sparsity pattern size"
    );
    let mut stats = YbusParity::default();
    for (&(r, c, value), (&(rr, rc), &expected)) in assembled.iter().zip(&reference) {
        assert_eq!((r, c), (rr, rc), "{name}: sparsity pattern or order");
        stats.entries += 1;
        let bitwise = value.re.to_bits() == expected.re.to_bits()
            && value.im.to_bits() == expected.im.to_bits();
        if r == c {
            stats.diagonal += 1;
            stats.bitwise_diagonal += usize::from(bitwise);
        } else {
            stats.off_diagonal += 1;
            stats.bitwise_off_diagonal += usize::from(bitwise);
            assert!(
                bitwise,
                "{name}: off-diagonal ({r}, {c}) PowerIO {value:e} != columns {expected:e}"
            );
        }
        stats.max_ulps = stats
            .max_ulps
            .max(ulps(value.re, expected.re))
            .max(ulps(value.im, expected.im));
        let scale = value.norm().max(expected.norm());
        if scale > 0.0 {
            let relative = (value - expected).norm() / scale;
            stats.max_relative = stats.max_relative.max(relative);
            assert!(
                relative <= 1e-12,
                "{name}: ({r}, {c}) PowerIO {value:e} vs columns {expected:e}, relative {relative:e}"
            );
        }
    }
    stats
}

/// Compare the DC KKT's per-bus incidence lists with PowerIO's incidence
/// matrix. Branch columns map through the preparation's analysis rows (the
/// rows `DcOperators::branch_rows` also reports), bus rows through
/// `bus_analysis_rows`. Incidence entries are exactly `+-1`, so the
/// comparison is exact.
fn assert_incidence_parity(name: &str, raw: &BalancedNetwork) -> usize {
    let dc = DcNetwork::from_network(raw).expect("dc model");
    let local = crate::sens::incidence_by_bus_for_parity(&dc);

    let mut norm = analysis_network(raw, HvdcLines::FixedInjection);
    for branch in norm.branches_mut() {
        (branch.angmin, branch.angmax) = normalize_angle_bounds(branch.angmin, branch.angmax);
    }
    let formula = BranchSusceptanceFormula::SeriesSusceptance;
    let opf = DcOpfInstance::from_network(norm.clone())
        .expect("dc opf instance")
        .with_branch_susceptance_formula(formula);
    let mut assembly = DcOpfAssemblyOptions::default();
    assembly.units = Units::PerUnit;
    assembly.skip_zero_impedance = false;
    assembly.synthesize_unrated_limits = true;
    assembly.hvdc_treatment = HvdcTreatment::FixedInjection;
    let prep = build_dc_opf_preparation(&opf, &assembly).expect("dc preparation");
    assert_eq!(prep.bus_analysis_rows, dc.bus_analysis_rows, "{name}");

    let pf = DcPfInstance::from_network(norm)
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
    assert_eq!(
        operators.branch_rows().len(),
        dc.m,
        "{name}: branch column count"
    );
    let mut reference: Vec<Vec<(usize, f64)>> = vec![Vec::new(); dc.n];
    for (value, (k, bus)) in a.iter() {
        let row = operators.branch_rows()[k];
        let e = *dense_branch
            .get(&row)
            .unwrap_or_else(|| panic!("{name}: operator branch row {row} has no model column"));
        let b = dense_bus[bus].unwrap_or_else(|| panic!("{name}: bus row {bus} dropped"));
        reference[b].push((e, *value));
    }
    for list in &mut reference {
        list.sort_by_key(|&(e, _)| e);
    }
    let mut sorted_local = local.clone();
    for list in &mut sorted_local {
        list.sort_by_key(|&(e, _)| e);
    }
    assert_eq!(sorted_local, reference, "{name}: incidence differs");
    reference.iter().map(Vec::len).sum()
}

/// Hand-built stress case: non-contiguous bus numbers, bus shunts, off-nominal
/// taps with and without phase shift, an out-of-service branch, a parallel
/// pair, a declared isolated bus, and an unsupplied island that is dropped.
const STRESS: &str = "\
function mpc = stress
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 10 3 0   0  0   0   1 1    0    230 1 1.1 0.9;
 20 1 90  30 4.5 19  1 1    0    230 1 1.1 0.9;
 35 2 0   0  0   -12 1 1    0    230 1 1.1 0.9;
 7  1 40  10 0   0   1 1    0    115 1 1.1 0.9;
 99 4 0   0  0   0   1 1    0    115 1 1.1 0.9;
 50 1 15  5  1   3   1 1    0    115 1 1.1 0.9;
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
fn powerio_ybus_matches_the_pi_columns_on_the_repository_cases() {
    for (name, text) in embedded_cases() {
        let raw = super::parse_matpower(&text).expect("parse");
        let stats = assert_ybus_parity(name, &raw);
        assert_eq!(stats.bitwise_diagonal, stats.diagonal, "{name}: {stats:?}");
        eprintln!("ybus parity {name}: {stats:?}");
    }
}

/// The SOCWR workspace reads a typed instance without normalizing it: the
/// unsupplied island and the declared isolated bus stay in the network, with
/// their branches in service, and the preparation leaves them out. Y_bus must
/// leave them out too. On this raw (MW) network the preparation scales a bus
/// shunt by `* (1 / baseMVA)` and `makeYbus` by `/ baseMVA`, so a diagonal
/// with a shunt may differ by an ulp; off-diagonal entries stay bitwise.
#[cfg(feature = "conic")]
#[test]
fn instance_ybus_matches_the_pi_columns_on_the_repository_cases() {
    for (name, text) in embedded_cases() {
        let raw = super::parse_matpower(&text).expect("parse");
        let instance = powerio_prob::AcOpfInstance::from_network(raw).expect("instance");
        let model = AcNetwork::from_instance(&instance).expect("ac model");
        assert_model_ybus_parity(name, &model);
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
    let ybus = model.ybus();
    assert_eq!(
        ybus.iter().map(|&(r, c, _)| (r, c)).collect::<Vec<_>>(),
        expected.iter().map(|&(rc, _)| rc).collect::<Vec<_>>(),
        "sparsity pattern"
    );
    for (&(r, c, value), &(_, want)) in ybus.iter().zip(&expected) {
        assert!(
            (value - want).norm() <= 1e-12 * want.norm(),
            "({r}, {c}): {value} != {want}"
        );
    }
    assert_ybus_parity("analytic3", &raw);
}

/// A near-ideal jumper keeps its series admittance in Y_bus but not its terminal
/// charging, matching the pi columns the branch flows read.
#[test]
fn ybus_drops_jumper_charging_like_the_pi_columns() {
    let text = super::CASE3.replace(
        "1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
        "1 3 1e-7 1e-6 0.5 250 250 250 0 0 1 -360 360;",
    );
    let raw = super::parse_matpower(&text).expect("parse");
    let model = AcNetwork::from_network_pf(&raw).expect("ac model");
    assert_eq!((model.b_fr[1], model.b_to[1]), (0.0, 0.0));
    let stats = assert_ybus_parity("jumper", &raw);
    assert_eq!(stats.bitwise_diagonal, stats.diagonal, "{stats:?}");
}

#[test]
fn incidence_matches_powerio_on_the_repository_cases() {
    for (name, text) in embedded_cases() {
        let raw = super::parse_matpower(&text).expect("parse");
        let nnz = assert_incidence_parity(name, &raw);
        eprintln!("incidence parity {name}: {nnz} entries");
    }
}

/// The full PGLib corpus at `$PGLIB_OPF_PATH` (default `~/Datasets/pglib-opf`).
/// Ignored by default: the corpus is not vendored. Run with
/// `cargo test --release -p tellegen powerio_parity -- --ignored --nocapture`.
#[test]
#[ignore = "needs the PGLib corpus on disk"]
fn ybus_and_incidence_parity_on_the_pglib_corpus() {
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
        let stats = assert_ybus_parity(&name, &raw);
        assert_eq!(stats.bitwise_diagonal, stats.diagonal, "{name}: {stats:?}");
        let nnz = assert_incidence_parity(&name, &raw);
        eprintln!("parity {name}: {stats:?}; incidence {nnz} entries");
        #[cfg(feature = "conic")]
        {
            let instance = powerio_prob::AcOpfInstance::from_network(raw).expect("instance");
            match AcNetwork::from_instance(&instance) {
                Ok(model) => {
                    let stats = assert_model_ybus_parity(&name, &model);
                    eprintln!("instance parity {name}: {stats:?}");
                }
                Err(error) => eprintln!("{name}: no SOCWR workspace ({error})"),
            }
        }
    }
}
