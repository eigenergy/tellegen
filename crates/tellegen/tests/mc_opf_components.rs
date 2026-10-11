//! Independent BMOPFTools component witnesses, frozen with the input hash.
#![cfg(feature = "mc-opf")]
use num_complex::Complex64;
use powerio::{McAcOpfInstance, ParseOptions, PioValue, Source};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tellegen::{solve_mc_ac_opf_instance, McOpfOptions, McOpfResult};

fn check_bundle(name: &str, bundle: &str) {
    let input = match bundle {
        "components" => include_str!("data/mc_opf/components.json"),
        "equipment" => include_str!("data/mc_opf/equipment.json"),
        "controls" => include_str!("data/mc_opf/controls.json"),
        _ => include_str!("data/mc_opf/bounds.json"),
    };
    let cases: serde_json::Value = serde_json::from_str(input).unwrap();
    let reference: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/tests/data/mc_opf/{bundle}-reference.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        reference["provenance"]["input_sha256"],
        Sha256::digest(input.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    assert_eq!(
        reference["provenance"]["bmopftools_commit"],
        "a8b52e069bfd4a7a57434c91bc0470ac03cfdd55"
    );
    let module = powerio::parse_with_options(
        Source::from_memory(
            "components.bmopf.json",
            serde_json::to_vec(&cases[name]).unwrap(),
        )
        .unwrap(),
        &ParseOptions::default().format("bmopf-json").unwrap(),
    )
    .unwrap();
    assert!(
        module
            .diagnostics()
            .iter()
            .all(|d| d.code() == "READ.BMOPF.TRANSFORMER_OPEN_DELTA_SPLIT"
                || (d.code() == "READ.BMOPF.CAPACITOR_LOWERED"
                    && d.severity() == powerio::DiagnosticSeverity::Remark
                    && name.starts_with("capacitor_"))
                || (d.code() == "READ.BMOPF.RETAINED_SOURCE_ONLY"
                    && (d.message().contains("va_diff")
                        || d.message().contains("va_nom")
                        || d.message().contains("i_max_from")
                        || d.message().contains("i_max_to")))),
        "{name}: {:?}",
        module.diagnostics()
    );
    let PioValue::MulticonductorNetwork(net) = module.into_value() else {
        panic!("expected network")
    };
    let result = solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(net).unwrap()),
        &McOpfOptions {
            objective_scale: if bundle == "controls" && name != "tap_wye_delta" {
                1000.0
            } else {
                1.0
            },
            power_base_va: 1000.0,
            ..McOpfOptions::default()
        },
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    let expected = &reference["cases"][name];
    let near = |a: f64, b: f64, tol: f64| {
        assert!((a - b).abs() <= tol, "{name}: {a} != {b}, tolerance {tol}")
    };
    if name.starts_with("ibr_") && name.ends_with("_limit") {
        let inv = result.devices.iter().find(|d| d.identity == "inv").unwrap();
        match name {
            "ibr_single_phase_current_limit" => near(
                inv.coil_current_a[0][0].hypot(inv.coil_current_a[0][1]),
                0.05,
                1e-6,
            ),
            "ibr_four_leg_neutral_limit" => {
                let current = inv
                    .coil_current_a
                    .iter()
                    .fold(Complex64::new(0.0, 0.0), |sum, i| {
                        sum + Complex64::new(i[0], i[1])
                    });
                near(current.norm(), 0.05, 1e-6);
            }
            "ibr_three_leg_apparent_limit" => {
                for power in &inv.coil_power_va {
                    near(power[0].hypot(power[1]), 30.0, 1e-4);
                }
            }
            _ => unreachable!(),
        }
    }
    near(
        result.solution.objective(),
        expected["objective"].as_f64().unwrap(),
        if bundle == "bounds" { 1e-3 } else { 1e-7 },
    );
    for (bus, terminals) in expected["bus"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|_| bundle != "bounds")
    {
        for (terminal, v) in terminals.as_object().unwrap() {
            let Some(vr) = v["vr"].as_f64() else { continue };
            let mag = result
                .solution
                .terminal_voltage_magnitude(bus, terminal)
                .unwrap();
            let angle = result
                .solution
                .terminal_voltage_angle(bus, terminal)
                .unwrap();
            near(
                mag * angle.cos(),
                vr,
                if bundle == "bounds" {
                    1e-2
                } else if bundle == "controls" {
                    1e-3
                } else {
                    2e-4
                },
            );
            near(
                mag * angle.sin(),
                v["vi"].as_f64().unwrap(),
                if bundle == "bounds" {
                    1e-2
                } else if bundle == "controls" {
                    1e-3
                } else {
                    2e-4
                },
            );
        }
    }
    if bundle == "bounds" {
        check_active_bound(name, &result, expected);
    }
    assert!(
        result.residuals.maximum() < 1e-6,
        "{name}: {:?}",
        result.residuals
    );
}
macro_rules! witnesses {($($name:ident),* $(,)?)=>{$(#[test] fn $name(){check_bundle(stringify!($name),"components");})*};}
witnesses!(
    load_constant_current,
    load_constant_impedance,
    load_exponential,
    load_zip,
    capacitor_wye,
    capacitor_delta,
    capacitor_single_phase,
    ibr_single_phase,
    ibr_four_leg,
    ibr_three_leg,
    ibr_pf_lag,
    ibr_pf_lead,
    ibr_dc_link,
    ibr_droop_pg_averaged,
    ibr_droop_pg_per_phase,
    ibr_droop_pn_averaged,
    ibr_droop_pn_per_phase,
    ibr_droop_pp_averaged,
    ibr_droop_pp_per_phase,
    transformer_single_phase,
    transformer_single_phase_ideal,
    transformer_single_phase_tap,
    transformer_center_tap,
    transformer_wye_delta,
    transformer_delta_wye,
    transformer_n_winding,
    transformer_regulator_a,
    transformer_regulator_b,
    transformer_open_delta_abbc,
    transformer_open_delta_bcac,
    transformer_open_delta_caba
);

macro_rules! bundle_witnesses {($bundle:literal;$($function:ident=>$name:literal),* $(,)?)=>{$(#[test] fn $function(){check_bundle($name,$bundle);})*};}
bundle_witnesses!("bounds"; a_vmax=>"A_vmax",d1_va_diff=>"D1_va_diff",f_vn_max=>"F_vn_max",g1_vpn_max=>"G1_vpn_max",g2_vpn_min=>"G2_vpn_min",h1_vpp_max=>"H1_vpp_max",h2_vpp_min=>"H2_vpp_min",s1_vpos_max=>"S1_vpos_max",s2_vneg_max=>"S2_vneg_max",s3_vzero_max=>"S3_vzero_max",w1_imax_shunt=>"W1_imax_shunt",x1_srating=>"X1_srating");
bundle_witnesses!("controls"; tap_single_phase=>"tap_single_phase",tap_center_tap=>"tap_center_tap",tap_delta_wye=>"tap_delta_wye",tap_wye_delta=>"tap_wye_delta",tap_regulator_a=>"tap_regulator_a",tap_regulator_b=>"tap_regulator_b",tap_open_delta_abbc=>"tap_open_delta_abbc",tap_open_delta_bcac=>"tap_open_delta_bcac",tap_open_delta_caba=>"tap_open_delta_caba");

// BMOPFTools validation.md uses 10 W dispatch and 0.01 V physical tolerances.
// Compare total dispatch, not a weakly determined per-phase optimizer allocation.
fn check_active_bound(name: &str, result: &McOpfResult, expected: &serde_json::Value) {
    let near = |a: f64, b: f64, tol: f64| {
        assert!(
            (a - b).abs() <= tol,
            "{name}: active quantity {a} != {b}, tolerance {tol}"
        )
    };
    for (id, phases) in expected["generator"].as_object().unwrap() {
        let expected: f64 = phases
            .as_object()
            .unwrap()
            .values()
            .filter_map(|v| v["pg"].as_f64())
            .sum();
        let actual: f64 = result
            .devices
            .iter()
            .find(|d| &d.identity == id)
            .unwrap()
            .coil_power_va
            .iter()
            .map(|s| s[0])
            .sum();
        near(actual, expected, 10.0);
    }
    let voltage = |bus: &str, t: &str| {
        Complex64::from_polar(
            result.solution.terminal_voltage_magnitude(bus, t).unwrap(),
            result.solution.terminal_voltage_angle(bus, t).unwrap(),
        )
    };
    let neutral = |bus: &str| {
        result
            .solution
            .terminal_voltage_magnitude(bus, "n")
            .map_or(Complex64::new(0.0, 0.0), |v| {
                Complex64::from_polar(v, result.solution.terminal_voltage_angle(bus, "n").unwrap())
            })
    };
    let pn = |bus: &str, t: &str| (voltage(bus, t) - neutral(bus)).norm();
    match name {
        "A_vmax" => {
            for t in ["1", "2", "3"] {
                near(voltage("loadbus", t).norm(), 235.0, 0.01);
            }
        }
        "D1_va_diff" => near(
            (voltage("sourcebus", "1") * voltage("busm", "1").conj()).arg(),
            -0.03,
            1e-5,
        ),
        "F_vn_max" => near(neutral("buse").norm(), 6.0, 0.01),
        "G1_vpn_max" => near(
            ["1", "2", "3"]
                .map(|t| pn("buse", t))
                .into_iter()
                .fold(0.0, f64::max),
            240.0,
            0.01,
        ),
        "G2_vpn_min" => near(
            ["1", "2", "3"]
                .map(|t| pn("buse", t))
                .into_iter()
                .fold(f64::INFINITY, f64::min),
            218.0,
            0.01,
        ),
        "H1_vpp_max" => near(
            (voltage("buse", "1") - voltage("buse", "2")).norm(),
            405.0,
            0.01,
        ),
        "H2_vpp_min" => near(
            (voltage("buse", "1") - voltage("buse", "2")).norm(),
            395.0,
            0.01,
        ),
        "S1_vpos_max" | "S2_vneg_max" | "S3_vzero_max" => {
            let a = Complex64::from_polar(1.0, std::f64::consts::TAU / 3.0);
            let [v1, v2, v3] = ["1", "2", "3"].map(|t| voltage("buse", t) - neutral("buse"));
            let (sequence, bound) = match name {
                "S1_vpos_max" => ((v1 + a * v2 + a * a * v3) / 3.0, 236.0),
                "S2_vneg_max" => ((v1 + a * a * v2 + a * v3) / 3.0, 3.0),
                _ => ((v1 + v2 + v3) / 3.0, 3.0),
            };
            near(sequence.norm(), bound, 0.01);
        }
        "W1_imax_shunt" => {
            let line = result
                .branches
                .iter()
                .find(|b| b.identity == "line:l1")
                .unwrap();
            for (from, to) in line.current_from_a.iter().zip(&line.current_to_a) {
                near(from[0].hypot(from[1]), 25.0, 0.01);
                near(to[0].hypot(to[1]), 24.6235, 0.01);
            }
        }
        "X1_srating" => {
            for tx in &result.transformers {
                near(
                    tx.coil_power_va[0][0].hypot(tx.coil_power_va[0][1]),
                    10000.0,
                    0.01,
                );
            }
        }
        _ => panic!("missing physical-bound assertion for {name}"),
    }
}

bundle_witnesses!("equipment";center_terminal_ratings=>"center_terminal_ratings",delta_wye_neutral_rating=>"delta_wye_neutral_rating",center_neutral_impedance=>"center_neutral_impedance",regulator_return_bond=>"regulator_return_bond",n_winding_core_1=>"n_winding_core_1",n_winding_core_3=>"n_winding_core_3");

bundle_witnesses!("equipment";n_winding_mixed_connections=>"n_winding_mixed_connections",n_winding_two_phase_delta=>"n_winding_two_phase_delta");

fn parsed_case(input: &str, name: &str) -> powerio_dist::MulticonductorNetwork {
    let cases: serde_json::Value = serde_json::from_str(input).unwrap();
    let module = powerio::parse_with_options(
        Source::from_memory(
            "witness.bmopf.json",
            serde_json::to_vec(&cases[name]).unwrap(),
        )
        .unwrap(),
        &ParseOptions::default().format("bmopf-json").unwrap(),
    )
    .unwrap();
    let PioValue::MulticonductorNetwork(net) = module.into_value() else {
        panic!("network")
    };
    net
}

#[test]
fn transformer_binding_limit_arbitrates_reversed_generator_costs() {
    let mut net = parsed_case(include_str!("data/mc_opf/bounds.json"), "X1_srating");
    let solve = |net| {
        solve_mc_ac_opf_instance(
            Arc::new(McAcOpfInstance::from_network(net).unwrap()),
            &McOpfOptions::default(),
        )
        .unwrap()
    };
    let initial = solve(net.clone());
    for generator in net.generators_mut() {
        generator.cost = Some(vec![
            if generator.name == "der_m" {
                -3.0
            } else {
                -1.0
            };
            generator.p_max.as_ref().unwrap().len()
        ]);
    }
    let reversed = solve(net);
    let dispatch = |r: &McOpfResult, id: &str| {
        r.devices
            .iter()
            .find(|d| d.identity == id)
            .unwrap()
            .coil_power_va[0][0]
    };
    assert!(dispatch(&initial, "der_e") > 12000.0);
    assert!(dispatch(&reversed, "der_e") < 1.0);
    assert!(dispatch(&reversed, "der_m") > 12000.0);
    for r in [&initial, &reversed] {
        for tx in &r.transformers {
            let s = tx.coil_power_va[0];
            assert!((s[0].hypot(s[1]) - 10000.0).abs() < 0.01);
        }
    }
}

#[test]
fn component_physics_survives_base_changes_and_terminal_storage_permutations() {
    for name in [
        "load_zip",
        "ibr_droop_pn_per_phase",
        "transformer_center_tap",
        "transformer_delta_wye",
        "transformer_n_winding",
    ] {
        let mut net = parsed_case(include_str!("data/mc_opf/components.json"), name);
        let initial = solve_mc_ac_opf_instance(
            Arc::new(McAcOpfInstance::from_network(net.clone()).unwrap()),
            &McOpfOptions {
                objective_scale: 1000.0,
                ..McOpfOptions::default()
            },
        )
        .unwrap();
        // Element terminal maps remain authoritative; only bus storage order changes.
        net.buses_mut().reverse();
        for bus in net.buses_mut() {
            bus.terminals.reverse();
        }
        let changed = solve_mc_ac_opf_instance(
            Arc::new(McAcOpfInstance::from_network(net).unwrap()),
            &McOpfOptions {
                voltage_base_v: 115.0,
                power_base_va: 2000.0,
                objective_scale: 1000.0,
                ..McOpfOptions::default()
            },
        )
        .unwrap();
        assert!(
            (initial.solution.objective() - changed.solution.objective()).abs() < 1e-7,
            "{name}"
        );
        for bus in initial.solution.instance().network().buses() {
            for terminal in &bus.terminals {
                let voltage = |r: &McOpfResult| {
                    Complex64::from_polar(
                        r.solution
                            .terminal_voltage_magnitude(&bus.id, terminal)
                            .unwrap(),
                        r.solution
                            .terminal_voltage_angle(&bus.id, terminal)
                            .unwrap(),
                    )
                };
                assert!(
                    (voltage(&initial) - voltage(&changed)).norm() < 2e-4,
                    "{name} {}:{terminal}",
                    bus.id
                );
            }
        }
    }
}

bundle_witnesses!("equipment";ibr_single_phase_current_limit=>"ibr_single_phase_current_limit",ibr_four_leg_neutral_limit=>"ibr_four_leg_neutral_limit",ibr_three_leg_apparent_limit=>"ibr_three_leg_apparent_limit");
