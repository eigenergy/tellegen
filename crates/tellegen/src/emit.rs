//! Emit a portable PowerIO solution from a solved tellegen model.
//!
//! The solver works on the dense normalized model; the portable solution
//! states its columns in the source network's table order, in served units
//! (MW, degrees, objective units per MW). The source row maps built at model
//! construction carry each dense value back to its table row; a source row
//! the model excluded (out of service, isolated) reads as NaN in the value
//! columns and zero in the dual columns, powerio's spelling for a value the
//! producer cannot state.

use std::sync::Arc;

use powerio::{DcOpfInstance, DcOpfSolution};
use powerio_matrix::PreparedObjective;
use powerio_prob::Termination;

use crate::model::DcNetwork;
use crate::problem::{dc_opf_cancellable, DcOpfSolution as SolverSolution};

/// Solve a typed PowerIO DC OPF instance and emit its portable solution. The
/// private solver workspace never crosses the API boundary, and its undeclared
/// load shedding relaxation stays disabled.
pub fn solve_dc_opf_instance(
    instance: Arc<DcOpfInstance>,
    producer: impl Into<String>,
) -> Result<DcOpfSolution, String> {
    let mut model = DcNetwork::from_instance(&instance)?;
    model.allow_shed = false;
    let solution = dc_opf_cancellable(&model, None)?;
    emit_dc_opf_solution(instance, &model, &solution, producer)
}

/// Assemble the portable [`powerio::DcOpfSolution`] for `instance` from
/// a solved model, with the optional economic outputs attached: the bus
/// column is the derivative of the optimal objective with respect to added
/// demand, and the two branch columns are the nonnegative multipliers on the
/// positive and negative thermal flow bounds.
///
/// `dc` must be the model [`DcNetwork::from_network`] built from
/// `instance.network()`; the source row maps are what align the dense
/// columns with the instance's tables.
pub fn emit_dc_opf_solution(
    instance: Arc<DcOpfInstance>,
    dc: &DcNetwork,
    sol: &SolverSolution,
    producer: impl Into<String>,
) -> Result<DcOpfSolution, String> {
    let network = instance.network();
    let base = dc.base_mva;
    let n_source_buses = network.buses().len();
    let n_source_branches = network.branches().len();
    let n_source_generators = network.generators().len();
    if dc.bus_analysis_rows.len() != dc.n || dc.bus_source_rows.len() != dc.n {
        return Err("DC bus row maps are not aligned with the solved bus columns".to_owned());
    }

    let mut bus_voltage_angle = vec![f64::NAN; n_source_buses];
    let mut bus_active_injection = vec![f64::NAN; n_source_buses];
    let economic_outputs = dc.objective == PreparedObjective::NetworkGeneratorCost;
    let mut bus_marginal = economic_outputs.then(|| vec![f64::NAN; n_source_buses]);
    let marginal = economic_outputs.then(|| sol.nodal_marginal_values(base));
    // Net active injection per dense bus: generation minus served demand
    // minus the constant shunt withdrawal, the DC balance restated.
    let mut injection = vec![0.0f64; dc.n];
    for (generator, &bus) in dc.gen_bus.iter().enumerate() {
        injection[bus] += sol.pg[generator];
    }
    for (dense, net_injection) in injection.iter_mut().enumerate() {
        *net_injection -= (dc.demand[dense] - sol.psh[dense]) + dc.shunt_conductance[dense];
    }
    for dense in 0..dc.n {
        let Some(row) = dc.bus_source_rows[dense] else {
            continue;
        };
        bus_voltage_angle[row] = sol.va[dense].to_degrees();
        bus_active_injection[row] = injection[dense] * base;
        if let (Some(output), Some(values)) = (&mut bus_marginal, &marginal) {
            output[row] = values[dense];
        }
    }

    let mut branch_from_active_flow = vec![f64::NAN; n_source_branches];
    let mut branch_to_active_flow = vec![f64::NAN; n_source_branches];
    let mut branch_from_limit_multiplier = vec![0.0; n_source_branches];
    let mut branch_to_limit_multiplier = vec![0.0; n_source_branches];
    for dense in 0..dc.m {
        // A synthetic row (three winding lowering) has no source branch; its
        // flow belongs to the transformer record, which the balanced
        // solution does not state.
        let Some(row) = dc.branch_source_rows[dense] else {
            continue;
        };
        let flow = sol.f[dense] * base;
        branch_from_active_flow[row] = flow;
        branch_to_active_flow[row] = -flow;
        branch_from_limit_multiplier[row] = sol.lam_ub[dense] / base;
        branch_to_limit_multiplier[row] = sol.lam_lb[dense] / base;
    }

    let mut generator_active_power = vec![f64::NAN; n_source_generators];
    for dense in 0..dc.k {
        let Some(row) = dc.gen_source_rows[dense] else {
            continue;
        };
        generator_active_power[row] = sol.pg[dense] * base;
    }

    let mut emitted = DcOpfSolution::new(
        instance,
        Termination::Converged,
        bus_voltage_angle,
        bus_active_injection,
        branch_from_active_flow,
        branch_to_active_flow,
        generator_active_power,
        sol.objective,
    )
    .map_err(|e| e.to_string())?;

    if let Some(bus_marginal) = bus_marginal {
        emitted = emitted
            .with_bus_active_power_marginals(bus_marginal)
            .map_err(|e| e.to_string())?
            .with_branch_thermal_limit_multipliers(
                branch_from_limit_multiplier,
                branch_to_limit_multiplier,
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(emitted.with_producer(producer.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_matpower;
    use crate::problem::dc_opf_cancellable;
    use powerio::{stored, PioModule, PioValue};

    #[test]
    fn the_emitted_solution_round_trips_with_marginals_and_bound_multipliers() {
        let mut net = parse_matpower(crate::model::CASE3).expect("parse");
        // State the binding rating on the instance itself. A portable solution
        // must never attach a solve of an amended workspace to the old instance.
        net.branches_mut()[0].rate_a = 36.0;
        let instance = Arc::new(DcOpfInstance::from_network(net.clone()).expect("instance"));
        let dc = DcNetwork::from_instance(&instance).expect("model");
        let sol = dc_opf_cancellable(&dc, None).expect("solve");

        let emitted = emit_dc_opf_solution(
            instance.clone(),
            &dc,
            &sol,
            format!(
                "tellegen {} (b-theta, kkt-implicit)",
                env!("CARGO_PKG_VERSION")
            ),
        )
        .expect("emit");

        // Keyed reads agree with the dense model through the source maps.
        let lmp = sol.nodal_marginal_values(dc.base_mva);
        for (dense, &expected) in lmp.iter().enumerate() {
            let id = powerio::BusId(dc.bus_ids[dense]);
            let marginal = emitted
                .bus_active_power_marginal(id)
                .expect("active demand marginal");
            assert!((marginal - expected).abs() < 1e-12);
            let angle = emitted.bus_voltage_angle(id).expect("angle");
            assert!((angle - sol.va[dense].to_degrees()).abs() < 1e-12);
        }
        assert!(
            emitted
                .branch_from_limit_multipliers()
                .expect("from multipliers")[0]
                + emitted
                    .branch_to_limit_multipliers()
                    .expect("to multipliers")[0]
                > 1e-3,
            "the binding line must carry a positive from-to dual"
        );
        assert_eq!(emitted.termination(), &Termination::Converged);
        assert!(emitted.producer().unwrap().starts_with("tellegen "));

        // The whole solution rides the stored module boundary intact.
        let module = PioModule::new(PioValue::DcOpfSolution(emitted));
        let text = stored::emit_module(&module).expect("write");
        let back = stored::read_module(&text).expect("read");
        let PioValue::DcOpfSolution(back) = back.value() else {
            panic!("expected dc_opf_solution");
        };
        assert!(back.bus_active_power_marginals().is_some());
        assert!(back.branch_from_limit_multipliers().is_some());
        assert!(back.branch_to_limit_multipliers().is_some());
        assert!((back.objective() - sol.objective).abs() < 1e-9);
    }

    #[test]
    fn feasibility_solution_omits_nonunique_economic_outputs() {
        let network = parse_matpower(crate::model::CASE3).expect("parse");
        let instance = Arc::new(
            DcOpfInstance::from_network(network)
                .expect("instance")
                .with_objective(powerio_prob::Objective::none()),
        );
        let model = DcNetwork::from_instance(&instance).expect("model");
        let solution = dc_opf_cancellable(&model, None).expect("solve");
        let emitted =
            emit_dc_opf_solution(instance, &model, &solution, "tellegen test").expect("emit");

        assert_eq!(emitted.objective(), 0.0);
        assert!(emitted.bus_active_power_marginals().is_none());
        assert!(emitted.branch_from_limit_multipliers().is_none());
        assert!(emitted.branch_to_limit_multipliers().is_none());
    }

    fn exact_objective(network: powerio::BalancedNetwork) -> f64 {
        let instance = DcOpfInstance::from_network(network).expect("finite difference instance");
        let mut model = DcNetwork::from_instance(&instance).expect("finite difference model");
        model.allow_shed = false;
        dc_opf_cancellable(&model, None)
            .expect("finite difference solve")
            .objective
    }

    #[test]
    fn emitted_demand_marginal_matches_a_fixed_active_set_central_difference() {
        let network = parse_matpower(crate::model::CASE3).expect("parse");
        let instance = Arc::new(DcOpfInstance::from_network(network.clone()).expect("instance"));
        let mut model = DcNetwork::from_instance(&instance).expect("model");
        model.allow_shed = false;
        let solution = dc_opf_cancellable(&model, None).expect("solve");
        let emitted =
            emit_dc_opf_solution(instance, &model, &solution, "tellegen test").expect("emit");

        // A 0.1 MW stencil stays inside the uncongested active set. The
        // derivative is with respect to added demand, so its sign is positive.
        let h_mw = 0.1;
        let demand_row = network
            .loads()
            .iter()
            .position(|load| load.bus == powerio::BusId(2))
            .expect("bus 2 load");
        let mut plus = network.clone();
        plus.loads_mut()[demand_row].p += h_mw;
        let mut minus = network;
        minus.loads_mut()[demand_row].p -= h_mw;
        let derivative = (exact_objective(plus) - exact_objective(minus)) / (2.0 * h_mw);
        let marginal = emitted
            .bus_active_power_marginal(powerio::BusId(2))
            .expect("emitted demand marginal");
        assert!(marginal > 0.0, "added demand must increase the objective");
        assert!(
            (marginal - derivative).abs() < 2e-4 * (1.0 + derivative.abs()),
            "emitted marginal {marginal} vs central difference {derivative}"
        );
    }

    #[test]
    fn emitted_shared_rating_multiplier_matches_a_fixed_active_set_central_difference() {
        let mut network = parse_matpower(crate::model::CASE3).expect("parse");
        network.branches_mut()[0].rate_a = 36.0;
        let instance = Arc::new(DcOpfInstance::from_network(network.clone()).expect("instance"));
        let mut model = DcNetwork::from_instance(&instance).expect("model");
        model.allow_shed = false;
        let solution = dc_opf_cancellable(&model, None).expect("solve");
        let emitted =
            emit_dc_opf_solution(instance, &model, &solution, "tellegen test").expect("emit");

        // Both perturbations leave branch 1 congested on the same directional
        // face. One shared rating relaxes both directional inequalities, so
        // dV/drating is the negative sum of their nonnegative multipliers.
        let h_mw = 0.05;
        let mut plus = network.clone();
        plus.branches_mut()[0].rate_a += h_mw;
        let mut minus = network;
        minus.branches_mut()[0].rate_a -= h_mw;
        let derivative = (exact_objective(plus) - exact_objective(minus)) / (2.0 * h_mw);
        let from = emitted
            .branch_from_limit_multipliers()
            .expect("from multipliers")[0];
        let to = emitted
            .branch_to_limit_multipliers()
            .expect("to multipliers")[0];
        let dual_derivative = -(from + to);
        assert!(from + to > 1e-3, "the stencil branch must remain binding");
        assert!(
            (dual_derivative - derivative).abs() < 2e-4 * (1.0 + derivative.abs()),
            "emitted derivative {dual_derivative} vs central difference {derivative}"
        );
    }
}
