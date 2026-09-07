//! Verify a prepared case with a capacity gradient, finite difference, and bounded search.
use tellegen::objective::{
    DecisionSpace, DecisionVariable, Intervention, ObservableWeight, StudyObjective,
};
use tellegen::{ElementKey, NetworkEdit, Operand, Power, Problem, Study};

fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: verify_planning CASE.pio.json")?;
    let input = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let study = Study::new(&input, Problem::DcOpf)?;
    let network = study.materialized_network()?;
    let target = study
        .solution()
        .lmp
        .as_ref()
        .ok_or("missing LMPs")?
        .iter()
        .max_by(|a, b| a.value.total_cmp(&b.value))
        .ok_or("empty LMPs")?;
    let objective = StudyObjective::WeightedObservable {
        operand: Operand::Price(Power::Active),
        weights: vec![ObservableWeight {
            element: ElementKey::Id(target.bus as i64),
            weight: 1.0,
        }],
    };
    let variables = network
        .branches()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.in_service && b.rate_a > 0.0)
        .map(|(i, _)| DecisionVariable {
            id: format!("line:{}", i + 1),
            element: ElementKey::Id((i + 1) as i64),
            intervention: Intervention::BranchRating,
            lower: 0.0,
            upper: 20.0,
            increment: 1.0,
            budget_weight: 1.0,
        })
        .collect::<Vec<_>>();
    let space = DecisionSpace {
        max_changed_elements: 2,
        total_budget: 20.0,
        demand: None,
        variables,
    };
    let zero = vec![0.0; space.variables.len()];
    let direction = study.objective_gradient(&objective, &space, &zero)?;
    let (row, derivative) = direction
        .gradient
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.value.abs().total_cmp(&b.value.abs()))
        .ok_or("empty gradient")?;
    let step = 0.001;
    let mut shifted = study.fork();
    shifted.commit(&[NetworkEdit::AdjustBranchRating {
        branch: space.variables[row].element.clone(),
        delta_mw: step,
    }])?;
    let mut changes = zero.clone();
    changes[row] = step;
    let moved = shifted.objective_value(&objective, &space, &changes)?;
    let finite_difference = (moved - direction.value) / step;
    if (finite_difference - derivative.value).abs() > 1e-4_f64.max(0.03 * derivative.value.abs()) {
        return Err(format!(
            "gradient {} differs from finite difference {}",
            derivative.value, finite_difference
        ));
    }
    let result = tellegen::exploration::explore(
        &study,
        &objective,
        &space,
        &zero,
        &tellegen::exploration::SearchOptions {
            max_solves: 2,
            beam_width: 2,
            max_iterations: 1,
            min_improvement: 1e-7,
        },
        || false,
    )?;
    if result.solve_count > 2
        || result
            .best
            .as_ref()
            .is_some_and(|b| b.value > direction.value + 1e-7)
    {
        return Err("planning exceeded its budget or accepted a worse result".into());
    }
    if !study.edits().is_empty() {
        return Err("planning applied an edit".into());
    }
    println!(
        "{}",
        serde_json::json!({"buses":network.buses().len(),"candidate_lines":space.variables.len(),"target_bus":target.bus,"baseline_lmp":direction.value,"derivative":derivative.value,"finite_difference":finite_difference,"trial_solves":result.solve_count,"trials":result.trials.len(),"termination":result.termination,"best_lmp":result.best.map(|b|b.value),"applied":false})
    );
    Ok(())
}
