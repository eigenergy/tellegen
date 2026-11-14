//! Parity of the AC model's local bus admittance assembly and the DC KKT's
//! per-bus incidence with PowerIO's own assemblies (issue #102).
//!
//! PowerIO's `calc_admittance_matrix` is MATPOWER's `makeYbus` over an
//! [`IndexedNetwork`] (every bus of the star lowered view), and
//! `DcOperators::calc_incidence_matrix` is the signed branch by bus incidence
//! over the accepted in-service branches. Tellegen's models index dense
//! columns: the preparation's active buses (`bus_analysis_rows`) and its
//! active branches (`branches.analysis_rows`). Those two row maps are the only
//! index translation the comparison needs.
//!
//! The sparsity pattern must match exactly and every off-diagonal entry
//! bitwise. Diagonals carry a bus shunt, which the local assembly adds before
//! the branch stamps and `makeYbus` after them; that summation order moves a
//! diagonal by a few ulps, so diagonals are held to `1e-12` relative.

use std::collections::BTreeMap;

use num_complex::Complex64;
use powerio::BalancedNetwork;
use powerio_matrix::{
    build_ac_opf_preparation, build_dc_opf_preparation, calc_admittance_matrix,
    AcOpfAssemblyOptions, BuildOptions, DcOperators, DcOpfAssemblyOptions, Units,
};
use powerio_prob::{AcOpfInstance, DcOpfInstance, DcPfInstance};
use powerio_tx::{BranchSusceptanceFormula, HvdcTreatment, IndexedNetwork};

use super::islands::{energize, ReferencePolicy};
use super::{normalize_angle_bounds, normalize_for_model, AcNetwork, DcNetwork, HvdcLines};

/// The network each model reads after energizing and normalizing, built the
/// way [`AcNetwork::from_network_pf`] and [`DcNetwork::from_network`] build it.
fn analysis_network(raw: &BalancedNetwork, hvdc: HvdcLines) -> BalancedNetwork {
    let energized = energize(raw, ReferencePolicy::GeneratorHosted);
    normalize_for_model(energized.network(raw), hvdc)
        .expect("normalize")
        .network
}

/// PowerIO's `makeYbus` on the AC model's analysis network, translated to the
/// model's dense bus columns: `(row, col) -> G + jB`, and the number of
/// entries PowerIO stamped on buses the model does not carry.
fn powerio_ybus(raw: &BalancedNetwork) -> (BTreeMap<(usize, usize), Complex64>, usize) {
    let mut norm = analysis_network(raw, HvdcLines::Reject);
    // Tellegen's one modelling departure from `makeYbus`: a near-ideal jumper
    // keeps its series admittance but loses its terminal charging. State it
    // on PowerIO's input so the two assemblies see the same network.
    for (row, branch) in norm.branches_mut().iter_mut().enumerate() {
        if let Ok(Some((g, b))) = branch.calc_series_admittance(row) {
            if super::ac::suppresses_terminal_charging(g, b) {
                branch.b = 0.0;
                branch.charging = None;
            }
        }
    }
    let instance = AcOpfInstance::from_network(norm.clone()).expect("ac instance");
    let mut assembly = AcOpfAssemblyOptions::default();
    assembly.units = Units::PerUnit;
    assembly.skip_zero_impedance = false;
    let prep = build_ac_opf_preparation(&instance, &assembly).expect("ac preparation");

    let view = IndexedNetwork::new(&norm);
    let parts = calc_admittance_matrix(&view, &BuildOptions::default()).expect("ybus");
    // The one index map: analysis row -> dense column.
    let mut dense_of = vec![None; view.n()];
    for (dense, &row) in prep.bus_analysis_rows.iter().enumerate() {
        dense_of[row] = Some(dense);
    }
    let mut out: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    let mut unmapped = 0;
    for (matrix, imaginary) in [(&parts.g, false), (&parts.b, true)] {
        for (value, (r, c)) in matrix.iter() {
            let (Some(r), Some(c)) = (dense_of[r], dense_of[c]) else {
                unmapped += 1;
                continue;
            };
            let entry = out.entry((r, c)).or_default();
            if imaginary {
                entry.im = *value;
            } else {
                entry.re = *value;
            }
        }
    }
    (out, unmapped)
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
    /// Entries Tellegen stores that PowerIO prunes because they are exactly zero.
    tellegen_structural_zeros: usize,
    off_diagonal: usize,
    bitwise_off_diagonal: usize,
    diagonal: usize,
    bitwise_diagonal: usize,
    max_ulps: u64,
    max_relative: f64,
}

/// Compare the AC model's `ybus()` with PowerIO's, entry by entry. Panics on
/// a pattern mismatch, on an off-diagonal entry that is not bitwise equal, or
/// on a relative difference above `1e-12`.
fn assert_ybus_parity(name: &str, raw: &BalancedNetwork) -> YbusParity {
    let model = AcNetwork::from_network_pf(raw).expect("ac model");
    let local: BTreeMap<(usize, usize), Complex64> = model
        .ybus()
        .into_iter()
        .map(|(r, c, v)| ((r, c), v))
        .collect();
    let (reference, unmapped) = powerio_ybus(raw);
    assert_eq!(
        unmapped, 0,
        "{name}: PowerIO stamped entries on buses the model drops"
    );

    let mut stats = YbusParity::default();
    for (&(r, c), &value) in &local {
        let Some(&expected) = reference.get(&(r, c)) else {
            assert_eq!(
                value,
                Complex64::new(0.0, 0.0),
                "{name}: ({r}, {c}) = {value} only in the local Ybus"
            );
            stats.tellegen_structural_zeros += 1;
            continue;
        };
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
                "{name}: off-diagonal ({r}, {c}) local {value:e} != PowerIO {expected:e}"
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
                "{name}: ({r}, {c}) local {value:e} vs PowerIO {expected:e}, relative {relative:e}"
            );
        }
    }
    for key in reference.keys() {
        assert!(
            local.contains_key(key),
            "{name}: {key:?} only in PowerIO's Ybus"
        );
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
fn ybus_matches_powerio_on_the_repository_cases() {
    for (name, text) in embedded_cases() {
        let raw = super::parse_matpower(&text).expect("parse");
        let stats = assert_ybus_parity(name, &raw);
        eprintln!("ybus parity {name}: {stats:?}");
    }
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
        let nnz = assert_incidence_parity(&name, &raw);
        eprintln!("parity {name}: {stats:?}; incidence {nnz} entries");
    }
}
