//! Opt-in export of the actual prepared reduced matrix for external KLU checks.
use super::*;
use powerio_dist::{
    Configuration, DistBus, DistLine, DistLineCode, DistLoad, DistLoadVoltageModel, VoltageSource,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

// Apple SDK malloc.h defines this process-wide allocator snapshot. This
// opt-in test runs alone; the delta around factorization includes LU and workspace.
#[cfg(target_os = "macos")]
fn heap_bytes() -> Option<usize> {
    #[repr(C)]
    #[derive(Default)]
    struct Statistics {
        blocks: u32,
        used: usize,
        maximum: usize,
        allocated: usize,
    }
    unsafe extern "C" {
        fn malloc_zone_statistics(zone: *mut std::ffi::c_void, statistics: *mut Statistics);
    }
    let mut statistics = Statistics::default();
    // SAFETY: null selects all zones, and Statistics matches the SDK C layout.
    unsafe {
        malloc_zone_statistics(std::ptr::null_mut(), &mut statistics);
    }
    Some(statistics.used)
}
#[cfg(not(target_os = "macos"))]
fn heap_bytes() -> Option<usize> {
    None
}

#[derive(Serialize, Deserialize)]
struct Input {
    terminals: Vec<(String, String)>,
    entries: Vec<(usize, usize, [f64; 2])>,
    rhs: Vec<[f64; 2]>,
    #[serde(default)]
    initial_solution: Option<Vec<[f64; 2]>>,
}

fn prepared_input(sections: usize) -> Input {
    let mut network = MulticonductorNetwork::new();
    let terminals = vec!["1".to_owned(), "4".to_owned()];
    for i in 0..=sections {
        let mut bus = DistBus::new(format!("b{i}"), terminals.clone());
        if i == 0 {
            bus.grounded.push("4".into());
        }
        network.buses_mut().push(bus);
    }
    network.line_codes_mut().push(DistLineCode::new(
        "c",
        vec![vec![0.1, 0.02], vec![0.02, 0.12]],
        vec![vec![0.04, 0.01], vec![0.01, 0.05]],
    ));
    network.sources_mut().push(VoltageSource::new(
        "source",
        "b0",
        terminals.clone(),
        vec![230.0, 0.0],
        vec![0.0, 0.0],
    ));
    for i in 1..=sections {
        // A binary tree avoids conflating a long feeder's voltage collapse with LU error.
        network.lines_mut().push(DistLine::new(
            format!("l{i}"),
            format!("b{}", (i - 1) / 2),
            format!("b{i}"),
            terminals.clone(),
            terminals.clone(),
            "c",
            1.0,
        ));
        let mut load = DistLoad::new(
            format!("d{i}"),
            format!("b{i}"),
            terminals.clone(),
            Configuration::Wye,
            vec![1.0],
            vec![0.2],
        );
        load.voltage_model = DistLoadVoltageModel::ConstantPower { v_nom: vec![230.0] };
        network.loads_mut().push(load);
    }
    let instance = McAcPfInstance::from_network(network).unwrap();
    instance_input(&instance)
}

fn instance_input(instance: &McAcPfInstance) -> Input {
    let prepared = PreparedNetwork::prepare(instance).unwrap();
    let mut entries = BTreeMap::<(usize, usize), Complex64>::new();
    for (row, &global) in prepared.unknown.iter().enumerate() {
        for matrix in [&prepared.passive, &prepared.yref] {
            for (column, value) in matrix.row(global) {
                let column = prepared.unknown_pos[column];
                if column != usize::MAX {
                    *entries.entry((row, column)).or_default() += value;
                }
            }
        }
    }
    // Check that the exported coefficients reproduce the factor actually retained
    // by preparation, including its fixed-terminal and compensation handling.
    let triplets: Vec<_> = entries
        .iter()
        .filter(|(_, v)| v.norm() > 0.0)
        .map(|(&(r, c), &v)| (r, c, v))
        .collect();
    let rebuilt = RetainedComplexLu::factor(prepared.unknown.len(), &triplets).unwrap();
    let mut original = vec![Complex64::new(0.0, 0.0); prepared.unknown.len()];
    let mut exported = original.clone();
    prepared
        .factor
        .as_ref()
        .unwrap()
        .solve_into(&prepared.source_rhs, &mut original)
        .unwrap();
    rebuilt
        .solve_into(&prepared.source_rhs, &mut exported)
        .unwrap();
    assert!(original
        .iter()
        .zip(&exported)
        .all(|(a, b)| (*a - *b).norm() < 1e-10));
    Input {
        initial_solution: None,
        terminals: prepared
            .unknown
            .iter()
            .map(|&i| prepared.index.terminal_ids()[i].clone())
            .collect(),
        entries: entries
            .into_iter()
            .filter(|(_, value)| value.norm() > 0.0)
            .map(|((row, col), value)| (row, col, [value.re, value.im]))
            .collect(),
        rhs: prepared
            .source_rhs
            .iter()
            .map(|value| [value.re, value.im])
            .collect(),
    }
}

#[test]
#[ignore = "writes matrix evidence; set TELLEGEN_LINEAR_OUTPUT and optional TELLEGEN_LINEAR_INPUT or TELLEGEN_LINEAR_SECTIONS"]
fn export_comparison() {
    let input: Input = if let Ok(path) = std::env::var("TELLEGEN_LINEAR_INPUT") {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    } else if let Ok(path) = std::env::var("TELLEGEN_LINEAR_DSS") {
        let module = powerio::parse(powerio::Source::open(path).unwrap()).unwrap();
        super::super::input::validate_mc_module(&module).unwrap();
        let powerio::PioValue::MulticonductorNetwork(network) = module.into_value() else {
            panic!("expected a multiconductor network");
        };
        instance_input(&McAcPfInstance::from_network(network).unwrap())
    } else {
        prepared_input(
            std::env::var("TELLEGEN_LINEAR_SECTIONS").map_or(1000, |s| s.parse().unwrap()),
        )
    };
    let entries: Vec<_> = input
        .entries
        .iter()
        .map(|&(r, c, [re, im])| (r, c, Complex64::new(re, im)))
        .collect();
    let rhs: Vec<_> = input
        .rhs
        .iter()
        .map(|&[re, im]| Complex64::new(re, im))
        .collect();
    let heap_before = heap_bytes();
    let start = Instant::now();
    let factor = RetainedComplexLu::factor(rhs.len(), &entries).unwrap();
    let factor_seconds = start.elapsed().as_secs_f64();
    let retained_heap_bytes = heap_bytes()
        .zip(heap_before)
        .map(|(after, before)| after.saturating_sub(before));
    let mut solution = vec![Complex64::new(0.0, 0.0); rhs.len()];
    factor.solve_into(&rhs, &mut solution).unwrap();
    let start = Instant::now();
    for _ in 0..20 {
        factor.solve_into(&rhs, &mut solution).unwrap();
    }
    let warm_solve_seconds = start.elapsed().as_secs_f64() / 20.0;
    let mut refined = solution.clone();
    let mut corrections = vec![Complex64::new(0.0, 0.0); rhs.len()];
    for _ in 0..3 {
        let mut residual = rhs.clone();
        for &(r, c, value) in &entries {
            residual[r] -= value * refined[c];
        }
        factor.solve_into(&residual, &mut corrections).unwrap();
        for (value, delta) in refined.iter_mut().zip(&corrections) {
            *value += delta;
        }
    }
    let wide =
        |v: Complex64| faer::cx128::new(faer::fx128::from_f64(v.re), faer::fx128::from_f64(v.im));
    let mut high_precision_refined = input.initial_solution.as_ref().map_or_else(
        || solution.clone(),
        |values| {
            values
                .iter()
                .map(|&[re, im]| Complex64::new(re, im))
                .collect()
        },
    );
    assert_eq!(high_precision_refined.len(), rhs.len());
    let mut correction_sizes = Vec::new();
    for _ in 0..8 {
        let mut residual: Vec<_> = rhs.iter().copied().map(wide).collect();
        for &(r, c, value) in &entries {
            residual[r] -= wide(value) * wide(high_precision_refined[c]);
        }
        let residual: Vec<_> = residual
            .into_iter()
            .map(|v| Complex64::new(v.re.0 + v.re.1, v.im.0 + v.im.1))
            .collect();
        factor.solve_into(&residual, &mut corrections).unwrap();
        correction_sizes.push(corrections.iter().map(|v| v.norm()).fold(0.0_f64, f64::max));
        for (value, delta) in high_precision_refined.iter_mut().zip(&corrections) {
            *value += delta;
        }
    }
    let pairs = |values: &[Complex64]| values.iter().map(|v| [v.re, v.im]).collect::<Vec<_>>();
    let output = serde_json::json!({"input": input, "faer": {"factor_seconds":factor_seconds,"retained_heap_bytes":retained_heap_bytes,"warm_solve_seconds":warm_solve_seconds,"solution":pairs(&solution),"refined_solution":pairs(&refined),"high_precision_refined_solution":pairs(&high_precision_refined),"high_precision_correction_sizes":correction_sizes}});
    std::fs::write(
        std::env::var("TELLEGEN_LINEAR_OUTPUT").expect("output path"),
        serde_json::to_vec(&output).unwrap(),
    )
    .unwrap();
}
