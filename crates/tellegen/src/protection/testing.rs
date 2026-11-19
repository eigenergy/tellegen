//! Protection test cases: a fault (or another disturbance), the relays that
//! must trip within a time window, and the ones that must not.
//!
//! [`standard_suite`] derives the usual commissioning and setting checks of
//! a scheme in the spirit of IEC 60255-121 (distance reach and operating
//! time at points along the line, reverse and out-of-zone faults),
//! IEC 60255-187-1 (differential of transformers and units: internal faults
//! trip, through faults do not; the same checks for lines and busbars) and the coordination checks of a protection study (selectivity,
//! backup by Z2 when the busbar protection fails, breaker failure,
//! auto-reclose, inverter ride-through).
//!
//! @block Protection tests

use serde::{Deserialize, Serialize};

use super::{
    BreakerRef, Direction, OperationKind, ProtectionFunction, ProtectionReport, ProtectionScheme,
    RelayLocation, Topology, TripRecord, UnitKind,
};
use crate::rms::{RmsEvent, RmsRequest, RmsSimulation};

// @block Test cases
// A test is a disturbance in RMS events, the relays and functions switched
// off for it, the breakers made to fail, and its expectations: a relay
// trips within a window, does not trip, only the listed relays trip,
// recloses, locks out, or its breaker-failure function acts. Its outcome
// says per expectation what was observed.

/// What a test expects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expectation {
    /// The relay trips (with a function whose label starts with `function`)
    /// between `min` and `max` seconds after the disturbance.
    Trip {
        relay: String,
        #[serde(default)]
        function: Option<String>,
        min: f64,
        max: f64,
    },
    /// The relay does not trip (by that function) during the run.
    NoTrip {
        relay: String,
        #[serde(default)]
        function: Option<String>,
    },
    /// Selectivity: no relay outside this list trips (up to `until` s
    /// after the disturbance, when given: what happens after the fault is
    /// cleared, such as an unstable swing, is not a selectivity question).
    OnlyTrips {
        relays: Vec<String>,
        #[serde(default)]
        until: Option<f64>,
    },
    /// The relay recloses (79) and does not lock out.
    Reclose { relay: String },
    /// The relay locks out after reclosing onto the fault.
    Lockout { relay: String },
    /// The relay's breaker-failure function trips the adjacent breakers.
    BreakerFailure { relay: String },
}

impl Expectation {
    fn text(&self) -> String {
        let f = |o: &Option<String>| o.as_ref().map_or(String::new(), |s| format!(" by {s}"));
        match self {
            Self::Trip {
                relay,
                function,
                min,
                max,
            } => {
                format!(
                    "{relay} trips{} within {:.0}–{:.0} ms",
                    f(function),
                    min * 1e3,
                    max * 1e3
                )
            }
            Self::NoTrip { relay, function } => format!("{relay} does not trip{}", f(function)),
            Self::OnlyTrips { relays, until } => match until {
                Some(u) => format!("only {} trip within {:.0} ms", relays.join(", "), u * 1e3),
                None => format!("only {} trip", relays.join(", ")),
            },
            Self::Reclose { relay } => format!("{relay} recloses successfully"),
            Self::Lockout { relay } => format!("{relay} locks out"),
            Self::BreakerFailure { relay } => {
                format!("{relay} breaker failure trips the adjacent breakers")
            }
        }
    }
}

/// One test case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtectionTest {
    pub name: String,
    /// `distance`, `differential`, `busbar`, `transformer`, `generator`,
    /// `inverter`, `breaker_failure`, `autoreclose`, `selectivity`.
    pub category: String,
    #[serde(default)]
    pub description: String,
    /// When the disturbance starts; expectation windows count from here.
    pub fault_time: f64,
    pub t_end: f64,
    pub events: Vec<RmsEvent>,
    /// Relays switched off for the test.
    #[serde(default)]
    pub disable: Vec<String>,
    /// Functions switched off in every relay: a label prefix (`87L`,
    /// `21`), or `POTT` for the teleprotection.
    #[serde(default)]
    pub disable_functions: Vec<String>,
    #[serde(default)]
    pub stuck_breakers: Vec<BreakerRef>,
    pub expect: Vec<Expectation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub expectation: String,
    pub passed: bool,
    pub observed: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestOutcome {
    pub name: String,
    pub category: String,
    pub passed: bool,
    pub checks: Vec<CheckOutcome>,
    /// Every trip, times from the disturbance.
    pub trips: Vec<TripRecord>,
    #[serde(default)]
    pub error: Option<String>,
    pub seconds: f64,
}

// @block Running a test
// The scheme as the test changes it, run with protection in the RMS engine
// at a 2.5 ms step (1 ms if that does not converge), and the relays' record
// checked against the expectations.

/// The scheme as a test runs it.
#[must_use]
pub fn prepare_scheme(scheme: &ProtectionScheme, test: &ProtectionTest) -> ProtectionScheme {
    let mut s = scheme.clone();
    for r in &mut s.relays {
        if test.disable.contains(&r.id) {
            r.enabled = false;
        }
        for token in &test.disable_functions {
            if token == "POTT" || token == "PUTT" {
                for f in &mut r.functions {
                    if let ProtectionFunction::Distance(d) = f {
                        d.teleprotection = None;
                    }
                }
            } else {
                r.functions
                    .retain(|f| !f.label().starts_with(token.as_str()));
            }
        }
    }
    s.stuck_breakers.extend(test.stuck_breakers.iter().cloned());
    s
}

/// Run a test in the RMS engine.
#[must_use]
pub fn run_rms(
    sim: &RmsSimulation,
    scheme: &ProtectionScheme,
    test: &ProtectionTest,
) -> TestOutcome {
    let started = web_now();
    let mut request = RmsRequest {
        t_end: test.t_end,
        dt: 0.0025,
        output_step: 0.01,
        events: test.events.clone(),
        small_signal: false,
        ..RmsRequest::default()
    };
    let scheme = prepare_scheme(scheme, test);
    // A run that does not converge at 2.5 ms gets one retry at 1 ms.
    let run = sim.run_with_protection(&request, &scheme).or_else(|_| {
        request.dt = 0.001;
        sim.run_with_protection(&request, &scheme)
    });
    match run {
        Ok((_, report)) => evaluate(test, &report, web_now() - started),
        Err(e) => TestOutcome {
            name: test.name.clone(),
            category: test.category.clone(),
            passed: false,
            checks: Vec::new(),
            trips: Vec::new(),
            error: Some(e),
            seconds: web_now() - started,
        },
    }
}

fn web_now() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64())
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

/// Check a run's relay record against the test's expectations.
// @block Check the expectations
#[must_use]
pub fn evaluate(test: &ProtectionTest, report: &ProtectionReport, seconds: f64) -> TestOutcome {
    let t0 = test.fault_time;
    let trips: Vec<TripRecord> = report
        .operations
        .iter()
        .filter(|o| o.kind == OperationKind::Trip)
        .map(|o| TripRecord {
            relay: o.relay.clone(),
            function: o.function.clone(),
            time: o.time - t0,
        })
        .collect();
    let matches = |relay: &str, function: &Option<String>, tr: &TripRecord| {
        tr.relay == relay
            && function
                .as_ref()
                .is_none_or(|f| tr.function.starts_with(f.as_str()))
    };
    let describe = |list: &[&TripRecord]| {
        if list.is_empty() {
            "no trip".to_owned()
        } else {
            list.iter()
                .map(|t| format!("{} {} at {:.0} ms", t.relay, t.function, t.time * 1e3))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let has_op = |relay: &str, kind: OperationKind| {
        report
            .operations
            .iter()
            .any(|o| o.relay == relay && o.kind == kind)
    };
    let checks: Vec<CheckOutcome> = test
        .expect
        .iter()
        .map(|ex| {
            let (passed, observed) = match ex {
                Expectation::Trip {
                    relay,
                    function,
                    min,
                    max,
                } => {
                    let mine: Vec<&TripRecord> = trips
                        .iter()
                        .filter(|t| matches(relay, function, t))
                        .collect();
                    let ok = mine
                        .first()
                        .is_some_and(|t| t.time >= min - 1e-6 && t.time <= max + 1e-6);
                    let all: Vec<&TripRecord> =
                        trips.iter().filter(|t| t.relay == *relay).collect();
                    (ok, describe(&all))
                }
                Expectation::NoTrip { relay, function } => {
                    let mine: Vec<&TripRecord> = trips
                        .iter()
                        .filter(|t| matches(relay, function, t))
                        .collect();
                    (mine.is_empty(), describe(&mine))
                }
                Expectation::OnlyTrips { relays, until } => {
                    let (response, others): (Vec<&TripRecord>, Vec<&TripRecord>) = trips
                        .iter()
                        .filter(|t| !relays.contains(&t.relay) && until.is_none_or(|u| t.time <= u))
                        .partition(|t| system_response(&t.function));
                    let mut seen = if others.is_empty() {
                        "selective".to_owned()
                    } else {
                        format!("also {}", describe(&others))
                    };
                    if !response.is_empty() {
                        seen.push_str(&format!(
                            "; responding to the system after the clearing: {}",
                            describe(&response)
                        ));
                    }
                    (others.is_empty(), seen)
                }
                Expectation::Reclose { relay } => {
                    let rec = has_op(relay, OperationKind::Reclose);
                    let lock = has_op(relay, OperationKind::Lockout);
                    // A follower the synchronism check holds (the angle across
                    // the line too large) is right not to close.
                    let held = has_op(relay, OperationKind::SyncHeld);
                    (
                        (rec || held) && !lock,
                        format!("reclose {rec}, held by 25 {held}, lockout {lock}"),
                    )
                }
                Expectation::Lockout { relay } => {
                    let lock = has_op(relay, OperationKind::Lockout);
                    (lock, format!("lockout {lock}"))
                }
                Expectation::BreakerFailure { relay } => {
                    let bf = has_op(relay, OperationKind::BreakerFailure);
                    (bf, format!("50BF {bf}"))
                }
            };
            CheckOutcome {
                expectation: ex.text(),
                passed,
                observed,
            }
        })
        .collect();
    TestOutcome {
        name: test.name.clone(),
        category: test.category.clone(),
        passed: checks.iter().all(|c| c.passed),
        checks,
        trips,
        error: None,
        seconds,
    }
}

// @block Suite helpers
// What the standard suite builds its tests from: its options, the pairs of
// line relays, which trips count as the system's response rather than fault
// clearing, a relay's zone delays and what its zones hold, and the
// reactance of a bolted fault.

/// Options of the standard suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuiteOptions {
    /// At most this many lines get the full set of distance tests (all
    /// lines when `None`).
    #[serde(default)]
    pub max_lines: Option<usize>,
    #[serde(default = "default_fault_time")]
    pub fault_time: f64,
}

fn default_fault_time() -> f64 {
    0.1
}

impl Default for SuiteOptions {
    fn default() -> Self {
        Self {
            max_lines: None,
            fault_time: default_fault_time(),
        }
    }
}

struct LineRelays<'a> {
    from: usize,
    to: usize,
    circuit: String,
    a: &'a super::Relay,
    b: &'a super::Relay,
}

/// Whether a trip by `function` answers the state of the system rather
/// than a fault: frequency (81), out of step (78), ride-through and voltage
/// (FRT, 27, 59), overexcitation (24), loss of field (40), reverse power (32)
/// and unbalance (46). Selectivity is the fault-clearing relays' matter;
/// a unit that loses synchronism or overspeeds after a correct clearing is
/// the system's response, which the run shows but the check does not fail on.
fn system_response(function: &str) -> bool {
    ["81", "78", "FRT", "27", "59", "24", "40", "32", "46"]
        .iter()
        .any(|p| function.starts_with(p))
}

fn zone_delay(r: &super::Relay, name: &str) -> Option<f64> {
    r.functions.iter().find_map(|f| match f {
        ProtectionFunction::Distance(d) => d
            .zones
            .iter()
            .find(|z| z.name == name && z.direction == Direction::Forward)
            .map(|z| z.delay),
        _ => None,
    })
}

/// Whether relay `r`'s forward zone `name` holds the apparent impedance
/// `zapp` (system pu) it measures at dense bus `bus`.
fn zone_sees(
    topo: &Topology,
    r: &super::Relay,
    name: &str,
    bus: usize,
    zapp: num_complex::Complex<f64>,
) -> bool {
    r.functions.iter().any(|f| match f {
        ProtectionFunction::Distance(d) => d
            .zones
            .iter()
            .filter(|z| z.name == name && z.direction == Direction::Forward)
            .any(|z| super::functions::zone_contains(d, z, zapp, topo.z_base(bus))),
        _ => false,
    })
}

fn has(r: &super::Relay, prefix: &str) -> bool {
    r.functions.iter().any(|f| f.label().starts_with(prefix))
}

/// Instantaneous operation: the relay's measuring time plus a margin of
/// two evaluation steps.
const FAST: f64 = 0.06;

/// Reactance of a bolted fault, system per unit: 3 % of the smallest series
/// impedance at the fault, at most 1e-4 pu. A fixed 1e-4 pu is 0.25 Ω at
/// 500 kV on a 100 MVA base, a third of a short EHV line: it leaves several
/// percent of voltage at the fault and moves what the relays measure. Much
/// less where it is not needed only drives the units there to exactly zero
/// voltage, where the solver gains nothing.
fn bolted_x(topo: &Topology, branches: impl Iterator<Item = usize>) -> f64 {
    let z = branches
        .map(|e| topo.branches[e].z.norm())
        .filter(|z| *z > 0.0)
        .fold(f64::INFINITY, f64::min);
    (0.03 * z).clamp(1e-7, 1e-4)
}

/// [`bolted_x`] for a fault at bus `number`, on its lines: a short
/// transformer to a plant needs no stiffer fault (its relays are
/// differential), and the plant would only be driven to zero voltage.
fn bolted_bus_x(topo: &Topology, number: usize) -> f64 {
    let Ok(b) = topo.bus(number) else { return 1e-4 };
    bolted_x(
        topo,
        (0..topo.branches.len()).filter(|&e| {
            let br = &topo.branches[e];
            !br.transformer && (br.from == b || br.to == b)
        }),
    )
}

// @block The standard suite
// The tests derived from a scheme and its network: for each line with
// distance relays at both ends, faults along it and the checks around it;
// for busbars, transformers, units and inverters their own faults; each
// expectation set from what the relays measure for that fault.

/// The standard tests of a scheme on a topology.
#[must_use]
pub fn standard_suite(
    topo: &Topology,
    scheme: &ProtectionScheme,
    opts: &SuiteOptions,
) -> Vec<ProtectionTest> {
    // @block Line pairs and fault events
    // The pairs of distance relays at the two ends of a line, and the line
    // fault every test applies: bolted, with a reactance small against the
    // line's impedance.
    let t0 = opts.fault_time;
    let mt = scheme.measuring_time;
    let mut tests = Vec::new();
    let relay = |id: &str| scheme.relays.iter().find(|r| r.id == id && r.enabled);
    // Line relay pairs with distance protection at both ends.
    let mut pairs: Vec<LineRelays> = Vec::new();
    for a in scheme.relays.iter().filter(|r| r.enabled && has(r, "21")) {
        let RelayLocation::Line {
            from,
            to,
            circuit,
            at,
        } = &a.location
        else {
            continue;
        };
        if at != from {
            continue;
        }
        let b = scheme.relays.iter().find(|o| {
            o.enabled
                && has(o, "21")
                && matches!(&o.location, RelayLocation::Line { from: f2, to: t2, circuit: c2, at: a2 } if f2 == from && t2 == to && c2 == circuit && a2 == to)
        });
        if let Some(b) = b {
            pairs.push(LineRelays {
                from: *from,
                to: *to,
                circuit: circuit.clone(),
                a,
                b,
            });
        }
    }
    let fault = |from: usize, to: usize, circuit: &str, location: f64, permanent: bool| {
        RmsEvent::LineFault {
            time: t0,
            from,
            to,
            circuit: circuit.to_owned(),
            location,
            r: 0.0,
            x: topo
                .branch(from, to, circuit)
                // A transformer's relays are differential: no stiffer fault.
                .map_or(1e-4, |e| {
                    if topo.branches[e].transformer {
                        1e-4
                    } else {
                        bolted_x(topo, std::iter::once(e))
                    }
                }),
            permanent,
        }
    };
    // @block Line tests
    // The tests of each line pair (all of them, or the first `max_lines`).
    let take = opts.max_lines.unwrap_or(usize::MAX);
    for p in pairs.iter().take(take) {
        // @block What each end sees
        // Per fault location, whether each end has infeed enough to measure
        // the fault, and whether its Z1 and Z2 hold what it measures (a
        // static short circuit of the test's own fault), so expectations
        // follow the setting.
        let (Some(z2a), Some(z2b)) = (zone_delay(p.a, "Z2"), zone_delay(p.b, "Z2")) else {
            continue;
        };
        let name = super::setting::branch_tag(p.from, p.to, &p.circuit);
        let only = vec![p.a.id.clone(), p.b.id.clone()];
        let distance_only = vec!["87L".to_owned(), "POTT".to_owned()];
        let t_dist = z2a.max(z2b) + 0.3;
        let calc = super::setting::FaultCalc::new(topo);
        let branch = topo.branch(p.from, p.to, &p.circuit).ok();
        let line_x = branch.map_or(1e-4, |e| {
            if topo.branches[e].transformer {
                1e-4
            } else {
                bolted_x(topo, std::iter::once(e))
            }
        });
        // Whether each end carries enough fault current to measure (a radial
        // end without a source behind it does not: weak infeed).
        let fed = |loc: f64| -> (bool, bool) {
            let Some(e) = branch else { return (true, true) };
            let alpha = if topo.branches[e].from_number == p.from {
                loc
            } else {
                1.0 - loc
            };
            let Ok(sc) = calc.solve(
                super::setting::FaultPoint::Branch(e, alpha),
                num_complex::Complex::new(0.0, 0.0),
                &[],
            ) else {
                return (true, true);
            };
            let (i_from, i_to) = (sc.branch[e].0.norm(), sc.branch[e].1.norm());
            let (ia, ib) = if topo.branches[e].from_number == p.from {
                (i_from, i_to)
            } else {
                (i_to, i_from)
            };
            let strong = |r: &super::Relay, i: f64| {
                let i_nom = r.i_nom.unwrap_or_else(|| {
                    if topo.branches[e].rate > 0.0 {
                        topo.branches[e].rate
                    } else {
                        1.0
                    }
                });
                let imin = r
                    .functions
                    .iter()
                    .find_map(|f| match f {
                        ProtectionFunction::Distance(d) => Some(d.min_current),
                        _ => None,
                    })
                    .unwrap_or(0.2);
                i > 2.0 * imin * i_nom
            };
            (strong(p.a, ia), strong(p.b, ib))
        };
        // Which end sees a fault at `loc` (from `p.from`) in zone 1: what its
        // relay measures against its zone 1 (static, the test's fault), so the
        // expectation follows the setting; by the position when the
        // calculation has no answer.
        let in_z1 = |loc: f64, near: bool, far: bool| -> (bool, bool, bool, bool) {
            let Some(e) = branch else {
                return (near, far, true, true);
            };
            let forward = topo.branches[e].from_number == p.from;
            let alpha = if forward { loc } else { 1.0 - loc };
            let Ok(sc) = calc.solve(
                super::setting::FaultPoint::Branch(e, alpha),
                num_complex::Complex::new(0.0, line_x),
                &[],
            ) else {
                return (near, far, true, true);
            };
            let br = &topo.branches[e];
            let (end_a, end_b) = if forward {
                (super::End::From, super::End::To)
            } else {
                (super::End::To, super::End::From)
            };
            let bus = |end| {
                if end == super::End::From {
                    br.from
                } else {
                    br.to
                }
            };
            let sees = |r: &super::Relay, end, zone: &str| {
                calc.apparent(&sc, e, end)
                    .is_some_and(|z| zone_sees(topo, r, zone, bus(end), z))
            };
            (
                sees(p.a, end_a, "Z1"),
                sees(p.b, end_b, "Z1"),
                sees(p.a, end_a, "Z2"),
                sees(p.b, end_b, "Z2"),
            )
        };
        // @block Faults along the line
        // Distance only (87L and POTT off), faults at 10, 50 and 90 %: an
        // end that sees the fault in Z1 trips at once, one in Z2 in Z2 time,
        // one that only sees it after the other end has opened sequentially;
        // an end without infeed is not expected to trip. No other relay may
        // trip while the fault lasts.
        for (loc, near_pos, far_pos) in [(0.1, true, false), (0.5, true, true), (0.9, false, true)]
        {
            let (near_z1, far_z1, near_z2, far_z2) = in_z1(loc, near_pos, far_pos);
            // An end whose Z2 does not see the fault while the other end feeds
            // it (infeed) sees it once the other end has opened in Z1: it
            // trips in Z2 time after that (sequential clearing).
            let window = |z1: bool, z2_sees: bool, z2: f64, other_z1: bool, other_breaker: f64| {
                if z1 {
                    (0.0, FAST)
                } else if z2_sees || !other_z1 {
                    (z2, z2 + mt + 0.04)
                } else {
                    (z2, z2 + mt + 0.04 + FAST + other_breaker)
                }
            };
            // An end not in Z1 while the other end is: once that one has
            // opened, this end may see the fault in Z1 too (sequential).
            let seq = |z1: bool, other_z1: bool| !z1 && other_z1;
            let (mut amin, amax) = window(near_z1, near_z2, z2a, far_z1, p.b.breaker_time);
            let (mut bmin, bmax) = window(far_z1, far_z2, z2b, near_z1, p.a.breaker_time);
            // Sequential: once the other end opened the fault may also fall
            // in Z1; any distance zone within that window will do.
            let seq_a = seq(near_z1, far_z1);
            let seq_b = seq(far_z1, near_z1);
            if seq_a {
                amin = 0.0;
            }
            if seq_b {
                bmin = 0.0;
            }
            let zone_fn = |z1: bool, seq: bool| -> String {
                if seq {
                    "21".into()
                } else if z1 {
                    "21 Z1".into()
                } else {
                    "21 Z2".into()
                }
            };
            let (fed_a, fed_b) = fed(loc);
            let mut expect = Vec::new();
            if fed_a {
                expect.push(Expectation::Trip {
                    relay: p.a.id.clone(),
                    function: Some(zone_fn(near_z1, seq_a)),
                    min: amin,
                    max: amax,
                });
            }
            if fed_b {
                expect.push(Expectation::Trip {
                    relay: p.b.id.clone(),
                    function: Some(zone_fn(far_z1, seq_b)),
                    min: bmin,
                    max: bmax,
                });
            }
            if !fed_a && !fed_b {
                continue;
            }
            // Selectivity while the fault lasts.
            let until = if fed_a && fed_b {
                amax.max(bmax)
            } else if fed_a {
                amax
            } else {
                bmax
            };
            expect.push(Expectation::OnlyTrips {
                relays: only.clone(),
                until: Some(until),
            });
            tests.push(ProtectionTest {
                name: format!("21 {name}: fault at {:.0} %", loc * 100.0),
                category: "distance".into(),
                description: format!(
                    "Distance only (87L and POTT off): {} trips in {} and {} in {}{}",
                    p.a.id,
                    if near_z1 { "Z1" } else { "Z2" },
                    p.b.id,
                    if far_z1 { "Z1" } else { "Z2" },
                    match (fed_a, fed_b) {
                        (true, true) => String::new(),
                        (false, _) => format!("; {} has no infeed behind it (weak infeed) and is not expected to see the fault", p.a.id),
                        (_, false) => format!("; {} has no infeed behind it (weak infeed) and is not expected to see the fault", p.b.id),
                    }
                ),
                fault_time: t0,
                t_end: t0 + t_dist,
                events: vec![fault(p.from, p.to, &p.circuit, loc, true)],
                disable: Vec::new(),
                disable_functions: distance_only.clone(),
                stuck_breakers: Vec::new(),
                expect,
            });
        }
        // @block POTT and line differential
        // A fault at 90 % with 87L off: where both Z2 see it, POTT trips the
        // end away from the fault at once instead of in Z2 time. With
        // distance off, both ends trip by 87L at once for a fault in the
        // middle.
        let (fed_a90, fed_b90) = fed(0.9);
        // POTT accelerates the far end, where both Z2 see the fault.
        let (_, _, pott_a, pott_b) = in_z1(0.9, false, true);
        if fed_a90
            && fed_b90
            && pott_a
            && pott_b
            && p.a
                .functions
                .iter()
                .any(|f| matches!(f, ProtectionFunction::Distance(d) if d.teleprotection.is_some()))
        {
            tests.push(ProtectionTest {
                name: format!("21 {name}: POTT at 90 %"),
                category: "distance".into(),
                description: format!(
                    "87L off: {} trips by POTT (its Z2 and the remote Z2 both see the fault)",
                    p.a.id
                ),
                fault_time: t0,
                t_end: t0 + 0.5,
                events: vec![fault(p.from, p.to, &p.circuit, 0.9, true)],
                disable: Vec::new(),
                disable_functions: vec!["87L".into()],
                stuck_breakers: Vec::new(),
                expect: vec![
                    Expectation::Trip {
                        relay: p.a.id.clone(),
                        function: Some("21 POTT".into()),
                        min: 0.0,
                        max: FAST + 0.02,
                    },
                    Expectation::Trip {
                        relay: p.b.id.clone(),
                        function: None,
                        min: 0.0,
                        max: FAST,
                    },
                    Expectation::OnlyTrips {
                        relays: only.clone(),
                        until: None,
                    },
                ],
            });
        }
        if has(p.a, "87L") && has(p.b, "87L") {
            tests.push(ProtectionTest {
                name: format!("87L {name}: internal fault"),
                category: "differential".into(),
                description: "Distance off: both ends trip by the line differential".into(),
                fault_time: t0,
                t_end: t0 + 0.4,
                events: vec![fault(p.from, p.to, &p.circuit, 0.5, true)],
                disable: Vec::new(),
                disable_functions: vec!["21".into()],
                stuck_breakers: Vec::new(),
                expect: vec![
                    Expectation::Trip {
                        relay: p.a.id.clone(),
                        function: Some("87L".into()),
                        min: 0.0,
                        max: FAST,
                    },
                    Expectation::Trip {
                        relay: p.b.id.clone(),
                        function: Some("87L".into()),
                        min: 0.0,
                        max: FAST,
                    },
                    Expectation::OnlyTrips {
                        relays: only.clone(),
                        until: None,
                    },
                ],
            });
        }
        // @block Reverse fault
        // Reverse fault: a line behind the near end, cleared by its own
        // relays; the near end must stay.
        if let Some(behind) = pairs.iter().find(|q| {
            !(q.from == p.from && q.to == p.to && q.circuit == p.circuit)
                && (q.from == p.from || q.to == p.from)
        }) {
            let loc = 0.5;
            tests.push(ProtectionTest {
                name: format!(
                    "21 {name}: reverse fault on {}",
                    super::setting::branch_tag(behind.from, behind.to, &behind.circuit)
                ),
                category: "selectivity".into(),
                description: format!("{} looks away from the fault and stays", p.a.id),
                fault_time: t0,
                t_end: t0 + 0.6,
                events: vec![fault(behind.from, behind.to, &behind.circuit, loc, true)],
                disable: Vec::new(),
                disable_functions: Vec::new(),
                stuck_breakers: Vec::new(),
                expect: vec![
                    Expectation::NoTrip {
                        relay: p.a.id.clone(),
                        function: None,
                    },
                    Expectation::OnlyTrips {
                        relays: vec![behind.a.id.clone(), behind.b.id.clone()],
                        until: Some(0.3),
                    },
                ],
            });
        }
        // @block Breaker failure
        // Breaker failure at the near end, where current still flows through
        // the stuck breaker once the far end has opened (a bus with nothing
        // behind it feeds no fault, and there is nothing to detect).
        let fed_stuck = branch.is_none_or(|e| {
            let alpha = 0.5;
            let (near, far) = if topo.branches[e].from_number == p.from {
                (super::End::From, super::End::To)
            } else {
                (super::End::To, super::End::From)
            };
            calc.solve_with(
                Some(super::setting::FaultPoint::Branch(e, alpha)),
                num_complex::Complex::new(0.0, line_x),
                &[(e, far)],
                &[],
            )
            .is_ok_and(|sc| {
                let i = if near == super::End::From {
                    sc.branch[e].0
                } else {
                    sc.branch[e].1
                };
                let bf = p.a.breaker_failure.as_ref().map_or(0.1, |b| b.current);
                let i_nom = p.a.i_nom.unwrap_or_else(|| {
                    if topo.branches[e].rate > 0.0 {
                        topo.branches[e].rate
                    } else {
                        1.0
                    }
                });
                i.norm() > 2.0 * bf * i_nom
            })
        });
        if p.a.breaker_failure.is_some() && fed_stuck {
            tests.push(ProtectionTest {
                name: format!("50BF {name}: stuck breaker at bus {}", p.from),
                category: "breaker_failure".into(),
                description: format!(
                    "{}'s breaker does not open; its 50BF trips the rest of bus {}",
                    p.a.id, p.from
                ),
                fault_time: t0,
                t_end: t0 + 0.6,
                events: vec![fault(p.from, p.to, &p.circuit, 0.5, true)],
                disable: Vec::new(),
                disable_functions: Vec::new(),
                stuck_breakers: vec![BreakerRef {
                    from: p.from,
                    to: p.to,
                    circuit: p.circuit.clone(),
                    at: p.from,
                    machine_id: None,
                }],
                expect: vec![
                    Expectation::Trip {
                        relay: p.a.id.clone(),
                        function: None,
                        min: 0.0,
                        max: FAST,
                    },
                    Expectation::BreakerFailure {
                        relay: p.a.id.clone(),
                    },
                ],
            });
        }
        // @block Auto-reclose
        // Auto-reclose. A transient fault dies out in the dead time and
        // both ends reclose (or the follower is held by the synchronism
        // check); a permanent one trips again and locks out.
        if p.a.autoreclose.is_some() && p.b.autoreclose.is_some() {
            let dead = p.a.autoreclose.as_ref().map_or(0.5, |a| a.dead_time);
            tests.push(ProtectionTest {
                name: format!("79 {name}: transient fault"),
                category: "autoreclose".into(),
                description:
                    "The fault dies out while the line is dead; the leader recloses, the follower closes under the synchronism check (or holds while the angle across is too large), and both stay".into(),
                fault_time: t0,
                // The follower waits up to 1 s after its 0.1 s for the angle.
                t_end: t0 + dead + 1.6,
                events: vec![fault(p.from, p.to, &p.circuit, 0.5, false)],
                disable: Vec::new(),
                disable_functions: Vec::new(),
                stuck_breakers: Vec::new(),
                expect: vec![
                    Expectation::Reclose {
                        relay: p.a.id.clone(),
                    },
                    Expectation::Reclose {
                        relay: p.b.id.clone(),
                    },
                    Expectation::OnlyTrips {
                        relays: only.clone(),
                        until: None,
                    },
                ],
            });
            tests.push(ProtectionTest {
                name: format!("79 {name}: permanent fault"),
                category: "autoreclose".into(),
                description: "Reclosing onto the fault trips again and locks out".into(),
                fault_time: t0,
                t_end: t0 + dead + 1.0,
                events: vec![fault(p.from, p.to, &p.circuit, 0.5, true)],
                disable: Vec::new(),
                disable_functions: Vec::new(),
                stuck_breakers: Vec::new(),
                expect: vec![
                    Expectation::Lockout {
                        relay: p.a.id.clone(),
                    },
                    Expectation::Lockout {
                        relay: p.b.id.clone(),
                    },
                ],
            });
        }
    }

    // @block Busbars
    // Busbars: internal fault, and Z2 backup when the busbar protection
    // fails. The busbar differential must clear the bus alone; with it off,
    // the remote line ends that measure the fault in Z2 (and have the infeed
    // to) trip in Z2 time and the ends at the bus stay.
    for r in scheme.relays.iter().filter(|r| r.enabled) {
        let RelayLocation::Busbar { bus } = &r.location else {
            continue;
        };
        let x_fault = bolted_bus_x(topo, *bus);
        let ev = vec![RmsEvent::BusFault {
            time: t0,
            bus: *bus,
            r: 0.0,
            x: x_fault,
        }];
        tests.push(ProtectionTest {
            name: format!("87B bus {bus}: busbar fault"),
            category: "busbar".into(),
            description: "The busbar differential clears the bus alone".into(),
            fault_time: t0,
            t_end: t0 + 0.5,
            events: ev.clone(),
            disable: Vec::new(),
            disable_functions: Vec::new(),
            stuck_breakers: Vec::new(),
            expect: vec![
                Expectation::Trip {
                    relay: r.id.clone(),
                    function: Some("87B".into()),
                    min: 0.0,
                    max: FAST,
                },
                // Units cut off by the bus trip may trip afterwards.
                Expectation::OnlyTrips {
                    relays: vec![r.id.clone()],
                    until: Some(FAST + 0.05),
                },
            ],
        });
        // The remote ends of the lines at this bus see it in Z2.
        let remote: Vec<&LineRelays> = pairs
            .iter()
            .filter(|p| p.from == *bus || p.to == *bus)
            .collect();
        if !remote.is_empty() {
            let mut expect = Vec::new();
            let mut t_end: f64 = 0.0;
            let calc = super::setting::FaultCalc::new(topo);
            // The fault the test applies, not a bolted one: its reactance,
            // times the infeed of the other lines, moves what the far
            // ends measure.
            let sc = topo.bus(*bus).ok().and_then(|b| {
                calc.solve(
                    super::setting::FaultPoint::Bus(b),
                    num_complex::Complex::new(0.0, x_fault),
                    &[],
                )
                .ok()
            });
            for p in &remote {
                let far = if p.from == *bus { p.b } else { p.a };
                let near = if p.from == *bus { p.a } else { p.b };
                // A far end fed by less than twice its minimum loop current
                // (weak infeed) is not expected to see the fault.
                let strong = topo
                    .branch(p.from, p.to, &p.circuit)
                    .ok()
                    .zip(sc.as_ref())
                    .is_none_or(|(e, sc)| {
                        let i = if p.from == *bus {
                            sc.branch[e].1
                        } else {
                            sc.branch[e].0
                        };
                        let i_nom = far.i_nom.unwrap_or_else(|| {
                            if topo.branches[e].rate > 0.0 {
                                topo.branches[e].rate
                            } else {
                                1.0
                            }
                        });
                        let imin = far
                            .functions
                            .iter()
                            .find_map(|f| match f {
                                ProtectionFunction::Distance(d) => Some(d.min_current),
                                _ => None,
                            })
                            .unwrap_or(0.2);
                        i.norm() > 2.0 * imin * i_nom
                    });
                // Expected where its Z2 holds what it measures for this fault
                // (the setting audit checks the bolted fault).
                let sees = topo
                    .branch(p.from, p.to, &p.circuit)
                    .ok()
                    .zip(sc.as_ref())
                    .is_none_or(|(e, sc)| {
                        let end = if p.from == *bus {
                            super::End::To
                        } else {
                            super::End::From
                        };
                        let at = if p.from == *bus {
                            topo.branches[e].to
                        } else {
                            topo.branches[e].from
                        };
                        calc.apparent(sc, e, end)
                            .is_none_or(|z| zone_sees(topo, far, "Z2", at, z))
                    });
                if let (Some(z2), true) = (zone_delay(far, "Z2"), strong && sees) {
                    t_end = t_end.max(z2 + mt + 0.15);
                    expect.push(Expectation::Trip {
                        relay: far.id.clone(),
                        function: Some("21 Z2".into()),
                        min: z2,
                        max: z2 + mt + 0.04,
                    });
                }
                expect.push(Expectation::NoTrip {
                    relay: near.id.clone(),
                    function: None,
                });
            }
            tests.push(ProtectionTest {
                name: format!("21 backup for bus {bus}"),
                category: "busbar".into(),
                description: "87B, 87L and POTT off: the remote line ends clear the bus fault in Z2 time; the ends at the bus look away".into(),
                fault_time: t0,
                t_end: t0 + t_end,
                events: ev,
                disable: vec![r.id.clone()],
                disable_functions: vec!["87L".into(), "POTT".into()],
                stuck_breakers: Vec::new(),
                expect,
            });
        }
    }

    // @block Transformers
    // Transformers: internal fault trips, external fault does not. A
    // winding fault trips 87T at once (a unit differential across it may
    // trip too); a cleared fault at the `to` bus leaves 87T stable.
    for r in scheme.relays.iter().filter(|r| r.enabled && has(r, "87T")) {
        let RelayLocation::Transformer { from, to, circuit } = &r.location else {
            continue;
        };
        // A unit differential across this transformer sees the fault too.
        let mut allowed = vec![r.id.clone()];
        allowed.extend(
            scheme
                .relays
                .iter()
                .filter(|g| g.enabled && has(g, "87U"))
                .filter(|g| matches!(&g.location, RelayLocation::Generator { bus, .. } if bus == from || bus == to))
                .map(|g| g.id.clone()),
        );
        tests.push(ProtectionTest {
            name: format!(
                "87T {}: internal fault",
                super::setting::branch_tag(*from, *to, circuit)
            ),
            category: "transformer".into(),
            description: "A winding fault at half the leakage impedance".into(),
            fault_time: t0,
            t_end: t0 + 0.4,
            events: vec![fault(*from, *to, circuit, 0.5, true)],
            disable: Vec::new(),
            disable_functions: Vec::new(),
            stuck_breakers: Vec::new(),
            expect: vec![
                Expectation::Trip {
                    relay: r.id.clone(),
                    function: Some("87T".into()),
                    min: 0.0,
                    max: FAST,
                },
                Expectation::OnlyTrips {
                    relays: allowed,
                    until: Some(FAST + 0.05),
                },
            ],
        });
        let lv = *to;
        let bus_relay = relay(&format!("B{lv}")).map(|b| b.id.clone());
        tests.push(ProtectionTest {
            name: format!(
                "87T {}: through fault at bus {lv}",
                super::setting::branch_tag(*from, *to, circuit)
            ),
            category: "transformer".into(),
            description: "The differential stays stable for a fault outside its zone".into(),
            fault_time: t0,
            t_end: t0 + 0.3,
            events: vec![
                RmsEvent::BusFault {
                    time: t0,
                    bus: lv,
                    r: 0.0,
                    x: bolted_bus_x(topo, lv),
                },
                RmsEvent::ClearFault {
                    time: t0 + 0.15,
                    bus: lv,
                },
            ],
            disable: bus_relay.into_iter().collect(),
            disable_functions: Vec::new(),
            stuck_breakers: Vec::new(),
            expect: vec![Expectation::NoTrip {
                relay: r.id.clone(),
                function: Some("87T".into()),
            }],
        });
    }

    // @block Units
    // Units. A terminal fault trips 87U at once; an inverter rides through
    // a 100 ms fault at its bus and trips by its envelope during a 1 s one.
    for r in scheme.relays.iter().filter(|r| r.enabled) {
        match &r.location {
            RelayLocation::Generator { bus, machine_id } if has(r, "87U") => {
                tests.push(ProtectionTest {
                    name: format!("87U {bus}-{machine_id}: terminal fault"),
                    category: "generator".into(),
                    description: "A fault between the generator and its step-up transformer".into(),
                    fault_time: t0,
                    t_end: t0 + 0.4,
                    events: vec![RmsEvent::BusFault {
                        time: t0,
                        bus: *bus,
                        r: 0.0,
                        x: bolted_bus_x(topo, *bus),
                    }],
                    disable: Vec::new(),
                    disable_functions: Vec::new(),
                    stuck_breakers: Vec::new(),
                    expect: vec![Expectation::Trip {
                        relay: r.id.clone(),
                        function: Some("87U".into()),
                        min: 0.0,
                        max: FAST,
                    }],
                });
            }
            RelayLocation::Inverter { bus, machine_id } => {
                let unit_ok = topo.units.iter().any(|u| {
                    u.bus_number == *bus
                        && u.machine_id == *machine_id
                        && u.kind == UnitKind::Inverter
                });
                if !unit_ok {
                    continue;
                }
                let bus_relay = relay(&format!("B{bus}")).map(|b| b.id.clone());
                let disable: Vec<String> = bus_relay.into_iter().collect();
                tests.push(ProtectionTest {
                    name: format!("FRT {bus}-{machine_id}: 100 ms fault rides through"),
                    category: "inverter".into(),
                    description:
                        "A bolted fault at the unit's bus cleared within the 150 ms of the profile: the ride-through envelope does not trip (frequency stages may still answer the swing after it)"
                            .into(),
                    fault_time: t0,
                    t_end: t0 + 1.0,
                    events: vec![
                        RmsEvent::BusFault {
                            time: t0,
                            bus: *bus,
                            r: 0.0,
                            x: bolted_bus_x(topo, *bus),
                        },
                        RmsEvent::ClearFault {
                            time: t0 + 0.1,
                            bus: *bus,
                        },
                    ],
                    disable: disable.clone(),
                    disable_functions: Vec::new(),
                    stuck_breakers: Vec::new(),
                    expect: vec![Expectation::NoTrip {
                        relay: r.id.clone(),
                        function: Some("FRT".into()),
                    }],
                });
                tests.push(ProtectionTest {
                    name: format!("FRT {bus}-{machine_id}: 1 s fault trips"),
                    category: "inverter".into(),
                    description:
                        "The voltage stays below the profile: the unit trips after the profile's 150 ms at zero voltage and before the fault clears, where on the ramp set by the voltage left at its terminal".into(),
                    fault_time: t0,
                    t_end: t0 + 1.5,
                    events: vec![
                        RmsEvent::BusFault {
                            time: t0,
                            bus: *bus,
                            r: 0.0,
                            x: bolted_bus_x(topo, *bus),
                        },
                        RmsEvent::ClearFault {
                            time: t0 + 1.0,
                            bus: *bus,
                        },
                    ],
                    disable,
                    disable_functions: Vec::new(),
                    stuck_breakers: Vec::new(),
                    expect: vec![Expectation::Trip {
                        relay: r.id.clone(),
                        function: Some("FRT".into()),
                        min: 0.15,
                        max: 1.0 + mt + 0.02,
                    }],
                });
            }
            _ => {}
        }
    }
    // @block Unique names
    // Names identify a test (the UI keys on them): number any repeats.
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for t in &mut tests {
        let n = seen.entry(t.name.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            t.name = format!("{} ({n})", t.name);
        }
    }
    tests
}

// @block Running the suite
// A suite's outcome: the audit of the scheme and every test run in RMS,
// spread over the machine's cores in native builds and one after the other
// in the browser.

/// A whole suite's outcome.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SuiteReport {
    pub audit: Vec<super::setting::Finding>,
    pub tests: Vec<TestOutcome>,
    pub passed: usize,
    pub failed: usize,
}

/// Audit a scheme and run its standard tests in RMS.
#[must_use]
pub fn run_suite(
    sim: &RmsSimulation,
    scheme: &ProtectionScheme,
    opts: &SuiteOptions,
) -> SuiteReport {
    let topo = sim.topology();
    let audit = super::setting::audit(topo, scheme);
    let tests = run_tests(sim, scheme, &standard_suite(topo, scheme, opts));
    let passed = tests.iter().filter(|t| t.passed).count();
    SuiteReport {
        failed: tests.len() - passed,
        passed,
        audit,
        tests,
    }
}

/// Run `tests` in RMS, in their order. The runs are independent, so native
/// builds spread them over the machine's cores; the browser build runs them
/// one after the other (on a worker of their own, apart from the RMS
/// session).
#[must_use]
pub fn run_tests(
    sim: &RmsSimulation,
    scheme: &ProtectionScheme,
    tests: &[ProtectionTest],
) -> Vec<TestOutcome> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        let threads = cores.min(tests.len()).max(1);
        if threads > 1 {
            let chunk = tests.len().div_ceil(threads);
            return std::thread::scope(|s| {
                let handles: Vec<_> = tests
                    .chunks(chunk)
                    .map(|part| {
                        s.spawn(move || {
                            part.iter()
                                .map(|t| run_rms(sim, scheme, t))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|h| h.join().expect("a test thread panicked"))
                    .collect()
            });
        }
    }
    tests.iter().map(|t| run_rms(sim, scheme, t)).collect()
}
