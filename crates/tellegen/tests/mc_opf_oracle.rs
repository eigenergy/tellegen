//! Synthetic, pinned BMOPFTools oracle; ordinary tests do not invoke Julia.
#![cfg(feature = "mc-opf")]
use powerio::{McAcOpfInstance, ParseOptions, PioValue, Source};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tellegen::{solve_mc_ac_opf_instance, McOpfOptions};

fn network() -> powerio_dist::MulticonductorNetwork {
    let text = include_str!("data/mc_opf/unbalanced.json");
    let module = powerio::parse_with_options(
        Source::from_memory("unbalanced.bmopf.json", text.as_bytes().to_vec()).unwrap(),
        &ParseOptions::default().format("bmopf-json").unwrap(),
    )
    .unwrap();
    assert!(
        module.diagnostics().is_empty(),
        "unexpected fixture conversion diagnostics: {:?}",
        module.diagnostics()
    );
    let PioValue::MulticonductorNetwork(net) = module.into_value() else {
        panic!("expected network")
    };
    net
}
fn near(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() <= tolerance,
        "{a} != {b}; tolerance {tolerance}"
    );
}
#[test]
fn unbalanced_wye_delta_dispatch_matches_bmopf() {
    let net = network();
    let result = solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(net).unwrap()),
        &McOpfOptions::default(),
    )
    .unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("data/mc_opf/unbalanced-reference.json")).unwrap();
    let digest: String = Sha256::digest(include_bytes!("data/mc_opf/unbalanced.json"))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        reference["provenance"]["input_sha256"].as_str(),
        Some(digest.as_str())
    );
    near(
        result.solution.objective(),
        reference["objective"].as_f64().unwrap(),
        1e-7,
    );
    for bus in ["s", "b"] {
        for terminal in ["a", "b", "c", "n"] {
            let magnitude = result
                .solution
                .terminal_voltage_magnitude(bus, terminal)
                .unwrap();
            let angle = result
                .solution
                .terminal_voltage_angle(bus, terminal)
                .unwrap();
            near(
                magnitude * angle.cos(),
                reference["bus"][bus][terminal]["vr"].as_f64().unwrap(),
                1e-5,
            );
            near(
                magnitude * angle.sin(),
                reference["bus"][bus][terminal]["vi"].as_f64().unwrap(),
                1e-5,
            );
            near(
                result
                    .solution
                    .terminal_voltage_magnitude(bus, terminal)
                    .unwrap(),
                reference["bus"][bus][terminal]["vm"].as_f64().unwrap(),
                1e-5,
            );
        }
    }
    for (k, terminal) in ["a", "b", "c", "n"].iter().enumerate() {
        let expected = &reference["line"]["l"][terminal];
        for (actual, key) in result.branches[0].current_from_a[k]
            .iter()
            .zip(["cr_fr", "ci_fr"])
            .chain(
                result.branches[0].current_to_a[k]
                    .iter()
                    .zip(["cr_to", "ci_to"]),
            )
        {
            near(*actual, expected[key].as_f64().unwrap(), 1e-5);
        }
    }
    for d in result.devices.iter().filter(|d| d.identity == "g") {
        for p in &d.coil_power_va {
            near(p[0], 50.0, 1e-3);
        }
        near(
            d.coil_power_va.iter().map(|p| p[0]).sum(),
            d.terminal_power_va.iter().map(|p| p[0]).sum(),
            1e-8,
        );
    }
    assert!(result.residuals.maximum() < 1e-6);
}

#[test]
fn terminal_and_conductor_permutations_preserve_physics() {
    let net = network();
    let original = solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(net.clone()).unwrap()),
        &McOpfOptions::default(),
    )
    .unwrap();
    let mut permuted = net;
    // Canonical scalar cost statements broadcast over phase coils.
    permuted.sources_mut()[0].energy_cost_rate = Some(vec![0.2]);
    permuted.generators_mut()[0].cost = Some(vec![0.05]);
    for bus in permuted.buses_mut() {
        bus.terminals.reverse();
    }
    // Device coil order stays the same. Line maps and both matrix axes move together.
    for line in permuted.lines_mut() {
        line.terminal_map_from.reverse();
        line.terminal_map_to.reverse();
    }
    for lc in permuted.line_codes_mut() {
        for matrix in [
            &mut lc.r_series,
            &mut lc.x_series,
            &mut lc.g_from,
            &mut lc.b_from,
            &mut lc.g_to,
            &mut lc.b_to,
        ] {
            matrix.reverse();
            for row in matrix {
                row.reverse();
            }
        }
        lc.i_max.as_mut().unwrap().reverse();
        lc.s_max.as_mut().unwrap().reverse();
    }
    // Voltage vectors are indexed by phase order in the bus declaration.
    for bus in permuted.buses_mut() {
        if let Some(v) = &mut bus.vpn_min {
            v.reverse();
        }
        if let Some(v) = &mut bus.vpn_max {
            v.reverse();
        }
    }
    let reordered = solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(permuted).unwrap()),
        &McOpfOptions::default(),
    )
    .unwrap();
    near(
        original.solution.objective(),
        reordered.solution.objective(),
        1e-8,
    );
    for (a, b) in original.branches[0]
        .current_from_a
        .iter()
        .zip(reordered.branches[0].current_from_a.iter().rev())
    {
        near(a[0], b[0], 1e-6);
        near(a[1], b[1], 1e-6);
    }
}
