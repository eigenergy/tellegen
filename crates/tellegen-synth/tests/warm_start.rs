mod common;

use tellegen::{AcPfStartStrategy, Iterations, PowerFlowStart, SolveResponse};
use tellegen_synth::{generate, AcPfOutcome};

fn iterations(response: &SolveResponse) -> usize {
    match response.iterations {
        Some(Iterations::Newton { count, residual }) => {
            assert!(residual < 1e-8);
            count
        }
        _ => panic!("expected Newton diagnostics"),
    }
}

#[test]
fn emitted_planning_state_survives_serialization_and_warms_the_viewer_solve() {
    let request = common::request(common::iraq_box(), 7, Some(80));
    let (module, report) = generate(request, common::places(), None).unwrap();
    let net = module.value();
    assert!(net.buses().iter().any(|b| b.va.abs() > 1e-3));
    assert!(net.buses().iter().all(|b| b.va.is_finite() && b.vm == 1.02));
    let ac = report.ac_pf.unwrap();
    assert_eq!(ac.status, AcPfOutcome::Converged, "{:?}", ac.message);
    for island in ac.islands.iter().filter(|i| i.status == "converged") {
        let init = island.initialization.unwrap();
        assert_eq!(init.requested, PowerFlowStart::Case);
        assert_eq!(init.used, AcPfStartStrategy::Case);
        assert_eq!(init.attempts, 1);
    }
    let text = tellegen::ir::serialize_module(&module).unwrap();
    let viewer = tellegen::Study::new(&text, tellegen::Problem::AcPf).unwrap();
    assert_eq!(
        viewer.solution().initialization.unwrap().requested,
        PowerFlowStart::Case
    );
    let explicit_flat = tellegen::Study::new_with_settings(
        &text,
        &tellegen::settings::SettingsRequest::new(tellegen::settings::Calculation::AcPf)
            .with_overrides(serde_json::from_value(serde_json::json!({"start": "flat"})).unwrap()),
    )
    .unwrap();
    assert_eq!(
        explicit_flat.solution().initialization.unwrap().requested,
        PowerFlowStart::Flat
    );
    let restored = tellegen::ir::deserialize_module(&text).unwrap();
    let powerio::PioValue::BalancedNetwork(restored) = restored.value() else {
        panic!("expected a balanced network")
    };
    for (before, after) in net.buses().iter().zip(restored.buses()) {
        assert!((before.va - after.va).abs() < 1e-12);
        assert_eq!(before.vm, after.vm);
    }
    let solve = |start| -> SolveResponse {
        serde_json::from_str(
            &tellegen::solve_module_json(
                &text,
                &format!(r#"{{"formulation":"acpf","start":"{start}"}}"#),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let warm = solve("case");
    let flat = solve("flat");
    assert!(iterations(&warm) <= iterations(&flat));
    for (a, b) in warm
        .vm
        .as_ref()
        .unwrap()
        .iter()
        .zip(flat.vm.as_ref().unwrap())
    {
        assert!((a.value - b.value).abs() < 1e-6);
    }
    for (a, b) in warm
        .va
        .as_ref()
        .unwrap()
        .iter()
        .zip(flat.va.as_ref().unwrap())
    {
        assert!((a.value - b.value).abs() < 1e-6);
    }
}
