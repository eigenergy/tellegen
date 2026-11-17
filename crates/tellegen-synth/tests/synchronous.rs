use std::sync::Arc;

use tellegen_synth::{GenerationReport, Generator, Places, Region, SynthRequest};

#[test]
fn areas_survive_export_and_ac_branches_never_cross_them() {
    let places = Places::parse(
        br#"[
        {"n":"Los Angeles","lo":-118.2,"la":34.1,"p":4000000,"c":"USA"},
        {"n":"Austin","lo":-97.7,"la":30.3,"p":1000000,"c":"USA"},
        {"n":"New York","lo":-74.0,"la":40.7,"p":8000000,"c":"USA"}
    ]"#,
    )
    .unwrap();
    let mut request = SynthRequest::new(Region::bbox(-125.0, 25.0, -68.0, 49.0, "Selected box"));
    request.target_bus_count = Some(60);
    request.demand = tellegen_synth::DemandSpec::TotalMw { mw: 1000.0 };
    let mut session = Generator::new(request, Arc::new(places), None).unwrap();
    while !session.is_done() {
        session.step().unwrap();
    }
    let output = session.report_output();
    assert_eq!(output.synchronous_areas.len(), 3);
    assert_eq!(output.planned_hvdc_links.len(), 2);
    let encoded = serde_json::to_value(&output).unwrap();
    let legacy: GenerationReport = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(&legacy, session.report());
    let module = session.module_json().unwrap();
    let reopened: serde_json::Value = serde_json::from_str(&module).unwrap();
    let stored = &reopened["extensions"]["tellegen.synchronous_areas"];
    assert_eq!(stored["areas"], encoded["synchronous_areas"]);
    assert_eq!(stored["links"], encoded["planned_hvdc_links"]);
    let mut by_bus = std::collections::BTreeMap::new();
    for area in output.synchronous_areas {
        for &id in &area.bus_ids {
            assert!(by_bus.insert(id, area.id).is_none());
        }
    }
    let network = session.network().unwrap();
    assert_eq!(by_bus.len(), network.buses().len());
    for branch in network.branches() {
        assert_eq!(by_bus[&branch.from.0], by_bus[&branch.to.0]);
    }
    for link in network.hvdc() {
        assert_ne!(by_bus[&link.from.0], by_bus[&link.to.0]);
        assert_eq!(link.pf, link.pt);
        assert!(link.pf >= 0.0 && link.pf <= link.pmax);
        assert_eq!(link.extras["tellegen_synth"]["source"], "synthetic");
    }
}
