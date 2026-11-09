#![cfg(feature = "sensitivity")]
use powerio::{BalancedNetwork, DcOpfInstance, PioModule, PioValue, Source};
use tellegen::{DcFormula as F, Problem, SolveRequest, Study, StudyOptions};
const CASE: &str = include_str!("data/dc_variants/case3_variants.m");
const FORMULAS: [F; 5] = [
    F::SeriesSusceptance,
    F::SeriesSusceptanceNoPhaseShift,
    F::TapAdjustedReactance,
    F::TapAdjustedSeriesSusceptance,
    F::ReactanceOnly,
];
fn network() -> BalancedNetwork {
    tellegen::ir::balanced_module(
        powerio::parse(Source::from_memory("case.m", CASE.as_bytes().to_vec()).unwrap()).unwrap(),
    )
    .unwrap()
    .into_value()
}
fn module(net: BalancedNetwork) -> String {
    tellegen::ir::serialize_module(&PioModule::new(PioValue::BalancedNetwork(net))).unwrap()
}
fn solve(net: BalancedNetwork, f: F, problem: &str) -> serde_json::Value {
    let req = serde_json::json!({"formulation":problem,"branch_susceptance_formula":f});
    serde_json::from_str(&tellegen::solve_module_json(&module(net), &req.to_string()).unwrap())
        .unwrap()
}
#[test]
fn omission_preserves_default_and_typed_instances_are_authoritative() {
    let text = module(network());
    let default: serde_json::Value =
        serde_json::from_str(&tellegen::solve_module_json(&text, "{}").unwrap()).unwrap();
    let explicit = solve(network(), F::SeriesSusceptance, "dcopf");
    assert_eq!(default["flows"], explicit["flows"]);
    assert_eq!(default["objective"], explicit["objective"]);
    for formula in FORMULAS {
        let instance = DcOpfInstance::from_network(network())
            .unwrap()
            .with_branch_susceptance_formula(formula.into());
        assert_eq!(
            tellegen::solve_instance(&instance, &SolveRequest::default())
                .unwrap()
                .branch_susceptance_formula,
            Some(formula)
        );
        let same = SolveRequest {
            branch_susceptance_formula: Some(formula),
            ..Default::default()
        };
        assert!(tellegen::solve_instance(&instance, &same).is_ok());
        let other = if formula == F::ReactanceOnly {
            F::SeriesSusceptance
        } else {
            F::ReactanceOnly
        };
        assert!(tellegen::solve_instance(
            &instance,
            &SolveRequest {
                branch_susceptance_formula: Some(other),
                ..Default::default()
            }
        )
        .unwrap_err()
        .contains("conflicts"));
    }
    assert!(tellegen::solve_module_json(
        &text,
        r#"{"formulation":"acpf","branch_susceptance_formula":"reactance_only"}"#
    )
    .unwrap_err()
    .contains("only supported"));
    assert!(
        tellegen::solve_module_json(&text, r#"{"branch_susceptance_formula":"unknown"}"#).is_err()
    );
}
#[test]
fn taps_and_shifts_only_affect_the_declared_modes() {
    for f in FORMULAS {
        let base = solve(network(), f, "dcpf");
        let mut changed = network();
        changed.branches_mut()[0].tap = 1.3;
        let tapped = solve(changed, f, "dcpf");
        assert_eq!(
            base["flows"] != tapped["flows"],
            matches!(f, F::TapAdjustedReactance | F::TapAdjustedSeriesSusceptance)
        );
        let mut changed = network();
        changed.branches_mut()[0].shift = 5.0;
        let shifted = solve(changed, f, "dcpf");
        assert_eq!(
            base["flows"] != shifted["flows"],
            !matches!(f, F::ReactanceOnly | F::SeriesSusceptanceNoPhaseShift)
        );
    }
    let mut ideal = network();
    for b in ideal.branches_mut() {
        b.r = 0.0;
        b.tap = 1.0;
        b.shift = 0.0;
    }
    let first = solve(ideal.clone(), FORMULAS[0], "dcpf");
    for f in FORMULAS {
        let other = solve(ideal.clone(), f, "dcpf");
        for (a, b) in first["flows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(other["flows"].as_array().unwrap())
        {
            assert!((a["pf"].as_f64().unwrap() - b["pf"].as_f64().unwrap()).abs() < 1e-10);
        }
    }
}
#[test]
fn saved_study_edits_and_reload_keep_the_convention() {
    for formula in FORMULAS {
        let mut study = Study::new_with_options(
            &module(network()),
            Problem::DcOpf,
            StudyOptions {
                branch_susceptance_formula: Some(formula),
            },
        )
        .unwrap();
        assert_eq!(study.solution().branch_susceptance_formula, Some(formula));
        let saved = study.save_instance_module().unwrap();
        let restored = Study::new(&saved, Problem::DcOpf).unwrap();
        assert_eq!(
            restored.solution().branch_susceptance_formula,
            Some(formula)
        );
        // A network-only export intentionally has no solver options.
        let net = tellegen::ir::deserialize_module(&study.save_module().unwrap()).unwrap();
        assert!(matches!(net.value(), PioValue::BalancedNetwork(_)));
        let edits = [tellegen::NetworkEdit::AddLoad {
            bus: tellegen::ElementKey::Id(2),
            p_mw: 0.01,
        }];
        study
            .preview(&edits, &[tellegen::Operand::Price(tellegen::Power::Active)])
            .unwrap();
        let response = study.commit(&edits).unwrap();
        let saved_edit = study.save_instance_module().unwrap();
        let reloaded = Study::new(&saved_edit, Problem::DcOpf).unwrap();
        assert!(
            (response.objective.unwrap() - reloaded.solution().objective.unwrap()).abs() < 1e-5
        );
        assert_eq!(response.branch_susceptance_formula, Some(formula));
        let solution = study.save_solution_module().unwrap();
        assert!(solution.contains(powerio::BranchSusceptanceFormula::from(formula).formula_name()));
    }
}

#[test]
fn independent_powermodels_reference_matches_flows_angles_dispatch_prices_and_cost() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("data/dc_variants/case3-reference.json")).unwrap();
    for formula in FORMULAS {
        let name = powerio::BranchSusceptanceFormula::from(formula).formula_name();
        for problem in ["dcpf", "dcopf"] {
            let actual = solve(network(), formula, problem);
            let reference = &expected[name][problem];
            for flow in actual["flows"].as_array().unwrap() {
                let i = flow["branch"].as_u64().unwrap().to_string();
                assert!(
                    (flow["pf"].as_f64().unwrap() - reference["pf"][&i].as_f64().unwrap()).abs()
                        < 1e-3
                );
            }
            for angle in actual["va"].as_array().unwrap() {
                let i = angle["bus"].as_u64().unwrap().to_string();
                assert!(
                    (angle["value"].as_f64().unwrap() - reference["va"][&i].as_f64().unwrap())
                        .abs()
                        < 1e-7
                );
            }
            if problem == "dcopf" {
                let objective = reference["objective"].as_f64().unwrap();
                assert!(
                    (actual["objective"].as_f64().unwrap() - objective).abs()
                        < 1e-6 * objective.abs().max(1.0)
                );
                for dispatch in actual["dispatch"].as_array().unwrap() {
                    let i = dispatch["gen"].as_u64().unwrap().to_string();
                    assert!(
                        (dispatch["pg"].as_f64().unwrap() - reference["pg"][&i].as_f64().unwrap())
                            .abs()
                            < 1e-3
                    );
                }
                for price in actual["lmp"].as_array().unwrap() {
                    let i = price["bus"].as_u64().unwrap().to_string();
                    assert!(
                        (price["value"].as_f64().unwrap() - reference["lmp"][&i].as_f64().unwrap())
                            .abs()
                            < 1e-4
                    );
                }
            }
        }
    }
}

#[test]
fn radial_dispatch_fixes_flows_while_taps_change_angles() {
    let mut radial = network();
    radial.branches_mut().remove(1);
    for formula in [F::TapAdjustedReactance, F::TapAdjustedSeriesSusceptance] {
        let a = solve(radial.clone(), formula, "dcpf");
        let mut changed = radial.clone();
        changed.branches_mut()[0].tap = 1.3;
        let b = solve(changed, formula, "dcpf");
        assert_ne!(a["va"], b["va"]);
        for (x, y) in a["flows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["flows"].as_array().unwrap())
        {
            assert!((x["pf"].as_f64().unwrap() - y["pf"].as_f64().unwrap()).abs() < 1e-9);
        }
    }
}

#[test]
fn durable_create_study_retains_the_formula_before_first_solve() {
    let request: tellegen::study_ops::CreateStudy = serde_json::from_value(serde_json::json!({
        "id":"variant", "title":"Variant", "input":module(network()), "formulation":"dcopf",
        "branch_susceptance_formula":"tap_adjusted_series_susceptance"
    }))
    .unwrap();
    let bundle = tellegen::study_ops::create_study(request).unwrap();
    let restored = tellegen::document::StudyBundle::import(&bundle.export().unwrap()).unwrap();
    let state = restored.document.states.values().next().unwrap();
    let input = &restored.artifacts[&state.input].text;
    let study = Study::new(input, Problem::DcOpf).unwrap();
    assert_eq!(
        study.solution().branch_susceptance_formula,
        Some(F::TapAdjustedSeriesSusceptance)
    );
}

#[test]
fn common_settings_calculate_and_retained_studies_keep_dc_conventions() {
    use tellegen::settings::{self, CalculationRequest, SettingsRequest};
    let text = module(network());
    for formula in FORMULAS {
        for calculation in ["dcpf", "dcopf"] {
            let request: CalculationRequest = serde_json::from_value(serde_json::json!({
                "module_json": text,
                "settings": {"calculation": calculation,
                    "overrides": {"branch_susceptance_formula": formula}}
            }))
            .unwrap();
            let actual = settings::calculate(&request, None).unwrap();
            let expected = solve(network(), formula, calculation);
            assert_eq!(actual.result["flows"], expected["flows"]);
            assert_eq!(
                actual.result["branch_susceptance_formula"],
                serde_json::json!(formula)
            );
        }
        let request: SettingsRequest = serde_json::from_value(serde_json::json!({
            "calculation": "dcopf", "overrides": {
                "branch_susceptance_formula": formula, "angle_difference": false
            }
        }))
        .unwrap();
        let study = Study::new_with_settings(&text, &request).unwrap();
        let saved = study.save_instance_module().unwrap();
        let mut restored =
            Study::new_with_options(&saved, Problem::DcOpf, StudyOptions::default()).unwrap();
        assert_eq!(
            restored.solution().branch_susceptance_formula,
            Some(formula)
        );
        assert_eq!(restored.calculation_options()["angle_difference"], false);
        let result = restored
            .commit(&[tellegen::NetworkEdit::AddLoad {
                bus: tellegen::ElementKey::Id(2),
                p_mw: 0.01,
            }])
            .unwrap();
        assert_eq!(result.branch_susceptance_formula, Some(formula));
        let other = if formula == F::ReactanceOnly {
            F::SeriesSusceptance
        } else {
            F::ReactanceOnly
        };
        let conflict: SettingsRequest = serde_json::from_value(serde_json::json!({
            "calculation": "dcopf", "overrides": {"branch_susceptance_formula": other}
        }))
        .unwrap();
        assert!(Study::new_with_settings(&saved, &conflict)
            .unwrap_err()
            .contains("conflicts"));
    }
    for calculation in ["dcopf", "dcpf", "acpf"] {
        let request: SettingsRequest = serde_json::from_value(serde_json::json!({
            "calculation": calculation, "overrides": {"branch_susceptance_formula": "unknown"}
        }))
        .unwrap();
        assert!(settings::resolve(&request).is_err());
    }
}
