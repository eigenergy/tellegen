//! Explicit snapshots using equipment settings already written in the input.
use powerio::{PioModule, PioValue};

pub const FIXED_CONTROLS_EXTENSION: &str = "tellegen.fixed_controls";

/// Keep written taps and capacitor states without running their controls.
/// The input is unchanged. Original control records and source files remain in
/// the returned module; all other calculation restrictions still apply.
pub fn prepare_mc_fixed_controls(
    module: &PioModule<PioValue>,
) -> Result<PioModule<PioValue>, String> {
    if module.extensions().contains_key(FIXED_CONTROLS_EXTENSION) {
        return Err("This case already uses written equipment settings".to_owned());
    }
    let mut prepared = module.clone();
    let PioValue::MulticonductorNetwork(network) = prepared.value_mut() else {
        return Err("Written equipment settings require a multiconductor network".to_owned());
    };
    let is_control = |class: &str| {
        class.eq_ignore_ascii_case("regcontrol") || class.eq_ignore_ascii_case("capcontrol")
    };
    let controls: Vec<_> = network
        .untyped_objects()
        .iter()
        .filter(|object| is_control(&object.class))
        .cloned()
        .collect();
    if controls.is_empty() {
        return Err("This case has no regulator or capacitor controls to hold fixed".to_owned());
    }
    for control in &controls {
        if control.class.eq_ignore_ascii_case("regcontrol") {
            let target = control.props.iter().rev().find_map(|(key, value)| {
                key.as_deref()
                    .filter(|key| key.eq_ignore_ascii_case("transformer"))
                    .map(|_| value.trim_matches(['\"', '\'']))
            });
            if !target.is_some_and(|target| {
                network
                    .transformers()
                    .iter()
                    .any(|transformer| transformer.name.eq_ignore_ascii_case(target))
            }) {
                return Err(format!(
                    "Regulator control {} has no supported transformer target",
                    control.name
                ));
            }
        }
        if control.class.eq_ignore_ascii_case("capcontrol") {
            let target = control.props.iter().rev().find_map(|(key, value)| {
                key.as_deref()
                    .filter(|key| key.eq_ignore_ascii_case("capacitor"))
                    .map(|_| value.trim_matches(['\"', '\'']))
            });
            if !target.is_some_and(|target| {
                network
                    .shunts()
                    .iter()
                    .any(|shunt| shunt.name.eq_ignore_ascii_case(target))
            }) {
                return Err(format!(
                    "Capacitor control {} has no supported capacitor target",
                    control.name
                ));
            }
        }
    }
    network
        .untyped_objects_mut()
        .retain(|object| !is_control(&object.class));
    prepared
        .insert_extension(
            FIXED_CONTROLS_EXTENSION,
            serde_json::json!({
                "mode": "written-equipment-settings", "controls": controls
            }),
        )
        .map_err(|error| error.to_string())?;
    super::validate_mc_module(&prepared)?;
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn module(extra: &str) -> PioModule<PioValue> {
        let source = format!("new circuit.test basekv=11\nnew transformer.t phases=3 windings=2 buses=[sourcebus load] conns=[wye wye] kvs=[11 .4] kvas=[100 100] taps=[1 1.025] xhl=4\nnew load.l bus1=load phases=3 kv=.4 kw=10 kvar=2\nnew regcontrol.r transformer=t winding=2 vreg=120 band=2 ptratio=1\n{extra}\n");
        powerio::parse(powerio::Source::from_memory("test.dss", source.as_bytes()).unwrap())
            .unwrap()
    }
    #[test]
    fn explicit_snapshot_preserves_original_controls_and_written_taps() {
        let original = module("");
        assert!(super::super::validate_mc_module(&original).is_err());
        let original_json = crate::ir::serialize_module(&original).unwrap();
        let fixed = prepare_mc_fixed_controls(&original).unwrap();
        assert_eq!(
            crate::ir::serialize_module(&original).unwrap(),
            original_json
        );
        assert_eq!(
            fixed.extensions()[FIXED_CONTROLS_EXTENSION]["controls"][0]["class"],
            "regcontrol"
        );
        let PioValue::MulticonductorNetwork(network) = fixed.value() else {
            panic!("network")
        };
        assert!((network.transformers()[0].windings[1].tap - 1.025).abs() < 1e-12);
        assert!(network.untyped_objects().is_empty());
        let saved = crate::ir::serialize_module(&fixed).unwrap();
        let restored = crate::ir::deserialize_module(&saved).unwrap();
        super::super::validate_mc_module(&restored).unwrap();
        assert!(prepare_mc_fixed_controls(&restored).is_err());
        let result =
            super::super::solve_mc_module_json(&saved, &super::super::McPfOptions::default())
                .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap()["converged"],
            true
        );
    }
    #[test]
    fn capacitor_settings_are_retained_and_unmodeled_steps_are_refused() {
        let source = "new capacitor.c bus1=load phases=3 kv=.4 kvar=2\nnew capcontrol.c capacitor=c element=transformer.t terminal=2 type=voltage";
        let original = module(source);
        let fixed = prepare_mc_fixed_controls(&original).unwrap();
        let (PioValue::MulticonductorNetwork(before), PioValue::MulticonductorNetwork(after)) =
            (original.value(), fixed.value())
        else {
            panic!("networks")
        };
        assert_eq!(before.shunts(), after.shunts());
        assert_eq!(
            fixed.extensions()[FIXED_CONTROLS_EXTENSION]["controls"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(prepare_mc_fixed_controls(&module(
            &source.replace("capacitor=c", "capacitor=missing")
        ))
        .is_err());
        assert!(
            prepare_mc_fixed_controls(&module(&source.replace("kvar=2", "kvar=2 states=[0]")))
                .is_err()
        );
    }

    #[test]
    fn serialized_controls_require_existing_transformer_targets() {
        for target in [None, Some("missing")] {
            let mut original = module("");
            let PioValue::MulticonductorNetwork(network) = original.value_mut() else {
                panic!("network")
            };
            let control = &mut network.untyped_objects_mut()[0];
            control
                .props
                .retain(|(key, _)| key.as_deref() != Some("transformer"));
            if let Some(target) = target {
                control
                    .props
                    .push((Some("transformer".into()), target.into()));
            }
            // The parser's retained diagnostic describes the original valid target.
            // Saved IR can change independently and must be checked on its own.
            let text = crate::ir::serialize_module(&original).unwrap();
            let restored = crate::ir::deserialize_module(&text).unwrap();
            let error = prepare_mc_fixed_controls(&restored).unwrap_err();
            assert!(error.contains("transformer target"), "{error}");
            assert_eq!(crate::ir::serialize_module(&restored).unwrap(), text);
        }
    }

    #[test]
    fn fixed_settings_never_clear_unrelated_errors_or_active_objects() {
        for extra in [
            "redirect missing.dss",
            "new relay.r monitoredobj=transformer.t",
            "new generator.g bus1=load phases=3 kv=.4 kw=1",
        ] {
            assert!(
                prepare_mc_fixed_controls(&module(extra)).is_err(),
                "{extra}"
            );
        }
    }
}
