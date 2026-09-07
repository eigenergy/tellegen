//! Raw BMOPF load-model preflight coverage.

#![cfg(feature = "mc-pf")]

use serde_json::{json, Map, Value};
use tellegen::validate_bmopf_json;

fn document(model: &str, extras: Value) -> String {
    let mut load = json!({
        "model": model,
        "p_nom": [1200.0, 800.0],
        "q_nom": [400.0, 300.0],
        "v_nom": [240.0, 240.0],
        "bus": "b",
        "terminal_map": ["a", "b", "n"],
        "configuration": "WYE"
    })
    .as_object()
    .unwrap()
    .clone();
    if let Value::Object(extra) = extras {
        load.extend(extra);
    }
    serde_json::to_string(&json!({ "load": { "ld": Value::Object(load) } })).unwrap()
}

fn scalar(value: f64) -> Value {
    json!([value])
}

fn zip_fields() -> Map<String, Value> {
    Map::from_iter([
        ("alpha_z".to_owned(), scalar(0.2)),
        ("alpha_i".to_owned(), scalar(0.3)),
        ("alpha_p".to_owned(), scalar(0.5)),
        ("beta_z".to_owned(), scalar(0.1)),
        ("beta_i".to_owned(), scalar(0.2)),
        ("beta_p".to_owned(), scalar(0.7)),
    ])
}

#[test]
fn accepts_all_supported_voltage_load_models() {
    let cases = [
        ("constant_power", json!({})),
        ("constant_current", json!({})),
        ("constant_impedance", json!({})),
        ("zip", Value::Object(zip_fields())),
        ("exponential", json!({ "gamma_p": [1.5], "gamma_q": [2.0] })),
    ];
    for (model, extras) in cases {
        let input = document(model, extras);
        assert!(
            validate_bmopf_json(&input).is_ok(),
            "{model} should validate"
        );
    }
}

#[test]
fn accepts_full_branch_coefficients_and_len_one_broadcasts() {
    let mut fields = zip_fields();
    fields.insert("alpha_z".to_owned(), json!([0.2, 0.1]));
    fields.insert("alpha_i".to_owned(), json!([0.3, 0.2]));
    fields.insert("alpha_p".to_owned(), json!([0.5, 0.7]));
    fields.insert("beta_z".to_owned(), json!([0.1, 0.2]));
    fields.insert("beta_i".to_owned(), json!([0.2, 0.1]));
    fields.insert("beta_p".to_owned(), json!([0.7, 0.7]));
    assert!(validate_bmopf_json(&document("zip", Value::Object(fields))).is_ok());

    assert!(validate_bmopf_json(&document(
        "exponential",
        json!({ "gamma_p": [1.5], "gamma_q": [2.0] })
    ))
    .is_ok());
}

#[test]
fn rejects_unknown_and_conflicting_model_fields() {
    let cases = [
        document("mystery", json!({})),
        document("constant_power", json!({ "alpha_z": [1.0] })),
        document("constant_power", json!({ "gamma_p": [1.0] })),
        document("zip", json!({ "gamma_p": [1.0], "gamma_q": [1.0] })),
        document("exponential", json!({ "alpha_z": [1.0] })),
    ];
    for input in cases {
        assert!(
            validate_bmopf_json(&input).is_err(),
            "conflicting input was accepted"
        );
    }
}

#[test]
fn rejects_incomplete_nonfinite_and_wrong_length_coefficients() {
    let nonfinite = document("zip", Value::Object(zip_fields()))
        .replace("\"alpha_z\":[0.2]", "\"alpha_z\":[1e999]");
    let cases = [
        document("zip", json!({ "alpha_z": [1.0] })),
        document("exponential", json!({ "gamma_p": [1.0] })),
        nonfinite,
        document(
            "zip",
            json!({
                "alpha_z": ["1"], "alpha_i": [0.0], "alpha_p": [0.0],
                "beta_z": [0.0], "beta_i": [0.0], "beta_p": [1.0]
            }),
        ),
        document(
            "zip",
            json!({
                "alpha_z": [null], "alpha_i": [0.0], "alpha_p": [0.0],
                "beta_z": [0.0], "beta_i": [0.0], "beta_p": [1.0]
            }),
        ),
        document(
            "zip",
            json!({
                "alpha_z": [0.2, 0.2, 0.2], "alpha_i": [0.3], "alpha_p": [0.5],
                "beta_z": [0.1], "beta_i": [0.2], "beta_p": [0.7]
            }),
        ),
    ];
    for input in cases {
        assert!(
            validate_bmopf_json(&input).is_err(),
            "invalid coefficient input was accepted"
        );
    }
}
