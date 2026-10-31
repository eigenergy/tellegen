//! Terminal-role contracts fail before optimization. No solver runs in this file.
#![cfg(feature = "mc-opf")]
use powerio::{McAcOpfInstance, ParseOptions, PioValue, Source};
use powerio_dist::{
    Configuration, DistBus, DistIbr, DistLoad, IbrPrimeMover, IbrTopology, MulticonductorNetwork,
    VoltageSource,
};
use std::sync::Arc;
use tellegen::{solve_mc_ac_opf_instance, McOpfError, McOpfErrorKind, McOpfOptions, McOpfStage};

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}

fn network() -> MulticonductorNetwork {
    let mut net = MulticonductorNetwork::new();
    let mut bus = DistBus::new("s", names(&["a", "b", "c", "n"]));
    bus.grounded = names(&["n"]);
    net.buses_mut().push(bus);
    net.sources_mut().push(VoltageSource::new(
        "grid",
        "s",
        names(&["a", "b", "c", "n"]),
        vec![230., 230., 230., 0.],
        vec![
            0.,
            -std::f64::consts::TAU / 3.,
            std::f64::consts::TAU / 3.,
            0.,
        ],
    ));
    net
}

fn assert_role_error(error: McOpfError) {
    assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
    assert_eq!(error.stage, McOpfStage::Preparation);
    assert!(error.solver_status.is_none());
    assert!(error.residuals.is_none());
    assert!(error
        .diagnostics
        .iter()
        .any(|d| d.code == "BUILD.MULTI.OPF_INVALID"
            && d.message.contains("bad")
            && d.message.contains("delta requires phase terminals")));
}

fn assert_rejected(net: MulticonductorNetwork) {
    let instance = McAcOpfInstance::from_network(net).unwrap();
    assert_role_error(
        solve_mc_ac_opf_instance(Arc::new(instance.clone()), &McOpfOptions::default()).unwrap_err(),
    );
}

#[test]
fn direct_instances_reject_nonphase_delta_and_three_leg_before_solving() {
    for terminal in ["n", "0", "return", "soil"] {
        for inverter in [false, true] {
            let mut net = network();
            net.buses_mut()[0].terminals[3] = terminal.into();
            net.buses_mut()[0].grounded[0] = terminal.into();
            net.sources_mut()[0].terminal_map[3] = terminal.into();
            if matches!(terminal, "return" | "soil") {
                let roles = if terminal == "return" {
                    serde_json::json!({"neutral":["return"],"earth":[]})
                } else {
                    serde_json::json!({"neutral":[],"earth":["soil"]})
                };
                net.extras_mut()
                    .insert("bmopf_terminal_conventions".into(), roles);
            }
            if inverter {
                net.ibrs_mut().push(DistIbr::new(
                    "bad",
                    "s",
                    names(&["a", "b", terminal]),
                    IbrTopology::ThreeLeg,
                    IbrPrimeMover::Battery,
                    vec![1000.; 3],
                ));
            } else {
                net.loads_mut().push(DistLoad::new(
                    "bad",
                    "s",
                    names(&["a", "b", terminal]),
                    Configuration::Delta,
                    vec![100.; 3],
                    vec![20.; 3],
                ));
            }
            assert_rejected(net);
        }
    }
}

#[test]
fn bmopf_delta_neutral_is_rejected_even_when_decoding_succeeds() {
    let input = serde_json::json!({
        "bus":{"s":{"terminal_names":["a","b","c","n"],"perfectly_grounded_terminals":["n"]}},
        "voltage_source":{"grid":{"bus":"s","terminal_map":["a","b","c","n"],
            "v_magnitude":[230,230,230,0],"v_angle":[0,-2.0943951023931953,2.0943951023931953,0]}},
        "load":{"bad":{"bus":"s","terminal_map":["a","b","n"],
            "configuration":"DELTA","p_nom":[1000,1000,1000],"q_nom":[200,200,200]}}
    });
    let module = powerio::parse_with_options(
        Source::from_memory("roles.bmopf.json", serde_json::to_vec(&input).unwrap()).unwrap(),
        &ParseOptions::default().format("bmopf-json").unwrap(),
    )
    .unwrap();
    let PioValue::MulticonductorNetwork(net) = module.into_value() else {
        panic!("expected a distribution network");
    };
    assert_rejected(net);
}
