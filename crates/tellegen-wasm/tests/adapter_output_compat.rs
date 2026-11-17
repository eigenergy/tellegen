//! Compare the actual adapter output with payloads captured from the browser
//! WASM before response records were introduced. Keep explicit nulls and all
//! display fields; saved module source metadata is outside this comparison.
#![cfg(feature = "mc-pf")]

use serde_json::Value;

fn display_payload(text: &str) -> Value {
    let mut value: Value = serde_json::from_str(text).expect("adapter JSON");
    assert!(value["module_json"].is_string());
    value.as_object_mut().unwrap().remove("module_json");
    for location in ["topology", "view"] {
        if let Some(equipment) = value
            .get_mut(location)
            .and_then(|v| v.get_mut("equipment"))
            .and_then(Value::as_object_mut)
        {
            if let Some(drawing) = equipment.remove("node_breaker") {
                assert_eq!(
                    drawing,
                    serde_json::json!({
                        "stations": [], "nodes": [], "busbars": [], "switches": [], "connections": [], "terminals": [],
                    })
                );
            }
        }
    }
    if let Some(layer) = value.get_mut("geo_layer") {
        let mut parsed: Value = serde_json::from_str(layer.as_str().unwrap()).unwrap();
        assert!(parsed["powerio_geo"]["powerio_version"].is_string());
        parsed["powerio_geo"]["powerio_version"] = Value::String("dependency version".into());
        *layer = parsed;
    }
    value
}

#[test]
fn balanced_output_preserves_existing_display_fields() {
    let source = include_bytes!("../../../python/tellegen/mcp/examples/case3.m");
    let actual = tellegen_wasm::ingest_case(source, "matpower").expect("balanced ingest");
    let expected = display_payload(include_str!("fixtures/adapter-balanced-before.json"));
    assert_eq!(display_payload(&actual), expected);
}

#[test]
fn distribution_output_preserves_nulls_and_graph_fields() {
    let source = "new circuit.compat basekv=.4\nnew line.feeder phases=3 bus1=sourcebus.1.2.3 bus2=target.1.2.3 r1=.1 x1=.1 length=1\nnew load.l phases=3 bus1=target.1.2.3 conn=delta kv=.4 kw=3 kvar=1\n";
    let actual = tellegen_wasm::ingest_dist_case(source, "dss").expect("distribution ingest");
    let expected = display_payload(include_str!("fixtures/adapter-distribution-before.json"));
    assert!(expected
        .as_object()
        .unwrap()
        .contains_key("mc_pf_unavailable_reason"));
    assert!(expected["mc_pf_unavailable_reason"].is_null());
    let target = expected["graph"]["buses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bus| bus["id"] == "target")
        .unwrap();
    assert!(!target.as_object().unwrap().contains_key("neutral_terminal"));
    assert_eq!(display_payload(&actual), expected);
}
