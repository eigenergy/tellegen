//! Borrowed typed state selection as a consumer solve path: PowerIO's
//! `select` surface feeds tellegen without serializing or cloning a
//! complete static module. A series of three demand states solves per
//! selected entry over one shared element identity set, and a static value
//! or a bad selector refuses with a coded error instead of a guess.

use powerio::select::{list_states, select_state, SelectedState, StateInventory, StateSelector};
use powerio::{PioValue, TimePoint, TimeSeries};

/// CASE3-shaped in-memory network with the load scaled by `factor`.
fn scaled_network(factor: f64) -> powerio::BalancedNetwork {
    use powerio::{Branch, Bus, BusId, BusType, GenCost, Generator, Load};
    let mut net = powerio::BalancedNetwork::in_memory(
        "series-entry",
        100.0,
        vec![
            Bus::new(BusId(1), BusType::Ref, 230.0),
            Bus::new(BusId(2), BusType::Pq, 230.0),
            Bus::new(BusId(3), BusType::Pq, 230.0),
        ],
        vec![
            Branch::new(BusId(1), BusId(2), 0.01, 0.1),
            Branch::new(BusId(2), BusId(3), 0.01, 0.1),
            Branch::new(BusId(1), BusId(3), 0.01, 0.1),
        ],
    );
    net.loads_mut()
        .push(Load::new(BusId(2), 90.0 * factor, 0.0));
    let mut generator = Generator::new(BusId(1));
    generator.pmax = 200.0;
    generator.cost = Some(GenCost::new(2, 0.0, 0.0, vec![0.02, 11.0, 3.0]));
    net.generators_mut().push(generator);
    let mut peaker = Generator::new(BusId(3));
    peaker.pmax = 150.0;
    peaker.cost = Some(GenCost::new(2, 0.0, 0.0, vec![0.03, 18.0, 0.0]));
    net.generators_mut().push(peaker);
    net
}

#[test]
fn a_selected_series_state_solves_without_an_exported_module() {
    let series = TimeSeries::new(
        vec![
            TimePoint::new("valley", None).unwrap(),
            TimePoint::new("shoulder", None).unwrap(),
            TimePoint::new("peak", None).unwrap(),
        ],
        vec![
            scaled_network(0.5),
            scaled_network(1.0),
            scaled_network(1.5),
        ],
    )
    .unwrap();
    let value = PioValue::BalancedNetworkTimeSeries(series);

    let StateInventory::TimePoints(points) = list_states(&value).expect("inventory") else {
        panic!("a time series inventories time points");
    };
    assert_eq!(points.len(), 3);

    // Solve each selected state through the borrow; costs must rise with
    // the demand, which pins that each selection names its own entry.
    let mut objectives = Vec::new();
    for position in 0..3 {
        let selected = select_state(&value, StateSelector::TimePosition(position)).expect("select");
        let SelectedState::BalancedNetwork(network) = selected else {
            panic!("a network series selects stored networks");
        };
        let instance =
            powerio::DcOpfInstance::from_network(network.clone()).expect("problem instance");
        let solution =
            tellegen::solve_instance(&instance, &tellegen::SolveRequest::default()).expect("solve");
        objectives.push(solution.objective.expect("DC OPF objective"));
    }
    assert!(
        objectives[0] < objectives[1] && objectives[1] < objectives[2],
        "objectives must rise with demand: {objectives:?}"
    );

    // Selection refusals are coded, never a silent first entry.
    let out_of_range = select_state(&value, StateSelector::TimePosition(3)).unwrap_err();
    assert!(out_of_range.info().is_some(), "{out_of_range}");
    let wrong_axis = select_state(&value, StateSelector::Scenario("peak")).unwrap_err();
    assert!(wrong_axis.info().is_some(), "{wrong_axis}");

    let static_value = PioValue::BalancedNetwork(scaled_network(1.0));
    let refused = list_states(&static_value).unwrap_err();
    assert!(refused.info().is_some(), "{refused}");
}
