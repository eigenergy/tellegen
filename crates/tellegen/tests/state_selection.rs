//! Borrowed time series entries as a consumer solve path: a PowerIO time
//! series feeds tellegen one entry at a time without serializing or cloning a
//! complete static module. Three demand states solve per entry over one
//! shared element identity set, an index past the end is `None`, and a static
//! value is not a collection.

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
fn a_series_entry_solves_without_an_exported_module() {
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
    let value = PioValue::from(series);
    assert_eq!(
        value.type_name(),
        "powerio.TimeSeries<powerio.BalancedNetwork>"
    );
    let PioValue::TimeSeries(series) = &value else {
        panic!("a time series value");
    };
    assert_eq!(series.len(), 3);
    assert_eq!(series.time_points()[2].label(), "peak");

    // Solve each entry through the borrow; costs must rise with the demand,
    // which pins that each index names its own entry.
    let mut objectives = Vec::new();
    for position in 0..3 {
        let PioValue::BalancedNetwork(network) = series.get(position).expect("entry") else {
            panic!("a network series holds networks");
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

    // An index past the end is absent, never a silent first entry.
    assert!(series.get(3).is_none());

    let static_value = PioValue::BalancedNetwork(scaled_network(1.0));
    assert!(!matches!(static_value, PioValue::TimeSeries(_)));
    assert_eq!(static_value.type_name(), "powerio.BalancedNetwork");
}
