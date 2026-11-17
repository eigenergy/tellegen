//! AC OPF sensitivities against central finite differences of full solves,
//! through the public solve request and in the served units of the response.
use super::super::Mode;
use super::*;
use crate::api::{solve_ac_opf_instance, Problem, SensRequest, SolveRequest, SolveResponse};
use powerio::AcOpfInstance;

/// A small MATPOWER case kept as numbers, so a test can move one parameter in
/// served units and solve again.
#[derive(Clone)]
struct Fixture {
    /// `[type, pd MW, qd MVAr, vmax, vmin]` per bus.
    buses: Vec<[f64; 5]>,
    /// `[bus, qmax, qmin, pmax, pmin, c2, c1]` per generator.
    gens: Vec<[f64; 7]>,
    /// `[from, to, r, x, b, rate MVA]` per branch; a zero rate is unlimited.
    branches: Vec<[f64; 6]>,
}

impl Fixture {
    fn text(&self) -> String {
        let mut text = String::from(
            "function mpc = fixture\nmpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [\n",
        );
        for (i, b) in self.buses.iter().enumerate() {
            text += &format!(
                " {} {} {} {} 0 0 1 1 0 230 1 {} {};\n",
                i + 1,
                b[0],
                b[1],
                b[2],
                b[3],
                b[4]
            );
        }
        text += "];\nmpc.gen = [\n";
        for g in &self.gens {
            text += &format!(
                " {} 0 0 {} {} 1 100 1 {} {} 0 0 0 0 0 0 0 0 0 0 0;\n",
                g[0], g[1], g[2], g[3], g[4]
            );
        }
        text += "];\nmpc.branch = [\n";
        for b in &self.branches {
            text += &format!(
                " {} {} {} {} {} {} {} {} 0 0 1 -360 360;\n",
                b[0], b[1], b[2], b[3], b[4], b[5], b[5], b[5]
            );
        }
        text += "];\nmpc.gencost = [\n";
        for g in &self.gens {
            text += &format!(" 2 0 0 3 {} {} 0;\n", g[5], g[6]);
        }
        text + "];\n"
    }

    fn request(cells: &[SensRequest]) -> SolveRequest {
        SolveRequest {
            formulation: Problem::Acopf,
            sensitivities: cells.to_vec(),
            ..Default::default()
        }
    }

    fn solve(&self, cells: &[SensRequest]) -> Result<SolveResponse, String> {
        let network = crate::model::parse_matpower(&self.text())?;
        let instance = AcOpfInstance::from_network(network).map_err(|e| e.to_string())?;
        solve_ac_opf_instance(&instance, &Self::request(cells))
    }

    /// Move parameter `p` of element `index` by `delta` in served units.
    fn bump(&mut self, p: Parameter, index: usize, delta: f64) {
        match p {
            Parameter::Demand(Power::Active) => self.buses[index][1] += delta,
            Parameter::Demand(Power::Reactive) => self.buses[index][2] += delta,
            Parameter::LineLimit => self.branches[index][5] += delta,
            Parameter::GenBound { power, bound } => {
                let field = match (power, bound) {
                    (Power::Reactive, Bound::Max) => 1,
                    (Power::Reactive, Bound::Min) => 2,
                    (Power::Active, Bound::Max) => 3,
                    (Power::Active, Bound::Min) => 4,
                };
                self.gens[index][field] += delta;
            }
            other => unreachable!("no fixture field for {other:?}"),
        }
    }
}

/// The three-bus fixture of the engine's other tests: generators at buses 1
/// and 3, a 90 MW + 30 MVAr load at bus 2, three equal lines.
fn case3() -> Fixture {
    Fixture {
        buses: vec![
            [3., 0., 0., 1.1, 0.9],
            [1., 90., 30., 1.1, 0.9],
            [2., 0., 0., 1.1, 0.9],
        ],
        gens: vec![
            [1., 300., -300., 250., 10., 0.11, 5.],
            [3., 300., -300., 270., 10., 0.085, 1.2],
        ],
        branches: vec![
            [1., 2., 0.01, 0.1, 0., 250.],
            [1., 3., 0.01, 0.1, 0., 250.],
            [2., 3., 0.01, 0.1, 0., 250.],
        ],
    }
}

/// The three-bus fixture with line 2-3 rated below its unconstrained flow.
fn case3_line_limit() -> Fixture {
    let mut fixture = case3();
    fixture.branches[2][5] = 40.;
    fixture
}

/// The three-bus fixture with generator 1 held at a raised lower active limit
/// and generator 2 at a lowered upper reactive limit.
fn case3_generator_bounds() -> Fixture {
    let mut fixture = case3();
    fixture.gens[0][4] = 35.;
    fixture.gens[1][1] = 10.;
    fixture
}

/// Four buses in a ring with a chord, two loads, a voltage ceiling of 1.05
/// and a tight rating on line 1-2.
fn case4() -> Fixture {
    Fixture {
        buses: vec![
            [3., 0., 0., 1.05, 0.95],
            [1., 90., 30., 1.05, 0.95],
            [2., 0., 0., 1.05, 0.95],
            [1., 40., 15., 1.05, 0.95],
        ],
        gens: vec![
            [1., 300., -300., 250., 10., 0.11, 5.],
            [3., 300., -300., 270., 10., 0.085, 1.2],
        ],
        branches: vec![
            [1., 2., 0.01, 0.1, 0.02, 45.],
            [1., 3., 0.01, 0.1, 0.02, 250.],
            [2., 3., 0.012, 0.11, 0.02, 250.],
            [2., 4., 0.02, 0.15, 0.01, 0.],
            [3., 4., 0.015, 0.12, 0.01, 250.],
        ],
    }
}

/// Two copies of the three-bus fixture as separate islands, each with its own
/// reference angle; the second serves 60 MW + 20 MVAr.
fn twin() -> Fixture {
    let one = case3();
    let mut fixture = one.clone();
    for bus in &one.buses {
        fixture.buses.push(*bus);
    }
    fixture.buses[4][1] = 60.;
    fixture.buses[4][2] = 20.;
    for gen in &one.gens {
        let mut gen = *gen;
        gen[0] += 3.;
        fixture.gens.push(gen);
    }
    for branch in &one.branches {
        let mut branch = *branch;
        branch[0] += 3.;
        branch[1] += 3.;
        fixture.branches.push(branch);
    }
    fixture
}

const OPERANDS: [Operand; 10] = [
    Operand::Price(Power::Active),
    Operand::Price(Power::Reactive),
    Operand::Dispatch(Power::Active),
    Operand::Dispatch(Power::Reactive),
    Operand::Voltage(VoltageKind::Magnitude),
    Operand::Voltage(VoltageKind::Angle),
    Operand::Flow {
        power: Power::Active,
        end: End::From,
    },
    Operand::Flow {
        power: Power::Active,
        end: End::To,
    },
    Operand::Flow {
        power: Power::Reactive,
        end: End::From,
    },
    Operand::Flow {
        power: Power::Reactive,
        end: End::To,
    },
];

const PARAMETERS: [Parameter; 7] = [
    Parameter::Demand(Power::Active),
    Parameter::Demand(Power::Reactive),
    Parameter::LineLimit,
    Parameter::GenBound {
        power: Power::Active,
        bound: Bound::Min,
    },
    Parameter::GenBound {
        power: Power::Active,
        bound: Bound::Max,
    },
    Parameter::GenBound {
        power: Power::Reactive,
        bound: Bound::Min,
    },
    Parameter::GenBound {
        power: Power::Reactive,
        bound: Bound::Max,
    },
];

/// The operand's values in the served units of the response.
fn read(response: &SolveResponse, operand: Operand) -> Vec<f64> {
    let bus = |values: &Option<Vec<crate::api::BusScalar>>| -> Vec<f64> {
        values
            .as_ref()
            .expect("bus block")
            .iter()
            .map(|entry| entry.value)
            .collect()
    };
    let flows = response.flows.as_ref().expect("flows");
    let dispatch = response.dispatch.as_ref().expect("dispatch");
    match operand {
        Operand::Price(Power::Active) => bus(&response.lmp),
        Operand::Price(Power::Reactive) => bus(&response.lmp_q),
        Operand::Voltage(VoltageKind::Magnitude) => bus(&response.vm),
        Operand::Voltage(VoltageKind::Angle) => bus(&response.va),
        Operand::Dispatch(Power::Active) => dispatch.iter().map(|g| g.pg).collect(),
        Operand::Dispatch(Power::Reactive) => dispatch.iter().map(|g| g.qg.expect("qg")).collect(),
        Operand::Flow { power, end } => flows
            .iter()
            .map(|flow| match (power, end) {
                (Power::Active, End::From) => flow.pf,
                (Power::Active, End::To) => flow.pt.expect("pt"),
                (Power::Reactive, End::From) => flow.qf.expect("qf"),
                (Power::Reactive, End::To) => flow.qt.expect("qt"),
            })
            .collect(),
        other => unreachable!("{other:?}"),
    }
}

fn cells() -> Vec<SensRequest> {
    OPERANDS
        .iter()
        .flat_map(|&operand| {
            PARAMETERS.iter().map(move |&parameter| SensRequest {
                operand,
                parameter,
                indices: None,
                mode: Mode::Auto,
            })
        })
        .collect()
}

fn parameter_count(fixture: &Fixture, parameter: Parameter) -> usize {
    match parameter.axis() {
        Axis::Bus => fixture.buses.len(),
        Axis::Branch => fixture.branches.len(),
        Axis::Generator => fixture.gens.len(),
        Axis::Constraint => 0,
    }
}

/// Compare every entry of every supported cell against a central finite
/// difference of two full solves. Returns the number of nonzero columns per
/// parameter, in [`PARAMETERS`] order, so a test can require that a limit
/// really binds.
fn check_against_finite_differences(fixture: &Fixture, step: f64) -> Vec<usize> {
    let response = fixture.solve(&cells()).expect("solve with sensitivities");
    assert_eq!(
        response.sensitivities.len(),
        OPERANDS.len() * PARAMETERS.len()
    );
    let mut nonzero_columns = vec![0; PARAMETERS.len()];
    let mut worst: f64 = 0.;
    for (p, &parameter) in PARAMETERS.iter().enumerate() {
        for column in 0..parameter_count(fixture, parameter) {
            // An unlimited branch stays unlimited; its column must be zero.
            let unlimited = parameter == Parameter::LineLimit && fixture.branches[column][5] == 0.;
            let difference: Vec<Vec<f64>> = if unlimited {
                OPERANDS
                    .iter()
                    .map(|&o| vec![0.; read(&response, o).len()])
                    .collect()
            } else {
                let (mut plus, mut minus) = (fixture.clone(), fixture.clone());
                plus.bump(parameter, column, step);
                minus.bump(parameter, column, -step);
                let plus = plus.solve(&[]).expect("plus solve");
                let minus = minus.solve(&[]).expect("minus solve");
                OPERANDS
                    .iter()
                    .map(|&o| {
                        read(&plus, o)
                            .iter()
                            .zip(read(&minus, o))
                            .map(|(a, b)| (a - b) / (2. * step))
                            .collect()
                    })
                    .collect()
            };
            let mut column_nonzero = false;
            for (o, &operand) in OPERANDS.iter().enumerate() {
                let matrix = &response.sensitivities[o * PARAMETERS.len() + p];
                assert_eq!(matrix.rows.len(), difference[o].len());
                assert_eq!(matrix.cols[column].index, column);
                let scale = read(&response, operand)
                    .iter()
                    .fold(1.0_f64, |m, v| m.max(v.abs()));
                for (row, &expected) in difference[o].iter().enumerate() {
                    let actual = matrix.values[row][column];
                    let error = (actual - expected).abs();
                    let allowed = 2e-5 * scale + 2e-3 * expected.abs();
                    worst = worst.max(error / allowed);
                    assert!(
                        error <= allowed,
                        "d({operand:?}[{row}])/d({parameter:?}[{column}]) = {actual:e}, finite difference {expected:e}"
                    );
                    column_nonzero |= actual.abs() > 1e-9;
                }
            }
            nonzero_columns[p] += usize::from(column_nonzero);
        }
    }
    eprintln!("worst finite-difference error ratio {worst:.3}");
    nonzero_columns
}

fn nonzero(counts: &[usize], parameter: Parameter) -> usize {
    counts[PARAMETERS.iter().position(|&p| p == parameter).unwrap()]
}

const STEP: f64 = 0.05;

const P_DEMAND: Parameter = Parameter::Demand(Power::Active);
const P_MIN: Parameter = Parameter::GenBound {
    power: Power::Active,
    bound: Bound::Min,
};
const P_MAX: Parameter = Parameter::GenBound {
    power: Power::Active,
    bound: Bound::Max,
};
const Q_MAX: Parameter = Parameter::GenBound {
    power: Power::Reactive,
    bound: Bound::Max,
};
const VM: Operand = Operand::Voltage(VoltageKind::Magnitude);
const PRICE: Operand = Operand::Price(Power::Active);

fn cell(operand: Operand, parameter: Parameter, indices: Option<Vec<usize>>) -> SensRequest {
    SensRequest {
        operand,
        parameter,
        indices,
        mode: Mode::Auto,
    }
}

#[test]
fn three_bus_sensitivities_match_finite_differences_with_binding_voltage_bounds() {
    let fixture = case3();
    let response = fixture.solve(&[]).unwrap();
    // Buses 1 and 3 sit at the 1.1 ceiling.
    let vm = read(&response, VM);
    assert!((vm[0] - 1.1).abs() < 1e-6 && (vm[2] - 1.1).abs() < 1e-6 && vm[1] < 1.09);
    let counts = check_against_finite_differences(&fixture, STEP);
    assert_eq!(nonzero(&counts, P_DEMAND), 3);
    assert_eq!(nonzero(&counts, Parameter::LineLimit), 0);
}

#[test]
fn binding_line_limit_sensitivities_match_finite_differences() {
    let fixture = case3_line_limit();
    let response = fixture.solve(&[]).unwrap();
    let flow = &response.flows.as_ref().unwrap()[2];
    let apparent = flow
        .pf
        .hypot(flow.qf.unwrap())
        .max(flow.pt.unwrap().hypot(flow.qt.unwrap()));
    assert!(
        (apparent - 40.).abs() < 1e-3,
        "line 2-3 carries {apparent} MVA"
    );
    // The congested line separates the prices, and bus 2 sits on its floor.
    let lmp = read(&response, PRICE);
    assert!(lmp[1] > lmp[0] && lmp[0] > lmp[2], "{lmp:?}");
    assert!((read(&response, VM)[1] - 0.9).abs() < 1e-6);
    let counts = check_against_finite_differences(&fixture, STEP);
    assert_eq!(nonzero(&counts, Parameter::LineLimit), 1);
}

#[test]
fn binding_generator_limit_sensitivities_match_finite_differences() {
    let fixture = case3_generator_bounds();
    let counts = check_against_finite_differences(&fixture, STEP);
    assert_eq!(nonzero(&counts, P_MIN), 1);
    assert_eq!(nonzero(&counts, Q_MAX), 1);
    assert_eq!(nonzero(&counts, P_MAX), 0);
}

#[test]
fn four_bus_sensitivities_match_finite_differences() {
    let fixture = case4();
    let response = fixture.solve(&[]).unwrap();
    assert!((read(&response, VM)[2] - 1.05).abs() < 1e-6);
    let counts = check_against_finite_differences(&fixture, STEP);
    assert_eq!(nonzero(&counts, P_DEMAND), 4);
    // Line 1-2 binds; the unrated line 2-4 has a zero column.
    assert_eq!(nonzero(&counts, Parameter::LineLimit), 1);
}

#[test]
fn two_island_sensitivities_match_finite_differences() {
    let fixture = twin();
    let response = fixture.solve(&[cell(PRICE, P_DEMAND, None)]).unwrap();
    // Demand in one island does not move a price in the other.
    let matrix = &response.sensitivities[0];
    for row in 0..6 {
        for column in 0..6 {
            if (row < 3) != (column < 3) {
                assert_eq!(matrix.values[row][column], 0.0);
            }
        }
    }
    check_against_finite_differences(&fixture, STEP);
}

#[test]
fn forward_and_adjoint_agree_and_cells_carry_ids_and_units() {
    let fixture = case4();
    let response = fixture
        .solve(&[
            SensRequest {
                mode: Mode::Forward,
                ..cell(PRICE, P_DEMAND, None)
            },
            SensRequest {
                mode: Mode::Adjoint,
                ..cell(PRICE, P_DEMAND, None)
            },
            cell(
                Operand::Flow {
                    power: Power::Active,
                    end: End::From,
                },
                Parameter::LineLimit,
                Some(vec![0]),
            ),
        ])
        .unwrap();
    let (forward, adjoint) = (&response.sensitivities[0], &response.sensitivities[1]);
    for (a, b) in forward
        .values
        .iter()
        .flatten()
        .zip(adjoint.values.iter().flatten())
    {
        assert!((a - b).abs() <= 1e-9 * (1. + a.abs()), "{a} vs {b}");
    }
    assert_eq!(forward.units, "(objective_unit/MW)/MW");
    assert!(matches!(forward.rows[3].element, ElementId::Bus(4)));
    let flow = &response.sensitivities[2];
    assert_eq!(flow.units, "(MW)/MVA");
    assert_eq!(flow.cols.len(), 1);
    assert!(matches!(flow.cols[0].element, ElementId::Branch(1)));
    assert_eq!(flow.rows.len(), 5);
    // Relaxing the binding rating of line 1-2 lets it carry more.
    assert!(flow.values[0][0] > 0.5, "{}", flow.values[0][0]);
}

/// On a lossless network with wide voltage limits and no congestion both
/// formulations dispatch the same generation, so the price at every bus is
/// the one marginal cost, in the same units and with the same sign.
#[test]
fn prices_match_dc_opf_on_a_lossless_uncongested_case() {
    let mut fixture = case3();
    for bus in &mut fixture.buses {
        bus[2] = 0.;
        bus[3] = 1.5;
        bus[4] = 0.5;
    }
    for branch in &mut fixture.branches {
        branch[2] = 0.;
    }
    let ac = fixture.solve(&[]).unwrap();
    let network = crate::model::parse_matpower(&fixture.text()).unwrap();
    let dc = crate::api::solve_network(
        &network,
        &SolveRequest {
            formulation: Problem::DcOpf,
            ..Default::default()
        },
    )
    .unwrap();
    let (ac_price, dc_price) = (read(&ac, PRICE), read(&dc, PRICE));
    assert_eq!(ac_price.len(), 3);
    for (ac_value, dc_value) in ac_price.iter().zip(&dc_price) {
        assert!(*dc_value > 5.0, "{dc_price:?}");
        assert!(
            (ac_value - dc_value).abs() < 1e-4,
            "AC {ac_price:?} vs DC {dc_price:?}"
        );
    }
    // The price is the marginal cost of the unconstrained generators.
    let pg = read(&ac, Operand::Dispatch(Power::Active));
    assert!((ac_price[0] - (2. * 0.11 * pg[0] + 5.)).abs() < 1e-4);
    // Reactive demand is free where no voltage or reactive limit binds.
    assert!(read(&ac, Operand::Price(Power::Reactive))
        .iter()
        .all(|price| price.abs() < 1e-5));
}

#[test]
fn a_limit_that_binds_with_a_zero_multiplier_is_refused() {
    // Rate line 2-3 at exactly the flow it carries unconstrained: the limit
    // touches the solution without shaping it.
    let free = {
        let mut fixture = case3();
        fixture.branches[2][5] = 0.;
        fixture
    };
    let flow = &free.solve(&[]).unwrap().flows.unwrap()[2];
    let apparent = flow
        .pf
        .hypot(flow.qf.unwrap())
        .max(flow.pt.unwrap().hypot(flow.qt.unwrap()));
    let mut fixture = case3();
    fixture.branches[2][5] = apparent;
    // The solve itself is fine and still publishes prices.
    assert!(fixture.solve(&[]).unwrap().lmp.is_some());
    let error = fixture
        .solve(&[cell(PRICE, P_DEMAND, None)])
        .expect_err("a weakly active limit has no two-sided derivative");
    assert!(
        error.contains("sensitivity is not defined at this solution")
            && error.contains("thermal limit of branch column 2"),
        "{error}"
    );
}

#[test]
fn unsupported_and_malformed_requests_are_refused() {
    let fixture = case3();
    let unsupported = fixture
        .solve(&[cell(Operand::Voltage(VoltageKind::Squared), P_DEMAND, None)])
        .unwrap_err();
    assert!(
        unsupported.contains("acopf does not support"),
        "{unsupported}"
    );
    let unsupported = fixture
        .solve(&[cell(PRICE, Parameter::Switching, None)])
        .unwrap_err();
    assert!(
        unsupported.contains("acopf does not support"),
        "{unsupported}"
    );
    let range = fixture
        .solve(&[cell(PRICE, P_DEMAND, Some(vec![3]))])
        .unwrap_err();
    assert!(range.contains("out of range"), "{range}");

    // A generator with equal limits has no derivative in one limit alone,
    // and is still differentiable in everything else.
    let mut fixed = case3();
    fixed.gens[0][3] = 35.;
    fixed.gens[0][4] = 35.;
    let error = fixed
        .solve(&[cell(PRICE, P_MAX, Some(vec![0]))])
        .unwrap_err();
    assert!(error.contains("equal lower and upper bounds"), "{error}");
    let response = fixed
        .solve(&[
            cell(PRICE, P_MAX, Some(vec![1])),
            cell(PRICE, P_DEMAND, None),
        ])
        .unwrap();
    assert_eq!(response.sensitivities.len(), 2);
}
