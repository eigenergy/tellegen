//! Finite difference checks through Tellegen's public PowerIO boundary.
//!
//! DC and SOCWR use typed PowerIO OPF instances. AC power flow uses a typed
//! [`powerio::AcPfInstance`]. Dense models, formulations, KKT systems,
//! and linear algebra stay private to Tellegen.

use powerio::{AcOpfInstance, AcPfInstance, BalancedNetwork, BusId, DcOpfInstance, Load};
use serde::Deserialize;
use serde_json::Value;
use tellegen::{
    solve_ac_instance, solve_ac_pf_instance, solve_instance, Bound, CostTerm, Edits, ElementId,
    End, Mode, Operand, Parameter, Power, Problem, SensRequest, SolveRequest, VoltageKind, GB,
};

use crate::record::ParitySummary;

const ZERO_FLOOR: f64 = 5e-3;
const FLOOR_FRAC: f64 = 1e-2;
const MAX_COLS: usize = 6;

#[derive(Debug, Deserialize)]
struct WireRow {
    element: ElementId,
}

#[derive(Debug, Deserialize)]
struct WireCol {
    element: ElementId,
}

#[derive(Debug, Deserialize)]
struct WireMatrix {
    values: Vec<Vec<f64>>,
    rows: Vec<WireRow>,
    cols: Vec<WireCol>,
}

fn l2(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn sample_indices(len: usize, limit: usize) -> Vec<usize> {
    if len <= limit {
        return (0..len).collect();
    }
    (0..limit).map(|index| index * len / limit).collect()
}

fn soft_cell(operand: Operand, parameter: Parameter) -> bool {
    let soft_operand = matches!(
        operand,
        Operand::Voltage(VoltageKind::Squared)
            | Operand::Dispatch(Power::Reactive)
            | Operand::Flow {
                power: Power::Reactive,
                ..
            }
            | Operand::Price(Power::Reactive)
    );
    let soft_parameter = matches!(
        parameter,
        Parameter::SeriesAdmittance(GB::Conductance) | Parameter::ShuntAdmittance(GB::Conductance)
    );
    soft_operand || soft_parameter
}

fn record_relative_error(
    summary: &mut ParitySummary,
    operand: Operand,
    parameter: Parameter,
    relative_error: f64,
) {
    if soft_cell(operand, parameter) {
        summary.coupled_errs.push(relative_error);
    } else {
        summary.clean_errs.push(relative_error);
    }
}

fn matrix_max_abs_diff(left: &[Vec<f64>], right: &[Vec<f64>]) -> f64 {
    left.iter()
        .zip(right)
        .flat_map(|(left_row, right_row)| left_row.iter().zip(right_row))
        .map(|(left_value, right_value)| (left_value - right_value).abs())
        .fold(0.0, f64::max)
}

fn column(matrix: &WireMatrix, index: usize) -> Vec<f64> {
    matrix
        .values
        .iter()
        .filter_map(|row| row.get(index).copied())
        .collect()
}

fn request(
    formulation: Problem,
    cell: Option<(Operand, Parameter, &[usize], Mode)>,
) -> SolveRequest {
    SolveRequest {
        formulation,
        sensitivities: cell
            .map(|(operand, parameter, indices, mode)| SensRequest {
                operand,
                parameter,
                indices: Some(indices.to_vec()),
                mode,
            })
            .into_iter()
            .collect(),
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
enum AcSolver<'a> {
    PowerFlow,
    Socwr(&'a AcOpfInstance),
}

impl AcSolver<'_> {
    fn solve(
        self,
        network: &BalancedNetwork,
        cell: Option<(Operand, Parameter, &[usize], Mode)>,
    ) -> Result<Value, String> {
        let response = match self {
            Self::PowerFlow => {
                let instance = AcPfInstance::from_network(network.clone())
                    .map_err(|error| error.to_string())?;
                solve_ac_pf_instance(&instance, &request(Problem::AcPf, cell))?
            }
            Self::Socwr(template) => {
                let instance = template
                    .clone()
                    .with_network(network.clone())
                    .map_err(|error| error.to_string())?;
                solve_ac_instance(&instance, &request(Problem::Socwr, cell))?
            }
        };
        serde_json::to_value(response).map_err(|error| error.to_string())
    }
}

fn first_array<'a>(response: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    keys.iter().find_map(|key| response[*key].as_array())
}

fn parameter_len(response: &Value, parameter: Parameter) -> usize {
    match parameter {
        Parameter::Demand(_) | Parameter::ShuntAdmittance(_) | Parameter::VoltageBound(_) => {
            first_array(response, &["va", "vm", "w"]).map_or(0, Vec::len)
        }
        Parameter::Cost(_) | Parameter::GenBound { .. } => {
            response["dispatch"].as_array().map_or(0, Vec::len)
        }
        Parameter::LineLimit
        | Parameter::SeriesAdmittance(_)
        | Parameter::Transformer(_)
        | Parameter::Switching => response["flows"].as_array().map_or(0, Vec::len),
        _ => 0,
    }
}

fn array_scalar(response: &Value, key: &str, id_key: &str, id: usize) -> Option<f64> {
    response[key]
        .as_array()?
        .iter()
        .find(|entry| entry[id_key].as_u64() == Some(id as u64))?["value"]
        .as_f64()
}

fn read_operand(response: &Value, operand: Operand, element: ElementId) -> Option<f64> {
    match (operand, element) {
        (Operand::Price(Power::Active), ElementId::Bus(bus)) => {
            array_scalar(response, "lmp", "bus", bus)
        }
        (Operand::Price(Power::Reactive), ElementId::Bus(bus)) => {
            array_scalar(response, "lmp_q", "bus", bus)
        }
        (Operand::Voltage(VoltageKind::Magnitude), ElementId::Bus(bus)) => {
            array_scalar(response, "vm", "bus", bus)
        }
        (Operand::Voltage(VoltageKind::Angle), ElementId::Bus(bus)) => {
            array_scalar(response, "va", "bus", bus)
        }
        (Operand::Voltage(VoltageKind::Squared), ElementId::Bus(bus)) => {
            array_scalar(response, "w", "bus", bus)
        }
        (Operand::Dispatch(power), ElementId::Generator(generator)) => {
            let entry = response["dispatch"]
                .as_array()?
                .iter()
                .find(|entry| entry["gen"].as_u64() == Some(generator as u64))?;
            match power {
                Power::Active => entry["pg"].as_f64(),
                Power::Reactive => entry["qg"].as_f64(),
                _ => None,
            }
        }
        (Operand::Flow { power, end }, ElementId::Branch(branch)) => {
            let entry = response["flows"]
                .as_array()?
                .iter()
                .find(|entry| entry["branch"].as_u64() == Some(branch as u64))?;
            let key = match (power, end) {
                (Power::Active, End::From) => "pf",
                (Power::Active, End::To) => "pt",
                (Power::Reactive, End::From) => "qf",
                (Power::Reactive, End::To) => "qt",
                _ => return None,
            };
            entry[key].as_f64()
        }
        _ => None,
    }
}

fn source_power_delta(network: &BalancedNetwork, served_delta: f64) -> Option<f64> {
    if network.is_normalized() {
        let base = network.base_mva();
        (base.is_finite() && base > 0.0).then_some(served_delta / base)
    } else {
        Some(served_delta)
    }
}

fn perturb_network(
    network: &BalancedNetwork,
    parameter: Parameter,
    element: ElementId,
    served_delta: f64,
) -> Option<BalancedNetwork> {
    let mut perturbed = network.clone();
    match (parameter, element) {
        (Parameter::Demand(power), ElementId::Bus(bus)) => {
            let delta = source_power_delta(network, served_delta)?;
            let bus = BusId(bus);
            let row = perturbed
                .loads()
                .iter()
                .position(|load| load.in_service && load.bus == bus);
            let row = match row {
                Some(row) => row,
                None => {
                    perturbed.loads_mut().push(Load::new(bus, 0.0, 0.0));
                    perturbed.loads().len() - 1
                }
            };
            match power {
                Power::Active => perturbed.loads_mut()[row].p += delta,
                Power::Reactive => perturbed.loads_mut()[row].q += delta,
                _ => return None,
            }
        }
        (Parameter::LineLimit, ElementId::Branch(branch)) => {
            let delta = source_power_delta(network, served_delta)?;
            let row = branch.checked_sub(1)?;
            perturbed.branches_mut().get_mut(row)?.rate_a += delta;
        }
        (Parameter::GenBound { power, bound }, ElementId::Generator(generator)) => {
            let delta = source_power_delta(network, served_delta)?;
            let row = generator.checked_sub(1)?;
            let generator = perturbed.generators_mut().get_mut(row)?;
            match (power, bound) {
                (Power::Active, Bound::Max) => generator.pmax += delta,
                (Power::Active, Bound::Min) => generator.pmin += delta,
                (Power::Reactive, Bound::Max) => generator.qmax += delta,
                (Power::Reactive, Bound::Min) => generator.qmin += delta,
                _ => return None,
            }
        }
        (Parameter::VoltageBound(bound), ElementId::Bus(bus)) => {
            let bus = perturbed
                .buses_mut()
                .iter_mut()
                .find(|row| row.id.0 == bus)?;
            match bound {
                Bound::Max => bus.vmax += served_delta,
                Bound::Min => bus.vmin += served_delta,
                _ => return None,
            }
        }
        // Admittance, cost, transformer, and switching parameters need a
        // source level edit whose convention is not stated by this harness.
        // Their forward/adjoint check still runs; finite differences do not.
        _ => return None,
    }
    Some(perturbed)
}

fn finite_difference_step(parameter: Parameter) -> f64 {
    match parameter {
        Parameter::Demand(_) | Parameter::LineLimit | Parameter::GenBound { .. } => 1e-3,
        _ => 1e-4,
    }
}

const CONIC_CELLS: &[(Operand, Parameter)] = &[
    (
        Operand::Price(Power::Active),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Active),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Active),
        Parameter::Cost(CostTerm::Linear),
    ),
    (
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        },
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Reactive),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Voltage(VoltageKind::Squared),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Flow {
            power: Power::Reactive,
            end: End::From,
        },
        Parameter::Demand(Power::Reactive),
    ),
    (Operand::Price(Power::Active), Parameter::LineLimit),
    (
        Operand::Dispatch(Power::Active),
        Parameter::GenBound {
            power: Power::Active,
            bound: Bound::Max,
        },
    ),
    (
        Operand::Price(Power::Reactive),
        Parameter::Demand(Power::Active),
    ),
];

const AC_CELLS: &[(Operand, Parameter)] = &[
    (
        Operand::Voltage(VoltageKind::Angle),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Voltage(VoltageKind::Magnitude),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Voltage(VoltageKind::Magnitude),
        Parameter::Demand(Power::Active),
    ),
];

fn wire_parity(
    formulation: &str,
    network: &BalancedNetwork,
    cells: &[(Operand, Parameter)],
    solver: AcSolver<'_>,
) -> ParitySummary {
    let mut summary = ParitySummary::new(formulation);
    let base = match solver.solve(network, None) {
        Ok(response) => response,
        Err(error) => {
            summary
                .notes
                .push(format!("{formulation} base solve failed: {error}"));
            return summary;
        }
    };

    for &(operand, parameter) in cells {
        summary.cells_probed += 1;
        let indices = sample_indices(parameter_len(&base, parameter), MAX_COLS);
        if indices.is_empty() {
            continue;
        }
        let forward_response =
            match solver.solve(network, Some((operand, parameter, &indices, Mode::Forward))) {
                Ok(response) => response,
                Err(_) => continue,
            };
        let matrix: WireMatrix =
            match serde_json::from_value(forward_response["sensitivities"][0].clone()) {
                Ok(matrix) => matrix,
                Err(_) => continue,
            };
        summary.cells_supported += 1;

        if let Ok(adjoint_response) =
            solver.solve(network, Some((operand, parameter, &indices, Mode::Adjoint)))
        {
            if let Ok(adjoint) =
                serde_json::from_value::<WireMatrix>(adjoint_response["sensitivities"][0].clone())
            {
                summary.worst_adjoint_forward = summary
                    .worst_adjoint_forward
                    .max(matrix_max_abs_diff(&matrix.values, &adjoint.values));
            }
        }

        let columns: Vec<Vec<f64>> = (0..matrix.cols.len())
            .map(|index| column(&matrix, index))
            .collect();
        let norms: Vec<f64> = columns.iter().map(|values| l2(values)).collect();
        let largest = norms.iter().copied().fold(0.0, f64::max);
        if largest < ZERO_FLOOR {
            continue;
        }
        let floor = FLOOR_FRAC * largest;
        let step = finite_difference_step(parameter);

        for (column_index, analytic) in columns.iter().enumerate() {
            if norms[column_index] < floor {
                continue;
            }
            let element = matrix.cols[column_index].element;
            let (Some(plus), Some(minus)) = (
                perturb_network(network, parameter, element, step),
                perturb_network(network, parameter, element, -step),
            ) else {
                continue;
            };
            let (Ok(plus), Ok(minus)) = (solver.solve(&plus, None), solver.solve(&minus, None))
            else {
                continue;
            };
            let finite_difference: Option<Vec<f64>> = matrix
                .rows
                .iter()
                .map(|row| {
                    Some(
                        (read_operand(&plus, operand, row.element)?
                            - read_operand(&minus, operand, row.element)?)
                            / (2.0 * step),
                    )
                })
                .collect();
            let Some(finite_difference) = finite_difference else {
                continue;
            };
            let error: Vec<f64> = analytic
                .iter()
                .zip(&finite_difference)
                .map(|(analytic, finite)| analytic - finite)
                .collect();
            summary.fd_columns += 1;
            record_relative_error(
                &mut summary,
                operand,
                parameter,
                l2(&error) / norms[column_index],
            );
        }
    }
    summary
}

pub fn conic_parity(instance: &AcOpfInstance) -> ParitySummary {
    wire_parity(
        "socwr",
        instance.network(),
        CONIC_CELLS,
        AcSolver::Socwr(instance),
    )
}

pub fn ac_parity(instance: &AcPfInstance) -> ParitySummary {
    wire_parity("ac", instance.network(), AC_CELLS, AcSolver::PowerFlow)
}

/// DC dLMP/dd parity through the typed PowerIO instance entry.
pub fn dc_parity(instance: &DcOpfInstance) -> ParitySummary {
    let mut summary = ParitySummary::new("dc");
    summary.cells_probed = 1;
    let base = match solve_instance(instance, &SolveRequest::default()) {
        Ok(response) => response,
        Err(error) => {
            summary.notes.push(format!("dc base solve failed: {error}"));
            return summary;
        }
    };
    let indices = sample_indices(base.lmp.as_deref().map_or(0, <[_]>::len), MAX_COLS);
    if indices.is_empty() {
        return summary;
    }
    let request = |mode| SolveRequest {
        sensitivities: vec![SensRequest {
            operand: Operand::Price(Power::Active),
            parameter: Parameter::Demand(Power::Active),
            indices: Some(indices.clone()),
            mode,
        }],
        ..Default::default()
    };
    let forward = match solve_instance(instance, &request(Mode::Forward)) {
        Ok(response) => response,
        Err(error) => {
            summary
                .notes
                .push(format!("dc forward sensitivity failed: {error}"));
            return summary;
        }
    };
    let Some(matrix) = forward.sensitivities.first() else {
        return summary;
    };
    summary.cells_supported = 1;
    if let Ok(adjoint) = solve_instance(instance, &request(Mode::Adjoint)) {
        if let Some(adjoint) = adjoint.sensitivities.first() {
            summary.worst_adjoint_forward = matrix_max_abs_diff(&matrix.values, &adjoint.values);
        }
    }

    let columns: Vec<Vec<f64>> = (0..matrix.cols.len())
        .map(|column_index| matrix.values.iter().map(|row| row[column_index]).collect())
        .collect();
    let norms: Vec<f64> = columns.iter().map(|values| l2(values)).collect();
    let largest = norms.iter().copied().fold(0.0, f64::max);
    let floor = (FLOOR_FRAC * largest).max(1e-6);
    const STEP_MW: f64 = 1.0;

    for (column_index, analytic) in columns.iter().enumerate() {
        if norms[column_index] < floor {
            continue;
        }
        let ElementId::Bus(bus) = matrix.cols[column_index].element else {
            continue;
        };
        let shifted = |delta| SolveRequest {
            edits: Edits {
                deltas: [((bus as i64).into(), delta)].into_iter().collect(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (Ok(plus), Ok(minus)) = (
            solve_instance(instance, &shifted(STEP_MW)),
            solve_instance(instance, &shifted(-STEP_MW)),
        ) else {
            continue;
        };
        let marginal = |response: &tellegen::SolveResponse, bus| {
            response
                .lmp
                .as_deref()?
                .iter()
                .find(|value| value.bus == bus)
                .map(|value| value.value)
        };
        let finite_difference: Option<Vec<f64>> = matrix
            .rows
            .iter()
            .map(|row| {
                let ElementId::Bus(row_bus) = row.element else {
                    return None;
                };
                Some((marginal(&plus, row_bus)? - marginal(&minus, row_bus)?) / (2.0 * STEP_MW))
            })
            .collect();
        let Some(finite_difference) = finite_difference else {
            continue;
        };
        let error: Vec<f64> = analytic
            .iter()
            .zip(&finite_difference)
            .map(|(analytic, finite)| analytic - finite)
            .collect();
        summary.fd_columns += 1;
        summary.clean_errs.push(l2(&error) / norms[column_index]);
    }
    summary
}
