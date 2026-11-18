//! Explicit snapshots using equipment settings already written in the input.
use powerio::{PioModule, PioValue};
use powerio_dist::{MulticonductorNetwork, UntypedObject};
use std::collections::HashSet;

pub const FIXED_CONTROLS_EXTENSION: &str = "tellegen.fixed_controls";

/// Keep written taps, capacitor states and closed fuses without running their
/// controls. The input is unchanged. Original control records and source files
/// remain in the returned module; all other calculation restrictions still apply.
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
        ["regcontrol", "capcontrol", "fuse"]
            .iter()
            .any(|control| class.eq_ignore_ascii_case(control))
    };
    let controls: Vec<_> = network
        .untyped_objects()
        .iter()
        .filter(|object| is_control(&object.class))
        .cloned()
        .collect();
    if controls.is_empty() {
        return Err(
            "This case has no regulator controls, capacitor controls or fuses to hold fixed"
                .to_owned(),
        );
    }
    let fuse_targets = FuseTargets::new(network);
    for control in &controls {
        if control.class.eq_ignore_ascii_case("regcontrol")
            && !written(control, "transformer").is_some_and(|target| {
                network
                    .transformers()
                    .iter()
                    .any(|transformer| transformer.name.eq_ignore_ascii_case(target))
            })
        {
            return Err(format!(
                "Regulator control {} has no supported transformer target",
                control.name
            ));
        }
        if control.class.eq_ignore_ascii_case("capcontrol")
            && !written(control, "capacitor").is_some_and(|target| {
                network
                    .shunts()
                    .iter()
                    .any(|shunt| shunt.name.eq_ignore_ascii_case(target))
            })
        {
            return Err(format!(
                "Capacitor control {} has no supported capacitor target",
                control.name
            ));
        }
        if control.class.eq_ignore_ascii_case("fuse") {
            fuse_targets.check_closed(control)?;
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

/// [`prepare_mc_fixed_controls`] followed by
/// [`prepare_mc_energized_network`](super::prepare_mc_energized_network),
/// checking the resulting network once instead of once in each.
pub fn prepare_mc_fixed_controls_energized_network(
    module: &PioModule<PioValue>,
) -> Result<(PioModule<PioValue>, Vec<super::McExcludedIsland>), String> {
    super::energized::record_selection(&prepare_mc_fixed_controls(module)?)
}

/// The last written value of a property, without quotes.
fn written<'a>(object: &'a UntypedObject, property: &str) -> Option<&'a str> {
    object.props.iter().rev().find_map(|(key, value)| {
        key.as_deref()
            .filter(|key| key.eq_ignore_ascii_case(property))
            .map(|_| value.trim_matches(['\"', '\'']))
    })
}

/// Lowercase `class.name` of the equipment a fuse may protect, and of the
/// equipment an `open` or `close` command switches.
struct FuseTargets {
    equipment: HashSet<String>,
    switched_by_command: HashSet<String>,
}

impl FuseTargets {
    fn new(network: &MulticonductorNetwork) -> Self {
        let lines = network.lines().iter().map(|line| &line.name).chain(
            network
                .untyped_objects()
                .iter()
                .filter(|object| super::input::is_disabled_line(object))
                .map(|object| &object.name),
        );
        let transformers = network.transformers().iter().map(|unit| &unit.name);
        Self {
            equipment: lines
                .map(|name| format!("line.{}", name.to_ascii_lowercase()))
                .chain(
                    transformers.map(|name| format!("transformer.{}", name.to_ascii_lowercase())),
                )
                .collect(),
            switched_by_command: network
                .commands()
                .iter()
                .filter(|(verb, _)| {
                    verb.eq_ignore_ascii_case("open") || verb.eq_ignore_ascii_case("close")
                })
                .filter_map(|(_, arguments)| arguments.split_whitespace().next())
                .map(|element| {
                    let element = element.trim_matches(['"', '\'']).to_ascii_lowercase();
                    element
                        .strip_prefix("object=")
                        .map_or(element.clone(), str::to_owned)
                })
                .collect(),
        }
    }

    /// A fuse with every phase closed leaves the circuit unchanged while it does
    /// not blow, which is what OpenDSS solves with `controlmode=off`. A fuse
    /// written open on any phase disconnects that phase even when the fuse is
    /// disabled, and a fuse closes a terminal that an earlier `open` command
    /// opened; neither is represented here, so both are refused.
    fn check_closed(&self, fuse: &UntypedObject) -> Result<(), String> {
        const SETTINGS: &[&str] = &[
            "monitoredobj",
            "monitoredterm",
            "switchedobj",
            "switchedterm",
            "fusecurve",
            "ratedcurrent",
            "delay",
            "basefreq",
            "enabled",
        ];
        for (key, value) in &fuse.props {
            let key = key.as_deref().unwrap_or_default().to_ascii_lowercase();
            if ["action", "normal", "state"].contains(&key.as_str()) {
                let mut phases = value
                    .split(|c: char| c.is_whitespace() || "[](){}\"',".contains(c))
                    .filter(|phase| !phase.is_empty())
                    .peekable();
                if phases.peek().is_none()
                    || phases.any(|phase| !phase.to_ascii_lowercase().starts_with('c'))
                {
                    return Err(format!(
                        "Fuse {} is written open; open fuses are not supported",
                        fuse.name
                    ));
                }
            } else if !SETTINGS.contains(&key.as_str()) {
                return Err(format!(
                    "Fuse {} has an unsupported property `{key}`",
                    fuse.name
                ));
            }
        }
        let target = written(fuse, "switchedobj")
            .or_else(|| written(fuse, "monitoredobj"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !self.equipment.contains(&target) {
            return Err(format!(
                "Fuse {} has no supported line or transformer target",
                fuse.name
            ));
        }
        if self.switched_by_command.contains(&target) {
            return Err(format!(
                "Fuse {} protects {target}, which an open or close command also switches",
                fuse.name
            ));
        }
        Ok(())
    }
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

    fn solved(module: &PioModule<PioValue>) -> super::super::McPfResult {
        let text = crate::ir::serialize_module(module).unwrap();
        let result =
            super::super::solve_mc_module_json(&text, &super::super::McPfOptions::default())
                .unwrap();
        serde_json::from_str(&result).unwrap()
    }

    const LINE: &str = "new line.feed phases=3 bus1=load bus2=far r1=.01 x1=.02 r0=.02 x0=.04 c1=0 c0=0 length=1 units=km\nnew load.far bus1=far phases=3 kv=.4 kw=5 kvar=1";

    #[test]
    fn closed_fuses_are_held_closed_and_change_nothing() {
        let without = solved(&prepare_mc_fixed_controls(&module(LINE)).unwrap());
        for fuse in [
            "new fuse.f monitoredobj=line.feed ratedcurrent=100",
            // Far below the load current: it would blow if controls ran.
            "new fuse.f monitoredobj=line.feed monitoredterm=2 ratedcurrent=.01 delay=0",
            "new fuse.f monitoredobj=transformer.t fusecurve=tlink enabled=n",
            "new fuse.f monitoredobj=load.far switchedobj=line.feed switchedterm=1",
            "new fuse.f monitoredobj=line.feed action=close normal=closed state=[closed closed closed]",
        ] {
            let original = module(&format!("{LINE}\n{fuse}"));
            assert!(super::super::validate_mc_module(&original).is_err());
            let fixed = prepare_mc_fixed_controls(&original).unwrap();
            let records = fixed.extensions()[FIXED_CONTROLS_EXTENSION]["controls"]
                .as_array()
                .unwrap();
            assert!(
                records.iter().any(|record| record["class"] == "fuse"),
                "{fuse}"
            );
            let with = solved(&fixed);
            assert!(with.converged);
            assert_eq!(
                serde_json::to_value(&with.terminals).unwrap(),
                serde_json::to_value(&without.terminals).unwrap(),
                "{fuse}"
            );
        }
    }

    #[test]
    fn written_settings_and_energized_network_together_match_the_two_steps() {
        let original = module(&format!(
            "{LINE}\nnew line.dead phases=3 bus1=island1 bus2=island2 r1=.01 x1=.02 r0=.02 x0=.04 c1=0 c0=0 length=1 units=km"
        ));
        let (together, islands) = prepare_mc_fixed_controls_energized_network(&original).unwrap();
        let (stepwise, expected) = super::super::prepare_mc_energized_network(
            &prepare_mc_fixed_controls(&original).unwrap(),
        )
        .unwrap();
        assert_eq!(islands, expected);
        assert_eq!(islands.len(), 1);
        assert_eq!(
            crate::ir::serialize_module(&together).unwrap(),
            crate::ir::serialize_module(&stepwise).unwrap()
        );
        assert!(solved(&together).converged);
        // The same refusals as the first step alone.
        assert!(
            prepare_mc_fixed_controls_energized_network(&module(&format!(
                "{LINE}\nnew fuse.f monitoredobj=line.feed state=open"
            )))
            .is_err()
        );
    }

    #[test]
    fn open_and_unsupported_fuses_are_refused() {
        for (fuse, reason) in [
            (
                "new fuse.f monitoredobj=line.feed state=open",
                "written open",
            ),
            (
                "new fuse.f monitoredobj=line.feed action=open",
                "written open",
            ),
            (
                "new fuse.f monitoredobj=line.feed normal=open",
                "written open",
            ),
            // OpenDSS keeps a phase open once any of these opened it.
            (
                "new fuse.f monitoredobj=line.feed normal=open state=closed",
                "written open",
            ),
            (
                "new fuse.f monitoredobj=line.feed state=[closed open closed]",
                "written open",
            ),
            // A disabled fuse still opens the phases it is written open on.
            (
                "new fuse.f monitoredobj=line.feed state=open enabled=n",
                "written open",
            ),
            ("new fuse.f monitoredobj=line.missing", "target"),
            ("new fuse.f monitoredobj=load.far", "target"),
            ("new fuse.f ratedcurrent=100", "target"),
            ("new fuse.f line.feed 1", "unsupported property"),
            // Shortened property names are not read.
            ("new fuse.f mon=line.feed", "unsupported property"),
            (
                "open line.feed 1\nnew fuse.f monitoredobj=line.feed",
                "target",
            ),
            (
                "new fuse.f monitoredobj=line.feed\nopen line.feed 1\nclose line.feed 1",
                "open or close command",
            ),
        ] {
            let error = prepare_mc_fixed_controls(&module(&format!("{LINE}\n{fuse}"))).unwrap_err();
            assert!(error.contains(reason), "{fuse}: {error}");
        }
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
