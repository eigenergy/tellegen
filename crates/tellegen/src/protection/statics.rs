//! Protection on the load flow layer: the relays at the operating point, and
//! a static protection study of a fault. The study runs the relays on the
//! short circuit (IEC 60909 equivalent source) with the same logic as the
//! dynamic runs: their timers run on the constant fault phasors; when
//! breakers open, the short circuit is solved again with them open, and the
//! relays go on from there, until the fault is isolated or nothing more
//! operates.

use serde::{Deserialize, Serialize};

use super::setting::{FaultCalc, FaultPoint, ShortCircuit};
use super::{
    balanced, Action, End, Measure, Operation, ProtectionRuntime, ProtectionScheme, Topology,
    TripRecord, C64,
};

/// Where a static fault sits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StaticFault {
    Bus {
        bus: usize,
        #[serde(default)]
        r: f64,
        #[serde(default)]
        x: f64,
    },
    /// Inside branch `(from, to, circuit)` at `location` from `from`.
    Line {
        from: usize,
        to: usize,
        #[serde(default = "one")]
        circuit: String,
        location: f64,
        #[serde(default)]
        r: f64,
        #[serde(default)]
        x: f64,
    },
}

fn one() -> String {
    "1".into()
}

/// What one relay measures at the fault inception.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelaySeen {
    pub relay: String,
    /// Current through the relay, kA (0 for busbar relays).
    pub current_ka: f64,
    /// Voltage at its bus, pu.
    pub voltage_pu: f64,
    /// Apparent impedance, primary ohms `[r, x]` (distance relays).
    pub impedance: Option<[f64; 2]>,
}

/// One step of the study: breakers opened at `time`, then the fault current.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StudyStep {
    pub time: f64,
    pub opened: Vec<String>,
    /// Fault current after the step, kA.
    pub fault_current_ka: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StaticStudy {
    /// Initial fault current, kA.
    pub fault_current_ka: f64,
    pub seen: Vec<RelaySeen>,
    pub operations: Vec<Operation>,
    pub trips: Vec<TripRecord>,
    pub steps: Vec<StudyStep>,
    /// Whether the fault was isolated, and when.
    pub cleared_at: Option<f64>,
    pub notes: Vec<String>,
}

/// The relays' view of a solved short circuit.
struct StaticMeasure<'a> {
    calc: &'a FaultCalc<'a>,
    sc: &'a ShortCircuit,
    t: f64,
    open: &'a [(usize, End)],
    units_out: &'a [usize],
}

impl Measure for StaticMeasure<'_> {
    fn time(&self) -> f64 {
        self.t
    }
    fn frame_speed(&self) -> f64 {
        1.0
    }
    fn bus_voltage(&self, b: usize) -> [C64; 3] {
        balanced(self.sc.v[b])
    }
    fn branch_current(&self, e: usize, end: End) -> [C64; 3] {
        balanced(if end == End::From {
            self.sc.branch[e].0
        } else {
            self.sc.branch[e].1
        })
    }
    fn unit_current(&self, u: usize) -> [C64; 3] {
        if self.units_out.contains(&u) {
            return [C64::new(0.0, 0.0); 3];
        }
        balanced(self.calc.unit_current(self.sc, u))
    }
    fn feeder_current(&self, _b: usize) -> [C64; 3] {
        // Loads are neglected in the short circuit (IEC 60909).
        [C64::new(0.0, 0.0); 3]
    }
    fn branch_end_voltage(&self, e: usize, end: End) -> Option<[C64; 3]> {
        let br = &self.calc.topo().branches[e];
        Some(balanced(
            self.sc.v[if end == End::From { br.from } else { br.to }],
        ))
    }
    fn end_closed(&self, e: usize, end: End) -> bool {
        !self.open.contains(&(e, end))
    }
    fn unit_in_service(&self, u: usize) -> bool {
        !self.units_out.contains(&u)
    }
}

/// Run the static study of `fault` for up to `t_max` seconds.
///
/// # Errors
/// When the fault names a missing element, a relay does not bind, or the
/// short circuit cannot be solved.
pub fn static_study(
    topo: &Topology,
    scheme: &ProtectionScheme,
    fault: &StaticFault,
    t_max: f64,
) -> Result<StaticStudy, String> {
    let mut calc = FaultCalc::new(topo);
    calc.c = 1.0;
    let (point, zf, faulted_branch, fault_bus) = match fault {
        StaticFault::Bus { bus, r, x } => (
            FaultPoint::Bus(topo.bus(*bus)?),
            C64::new(*r, *x),
            None,
            Some(topo.bus(*bus)?),
        ),
        StaticFault::Line {
            from,
            to,
            circuit,
            location,
            r,
            x,
        } => {
            let e = topo.branch(*from, *to, circuit)?;
            if !(0.0..=1.0).contains(location) {
                return Err("location must lie in [0, 1]".into());
            }
            let alpha = if topo.branches[e].from_number == *from {
                *location
            } else {
                1.0 - location
            };
            (
                FaultPoint::Branch(e, alpha),
                C64::new(*r, *x),
                Some(e),
                None,
            )
        }
    };
    let mut runtime = ProtectionRuntime::bind(scheme, topo, true)?;
    let mut open: Vec<(usize, End)> = Vec::new();
    let mut units_out: Vec<usize> = Vec::new();
    let ka = |i: C64, b: usize| {
        let kv = topo.base_kv.get(b).copied().unwrap_or(1.0).max(1e-9);
        i.norm() * topo.sbase / (3f64.sqrt() * kv)
    };
    let fault_kv_bus =
        fault_bus.unwrap_or_else(|| faulted_branch.map_or(0, |e| topo.branches[e].from));
    // Pre-fault: the memory voltage of the relays.
    let pre = calc.solve_with(None, C64::new(0.0, 0.0), &open, &units_out)?;
    runtime.evaluate(&StaticMeasure {
        calc: &calc,
        sc: &pre,
        t: -0.1,
        open: &open,
        units_out: &units_out,
    });
    let mut sc = calc.solve_with(Some(point), zf, &open, &units_out)?;
    let mut study = StaticStudy {
        fault_current_ka: ka(sc.fault_current, fault_kv_bus),
        ..StaticStudy::default()
    };
    // What each relay sees at the inception.
    {
        let m = StaticMeasure {
            calc: &calc,
            sc: &sc,
            t: 0.0,
            open: &open,
            units_out: &units_out,
        };
        study.seen = runtime.seen(&m, |b, i| ka(i, b));
    }
    let mut t = 0.0;
    runtime.evaluate(&StaticMeasure {
        calc: &calc,
        sc: &sc,
        t,
        open: &open,
        units_out: &units_out,
    });
    let isolated = |open: &[(usize, End)], units_out: &[usize]| -> bool {
        if let Some(e) = faulted_branch {
            return open.contains(&(e, End::From)) && open.contains(&(e, End::To));
        }
        let Some(b) = fault_bus else { return false };
        let branches_open = topo.branches.iter().enumerate().all(|(e, br)| {
            (br.from != b || open.contains(&(e, End::From)))
                && (br.to != b || open.contains(&(e, End::To)))
        });
        let units = topo
            .units
            .iter()
            .enumerate()
            .all(|(u, x)| x.bus != b || units_out.contains(&u));
        branches_open && units
    };
    for _ in 0..2000 {
        let Some(next) = runtime.next_deadline() else {
            break;
        };
        if next > t_max {
            break;
        }
        // A deadline already passed (a timer that ran out while its relay
        // could not act) moves the clock on by a millisecond.
        t = if next > t + 1e-9 { next } else { t + 1e-3 };
        let actions = runtime.take_due(
            t,
            &StaticMeasure {
                calc: &calc,
                sc: &sc,
                t,
                open: &open,
                units_out: &units_out,
            },
        );
        runtime.applied();
        if !actions.is_empty() {
            let mut opened = Vec::new();
            for a in &actions {
                match *a {
                    Action::OpenEnd(e, end) => {
                        open.push((e, end));
                        let br = &topo.branches[e];
                        let at = if end == End::From {
                            br.from_number
                        } else {
                            br.to_number
                        };
                        opened.push(format!("{}-{} at {at}", br.from_number, br.to_number));
                    }
                    Action::CloseEnd(e, end) => {
                        open.retain(|x| *x != (e, end));
                        let br = &topo.branches[e];
                        opened.push(format!("{}-{} reclosed", br.from_number, br.to_number));
                    }
                    // The static study keeps series capacitors in.
                    Action::Bypass(_) | Action::Reinsert(_) => {}
                    Action::TripUnit(u) => {
                        units_out.push(u);
                        let unit = &topo.units[u];
                        opened.push(format!("unit {} '{}'", unit.bus_number, unit.machine_id));
                    }
                }
            }
            let cleared = isolated(&open, &units_out);
            sc = if cleared {
                calc.solve_with(None, C64::new(0.0, 0.0), &open, &units_out)?
            } else {
                calc.solve_with(Some(point), zf, &open, &units_out)?
            };
            study.steps.push(StudyStep {
                time: t,
                opened,
                fault_current_ka: if cleared {
                    0.0
                } else {
                    ka(sc.fault_current, fault_kv_bus)
                },
            });
            if cleared && study.cleared_at.is_none() {
                study.cleared_at = Some(t);
            } else if !cleared {
                study.cleared_at = None;
            }
        }
        runtime.evaluate(&StaticMeasure {
            calc: &calc,
            sc: &sc,
            t,
            open: &open,
            units_out: &units_out,
        });
    }
    if study.cleared_at.is_none() {
        study
            .notes
            .push(format!("the fault is not isolated within {t_max} s"));
    }
    study.notes.push("short circuit with the equivalent source c = 1.0 behind subtransient reactances; loads neglected; frequency functions do not act in a static study".into());
    let report = runtime.into_report();
    study.operations = report.operations;
    study.trips = report.trips;
    Ok(study)
}

/// The relays at the load flow operating point: what picks up (overload,
/// voltage, load inside a zone) and what each relay measures.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OperatingPoint {
    pub pickups: Vec<Operation>,
    pub seen: Vec<RelaySeen>,
}
