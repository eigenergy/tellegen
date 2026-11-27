//! The experimental environment switches of the RMS engine (`TELLEGEN_*`),
//! each against what its documentation states: what it writes, what it
//! changes, and that a switch documented to leave a run unchanged does so
//! bit for bit or within the stated tolerance.
//!
//! The engine reads each switch once per process, so a test here never sets
//! one in its own process: it runs this test binary again as a child
//! ([`child_run`], ignored when run by hand) with the switches set, the child
//! writes what its run recorded to a file, and the test compares that with
//! a run in its own process without the switch. Floats travel as their bit
//! patterns, so "identical" means bit for bit.
//!
//! The cases are built from the shipped multi-machine test case (the whole
//! system, and a single-machine case built from one of its units); without
//! its data (the published crate) every test here is skipped.

#![cfg(feature = "sensitivity")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use tellegen::rms::{
    LoadModel, LowVoltageLoad, RmsEvent, RmsOptions, RmsRequest, RmsResult, RmsSimulation,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/wecc240.rs"
));

// The child process.

/// Where the child writes its run (set by the parent; without it the child
/// does nothing, so running the ignored tests by hand is harmless).
const CHILD_OUT: &str = "RMS_KNOB_CHILD_OUT";
/// The case the child runs ([`case`]).
const CHILD_CASE: &str = "RMS_KNOB_CHILD_CASE";
/// The child's `RmsOptions`, as JSON.
const CHILD_OPTIONS: &str = "RMS_KNOB_CHILD_OPTIONS";
/// When set, the child hands the engine a browser clock before its run.
const CHILD_SET_CLOCK: &str = "RMS_KNOB_CHILD_SET_CLOCK";

/// What a run recorded, in a form that survives the trip through a file bit
/// for bit.
#[derive(Debug, Clone)]
struct Run {
    time: Vec<f64>,
    channels: BTreeMap<String, Vec<f64>>,
    steps: usize,
    newton_iterations: usize,
    notes: Vec<String>,
}

impl From<RmsResult> for Run {
    fn from(r: RmsResult) -> Self {
        Self {
            time: r.time,
            channels: r.channels,
            steps: r.steps,
            newton_iterations: r.newton_iterations,
            notes: r.notes,
        }
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn from_bits(v: &serde_json::Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| f64::from_bits(b.as_u64().unwrap()))
        .collect()
}

/// The child's run and the engine's statistics lines after it.
struct Child {
    run: Result<Run, String>,
    lu_stats: String,
    region_stats: String,
}

/// Run by the tests below in a child process with the switches under test
/// set: runs the case named in `RMS_KNOB_CHILD_CASE` and writes the result
/// and the statistics lines to `RMS_KNOB_CHILD_OUT`.
#[test]
#[ignore = "run by the other tests in a child process with the switches set"]
fn child_run() {
    let Some(out) = std::env::var_os(CHILD_OUT) else {
        return;
    };
    let name = std::env::var(CHILD_CASE).expect("the case");
    let options: RmsOptions =
        serde_json::from_str(&std::env::var(CHILD_OPTIONS).expect("the options")).unwrap();
    if std::env::var_os(CHILD_SET_CLOCK).is_some() {
        tellegen::rms::set_clock(|| 0.0);
    }
    let (sim, request) = case(&name, options).expect("the case data");
    let run = sim.run(&request);
    let value = match run {
        Ok(r) => serde_json::json!({
            "ok": {
                "time": bits(&r.time),
                "channels": r.channels.iter().map(|(k, v)| (k.clone(), bits(v))).collect::<BTreeMap<_, _>>(),
                "steps": r.steps,
                "newton_iterations": r.newton_iterations,
                "notes": r.notes,
            }
        }),
        Err(e) => serde_json::json!({ "err": e }),
    };
    let value = serde_json::json!({
        "run": value,
        "lu_stats": tellegen::rms::lu_stats(),
        "region_stats": tellegen::rms::region_stats(),
    });
    std::fs::write(out, serde_json::to_string(&value).unwrap()).unwrap();
}

/// A fresh path for a file of this test run.
fn scratch(tag: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rms_env_knobs");
    std::fs::create_dir_all(&dir).unwrap();
    let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("{}-{k}-{tag}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// Run `name` with `options` in a child process with the environment `env`.
fn in_child(name: &str, options: RmsOptions, env: &[(&str, &str)]) -> Child {
    let out = scratch(&format!("{name}.json"));
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "child_run",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_OUT, &out)
        .env(CHILD_CASE, name)
        .env(CHILD_OPTIONS, serde_json::to_string(&options).unwrap())
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "the child run of {name} with {env:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&out).unwrap_or_else(|e| {
        panic!(
            "the child run of {name} with {env:?} wrote nothing ({e}):\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let _ = std::fs::remove_file(&out);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let run = match &v["run"] {
        serde_json::Value::Object(m) if m.contains_key("ok") => {
            let r = &m["ok"];
            Ok(Run {
                time: from_bits(&r["time"]),
                channels: r["channels"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, s)| (k.clone(), from_bits(s)))
                    .collect(),
                steps: r["steps"].as_u64().unwrap() as usize,
                newton_iterations: r["newton_iterations"].as_u64().unwrap() as usize,
                notes: r["notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str().unwrap().to_owned())
                    .collect(),
            })
        }
        other => Err(other["err"].as_str().unwrap().to_owned()),
    };
    Child {
        run,
        lu_stats: v["lu_stats"].as_str().unwrap().to_owned(),
        region_stats: v["region_stats"].as_str().unwrap().to_owned(),
    }
}

/// Run `name` with `options` in this process (no switch set).
fn here(name: &str, options: RmsOptions) -> Result<Run, String> {
    let (sim, request) = case(name, options).expect("the case data");
    sim.run(&request).map(Run::from)
}

// The cases.

fn network(raw: &str) -> powerio::BalancedNetwork {
    let src = powerio::Source::from_memory("case.raw", raw.as_bytes().to_vec()).unwrap();
    let module = powerio::parse(src).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    net.clone()
}

fn fault(bus: usize, x: f64, on: f64, off: f64) -> Vec<RmsEvent> {
    vec![
        RmsEvent::BusFault {
            time: on,
            bus,
            r: 0.0,
            x,
        },
        RmsEvent::ClearFault { time: off, bus },
    ]
}

/// The bus of the multi-machine case the faults of [`case`] are at.
const FAULT_BUS: usize = 1101;

/// A short case by name, with `options`:
/// - `smib`: GENROU + SEXS + TGOV1 against an infinite bus, a fault through
///   half the line reactance from 1.0 to 1.1 s that drives the field voltage
///   to its ceiling, 2 s at 5 ms;
/// - `smib-coarse`: the same unit with steps of 100 s and a lasting fault at
///   100 s, where trapezoidal steps fail and fall back;
/// - `pqbrak` and `pqbrak-near`: the same unit with a 60 MW constant-power
///   load at its bus and a fault that holds the bus near 0.66 pu for 0.15 s;
///   the load converts at 0.7 pu (`pqbrak`) or at 95 % of its power flow
///   voltage (`pqbrak-near`);
/// - `grid`: the multi-machine case, a fault at bus 1101 from 0.1 to 0.2 s,
///   0.5 s at 5 ms;
/// - `grid-collapse`: the multi-machine case with the exact load laws, every
///   load scaled by 10 at 0.1 s, which leaves a network that cannot be solved;
/// - `grid-head`: the multi-machine case with the exact load laws, without
///   events, up to 0.1 s.
///
/// `None` without the shipped data.
fn case(name: &str, options: RmsOptions) -> Option<(RmsSimulation, RmsRequest)> {
    let request = |t_end: f64, dt: f64, output_step: f64, events: Vec<RmsEvent>| RmsRequest {
        t_end,
        dt,
        output_step,
        events,
        ..RmsRequest::default()
    };
    match name {
        "smib" | "smib-coarse" | "pqbrak" | "pqbrak-near" => {
            shipped_wecc240()?;
            let case = &wecc240_smib_cases()[0];
            let x = case.x_line;
            let (net, dyr) = smib_study(case);
            Some(match name {
                "smib" => (
                    RmsSimulation::with_options(&net, &dyr, LoadModel::PowerFlow, options).unwrap(),
                    request(2.0, 0.005, 0.005, fault(102, 0.5 * x, 1.0, 1.1)),
                ),
                "smib-coarse" => (
                    RmsSimulation::with_options(&net, &dyr, LoadModel::PowerFlow, options).unwrap(),
                    request(
                        1000.0,
                        100.0,
                        100.0,
                        vec![RmsEvent::BusFault {
                            time: 100.0,
                            bus: 102,
                            r: 0.0,
                            x: 0.05 * x,
                        }],
                    ),
                ),
                _ => {
                    let raw = with_load(
                        &case.raw,
                        "102,'1 ',1,   1,   1,    60.000,    10.000,     0.000,     0.000,     0.000,     0.000,   1,1",
                    );
                    let v_break = if name == "pqbrak" { 0.7 } else { -0.95 };
                    let options = RmsOptions {
                        low_voltage_load: LowVoltageLoad::Break { v_break },
                        ..options
                    };
                    (
                        RmsSimulation::with_options(
                            &network(&raw),
                            &dyr,
                            LoadModel::PowerFlow,
                            options,
                        )
                        .unwrap(),
                        request(2.0, 0.005, 0.005, fault(102, 0.8 * x, 1.0, 1.15)),
                    )
                }
            })
        }
        "grid" | "grid-collapse" | "grid-head" => {
            let dir = shipped_wecc240()?;
            let net = shipped_wecc240_network()?;
            let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap())
                .unwrap();
            if name == "grid" {
                let sim =
                    RmsSimulation::with_options(&net, &dyr, LoadModel::PowerFlow, options).unwrap();
                return Some((
                    sim,
                    request(0.5, 0.005, 0.01, fault(FAULT_BUS, 0.01, 0.1, 0.2)),
                ));
            }
            let options = RmsOptions {
                low_voltage_load: LowVoltageLoad::Exact,
                ..options
            };
            let sim =
                RmsSimulation::with_options(&net, &dyr, LoadModel::PowerFlow, options).unwrap();
            if name == "grid-head" {
                return Some((sim, request(0.1, 0.005, 0.01, Vec::new())));
            }
            let events = sim
                .buses()
                .iter()
                .map(|&bus| RmsEvent::LoadChange {
                    time: 0.1,
                    bus,
                    scale: 10.0,
                })
                .collect();
            Some((sim, request(0.3, 0.005, 0.01, events)))
        }
        _ => panic!("no case {name}"),
    }
}

// Comparisons.

/// Every channel of `a` and `b` bit for bit, with the same sample times,
/// steps and Newton iterations.
fn assert_identical(a: &Run, b: &Run, what: &str) {
    assert_eq!(bits(&a.time), bits(&b.time), "{what}: sample times");
    assert_eq!(
        a.channels.keys().collect::<Vec<_>>(),
        b.channels.keys().collect::<Vec<_>>(),
        "{what}: channels"
    );
    for (k, s) in &a.channels {
        assert!(
            bits(s) == bits(&b.channels[k]),
            "{what}: channel {k} differs by up to {:e}",
            max_diff(s, &b.channels[k])
        );
    }
    assert_eq!(
        (a.steps, a.newton_iterations),
        (b.steps, b.newton_iterations),
        "{what}: steps and Newton iterations"
    );
}

/// Largest difference of two series (infinite when either has a NaN).
fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            let d = (x - y).abs();
            if d.is_nan() {
                f64::INFINITY
            } else {
                d
            }
        })
        .fold(0.0, f64::max)
}

/// The largest difference over every channel of two runs with the same
/// sample times, and the channel it is in.
fn largest_diff(a: &Run, b: &Run) -> (f64, String) {
    assert_eq!(bits(&a.time), bits(&b.time), "sample times");
    assert_eq!(
        a.channels.keys().collect::<Vec<_>>(),
        b.channels.keys().collect::<Vec<_>>()
    );
    a.channels
        .iter()
        .map(|(k, s)| (max_diff(s, &b.channels[k]), k.clone()))
        .fold((0.0, String::new()), |m, d| if d.0 > m.0 { d } else { m })
}

/// The number right after the first `key` in a statistics line.
fn stat(text: &str, key: &str) -> f64 {
    let k = text
        .find(key)
        .unwrap_or_else(|| panic!("no {key:?} in {text}"));
    let rest = &text[k + key.len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
        .unwrap_or(rest.len());
    rest[..end]
        .parse()
        .unwrap_or_else(|e| panic!("{key:?} in {text}: {e}"))
}

/// How far two runs may be apart that solve the same steps to Newton's
/// tolerance (corrections below 1e-10) along different iterations: a
/// different factorization, refresh schedule or fallback path. On these
/// cases the default tolerance's own error, against a run with a hundred
/// times tighter tolerance, is near 2e-5 on branch flows and 4e-7 on rotor
/// angles; the switches below stay three orders of magnitude inside that.
const SAME_SOLUTION: f64 = 1e-8;

/// One line of the trace file (`TELLEGEN_TRACE`).
#[derive(Debug)]
enum TraceLine {
    /// An accepted step: Newton iterations, factorizations refreshed in it,
    /// the step, the integration weight, the residual after each iteration and
    /// the variables where it was largest (with `TELLEGEN_TRACE_WHERE`).
    Step {
        iters: usize,
        refreshes: usize,
        h: f64,
        theta: f64,
        res: Vec<f64>,
        at: Vec<String>,
    },
    /// A re-solve after an event, the same fields without the step.
    Reinit {
        iters: usize,
        res: Vec<f64>,
        at: Vec<String>,
    },
    /// A failed attempt: the step, the weight, the depth of halving, the
    /// residuals and why.
    Fail {
        h: f64,
        theta: f64,
        depth: usize,
        why: String,
    },
}

/// The lines of a trace file, each checked against its documented format:
/// `<us> <iters> <refreshes> <h> <theta> <res,...> <where|...>`,
/// `REINIT <us> <iters> <refreshes> <res,...> <where|...>` and
/// `FAIL <us> <h> <theta> <depth> <res,...> | <why>`.
fn parse_trace(text: &str) -> Vec<TraceLine> {
    let residuals = |s: &str, line: &str| -> Vec<f64> {
        if s.is_empty() {
            return Vec::new();
        }
        s.split(',')
            .map(|v| v.parse().unwrap_or_else(|e| panic!("{line}: {v}: {e}")))
            .collect()
    };
    let places = |s: &str| -> Vec<String> {
        if s.is_empty() {
            Vec::new()
        } else {
            s.split('|').map(str::to_owned).collect()
        }
    };
    text.lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("FAIL ") {
                let (head, why) = rest.split_once(" | ").unwrap_or_else(|| panic!("{line}"));
                let f: Vec<&str> = head.split(' ').collect();
                assert_eq!(f.len(), 5, "{line}");
                f[0].parse::<u128>()
                    .unwrap_or_else(|e| panic!("{line}: {e}"));
                residuals(f[4], line);
                TraceLine::Fail {
                    h: f[1].parse().unwrap(),
                    theta: f[2].parse().unwrap(),
                    depth: f[3].parse().unwrap(),
                    why: why.to_owned(),
                }
            } else if let Some(rest) = line.strip_prefix("REINIT ") {
                let f: Vec<&str> = rest.split(' ').collect();
                assert_eq!(f.len(), 5, "{line}");
                f[0].parse::<u128>()
                    .unwrap_or_else(|e| panic!("{line}: {e}"));
                f[2].parse::<usize>()
                    .unwrap_or_else(|e| panic!("{line}: {e}"));
                TraceLine::Reinit {
                    iters: f[1].parse().unwrap(),
                    res: residuals(f[3], line),
                    at: places(f[4]),
                }
            } else {
                let f: Vec<&str> = line.split(' ').collect();
                assert_eq!(f.len(), 7, "{line}");
                f[0].parse::<u128>()
                    .unwrap_or_else(|e| panic!("{line}: {e}"));
                TraceLine::Step {
                    iters: f[1].parse().unwrap(),
                    refreshes: f[2].parse().unwrap(),
                    h: f[3].parse().unwrap(),
                    theta: f[4].parse().unwrap(),
                    res: residuals(f[5], line),
                    at: places(f[6]),
                }
            }
        })
        .collect()
}

/// Run `name` in a child with `TELLEGEN_TRACE` (and `env`) and return the
/// run and the trace's lines.
fn traced(name: &str, options: RmsOptions, env: &[(&str, &str)]) -> (Run, Vec<TraceLine>) {
    let path = scratch("trace.txt");
    let mut env = env.to_vec();
    env.push(("TELLEGEN_TRACE", path.to_str().unwrap()));
    let run = in_child(name, options, &env).run.unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    (run, parse_trace(&text))
}

/// The harness itself: a child run without any switch is this process's
/// run bit for bit, so every difference the tests below see is the switch's.
#[test]
fn a_child_without_switches_runs_as_this_process() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    for name in ["smib", "grid"] {
        let child = in_child(name, d, &[]).run.unwrap();
        assert_identical(&child, &here(name, d).unwrap(), name);
    }
}

/// `TELLEGEN_PARTIAL=1`: a run that cannot go on (here every load scaled by
/// 10 at 0.1 s, a network the re-solve cannot solve)
/// returns what it recorded up to the last output time, with the error a run
/// without the switch returns as a note. The samples are those of a run
/// that ends at that time, bit for bit.
#[test]
fn a_partial_run_returns_the_samples_before_the_failure() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let error = here("grid-collapse", d).unwrap_err();
    assert!(
        error.starts_with("re-solving the network at t = 0.1000 s"),
        "{error}"
    );
    let partial = in_child("grid-collapse", d, &[("TELLEGEN_PARTIAL", "1")])
        .run
        .unwrap_or_else(|e| panic!("with TELLEGEN_PARTIAL the run still fails: {e}"));
    let note = format!("stopped early (TELLEGEN_PARTIAL): {error}");
    assert!(
        partial.notes.contains(&note),
        "{note} not in {:?}",
        partial.notes
    );
    assert!(
        (partial.time.last().unwrap() - 0.1).abs() < 1e-12,
        "{:?}",
        partial.time.last()
    );
    assert_identical(
        &partial,
        &here("grid-head", d).unwrap(),
        "partial run against a run to 0.1 s",
    );
    // The switch is `1` only.
    let other = in_child("grid-collapse", d, &[("TELLEGEN_PARTIAL", "yes")]);
    assert_eq!(other.run.unwrap_err(), error);
}

/// `TELLEGEN_TRACE=<file>`: one line per accepted step with its Newton
/// iterations, refreshed factorizations, step, weight and the residual after
/// each iteration (falling to the stopping test's level), a `REINIT` line per
/// re-solve after an event and a `FAIL` line per failed attempt (on this case
/// each re-solve with the device states free meets a zero pivot and falls
/// back). The step lines account for every step of the run, the step and
/// re-solve lines for every Newton iteration it reports, and the run is the
/// same bit for bit. Without `TELLEGEN_TRACE_WHERE` no variable is named.
#[test]
fn the_trace_has_a_line_per_step_and_leaves_the_run_unchanged() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let (run, lines) = traced("grid", d, &[]);
    assert_identical(&run, &here("grid", d).unwrap(), "with the trace");
    let (mut steps, mut reinits, mut iterations) = (0, 0, 0);
    for line in &lines {
        match line {
            TraceLine::Step {
                iters,
                refreshes,
                h,
                theta,
                res,
                at,
            } => {
                steps += 1;
                iterations += iters;
                assert!(*iters >= 1 && *refreshes <= *iters, "{line:?}");
                assert!((h - 0.005).abs() < 1e-12 && *theta == 0.5, "{line:?}");
                assert!(!res.is_empty() && res.len() <= *iters, "{line:?}");
                assert!(*res.last().unwrap() < 1e-6, "{line:?}");
                assert!(at.is_empty(), "{line:?}");
            }
            TraceLine::Reinit { iters, res, at } => {
                reinits += 1;
                iterations += iters;
                assert_eq!(res.len(), *iters, "{line:?}");
                assert!(*res.last().unwrap() < 1e-6, "{line:?}");
                assert!(at.is_empty(), "{line:?}");
            }
            TraceLine::Fail {
                h,
                theta,
                depth,
                why,
            } => {
                assert!(*h == 0.0 && *theta == 0.5 && *depth == 0, "{line:?}");
                assert!(
                    why.starts_with("reinit: ") && why.contains("zero pivot"),
                    "{line:?}"
                );
            }
        }
    }
    assert_eq!(steps, run.steps);
    // The fault and its clearing.
    assert_eq!(reinits, 2);
    assert_eq!(iterations, run.newton_iterations);
    assert!(lines.iter().any(|l| matches!(l, TraceLine::Fail { .. })));
}

/// `TELLEGEN_TRACE_WHERE` with the trace: each step line also names, for its
/// first iterations (every iteration of a re-solve), the variable with the
/// largest residual, spaces as underscores, `|` between iterations. The run
/// is the same bit for bit.
#[test]
fn trace_where_names_the_largest_residual() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let (run, lines) = traced("grid", d, &[("TELLEGEN_TRACE_WHERE", "1")]);
    assert_identical(&run, &here("grid", d).unwrap(), "with the trace and places");
    let named = |at: &[String]| {
        at.iter().all(|n| {
            !n.contains(' ') && (n.starts_with("bus_") || n.starts_with('G') || n.starts_with('R'))
        })
    };
    let mut checked = 0;
    for line in &lines {
        match line {
            TraceLine::Step { res, at, .. } => {
                assert!(!at.is_empty() && at.len() <= res.len().min(5), "{line:?}");
                assert!(named(at), "{line:?}");
                checked += 1;
            }
            TraceLine::Reinit { res, at, .. } => {
                assert_eq!(at.len(), res.len(), "{line:?}");
                assert!(named(at), "{line:?}");
            }
            TraceLine::Fail { .. } => {}
        }
    }
    assert_eq!(checked, run.steps);
}

/// The trace's `FAIL` lines for steps: with 100 s steps through a lasting
/// fault, the first trapezoidal step after it fails
/// (`FAIL <us> 100 0.5 0 <residuals> | <why>`), then its backward-Euler
/// fallback (weight 1), and the step is halved: every failed attempt and
/// every step taken has a step of `100 / 2^depth` with a depth of at most 10
/// and a weight of 0.5 or 1. The steps taken with weight 1 are the fallbacks
/// the run notes, and the run is the same bit for bit.
#[test]
fn the_trace_names_failed_steps_and_their_fallback() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let (run, lines) = traced("smib-coarse", d, &[]);
    assert_identical(&run, &here("smib-coarse", d).unwrap(), "with the trace");
    let halved = |h: f64| (0..=10).any(|k| h == 100.0 / f64::from(1 << k));
    let mut first = Vec::new();
    let mut euler = 0;
    for line in &lines {
        match line {
            TraceLine::Fail {
                h,
                theta,
                depth,
                why,
            } => {
                assert!(
                    *depth <= 10 && *h == 100.0 / f64::from(1 << depth),
                    "{line:?}"
                );
                assert!(*theta == 0.5 || *theta == 1.0, "{line:?}");
                assert!(!why.is_empty() && !why.starts_with("reinit"), "{line:?}");
                if *depth == 0 {
                    first.push(*theta);
                }
            }
            TraceLine::Step { h, theta, .. } => {
                assert!(halved(*h) && (*theta == 0.5 || *theta == 1.0), "{line:?}");
                if *theta == 1.0 {
                    euler += 1;
                }
            }
            TraceLine::Reinit { .. } => {}
        }
    }
    assert_eq!(first, [0.5, 1.0], "the first failure, then its fallback");
    assert!(euler > 0);
    let note = format!("{euler} step(s) fell back from the trapezoidal rule to backward Euler");
    assert!(
        run.notes.iter().any(|n| n.starts_with(&note)),
        "{note} not in {:?}",
        run.notes
    );
}

/// `TELLEGEN_DIAG_STEP=<k>,<k>` with `TELLEGEN_DIAG_OUT=<file>`: the Newton
/// solve of the step after `k` accepted steps is analysed on copies and
/// appended to the file, one `=== step after k accepted steps, h, theta, nx`
/// section per listed step, with the pure Newton iterations by class of
/// state and the Jacobian check along the last correction and its parts.
/// The run is the same bit for bit.
#[test]
fn diag_step_writes_its_analysis_and_leaves_the_run_unchanged() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let path = scratch("diag.txt");
    let child = in_child(
        "smib",
        d,
        &[
            ("TELLEGEN_DIAG_STEP", "0, 250"),
            ("TELLEGEN_DIAG_OUT", path.to_str().unwrap()),
        ],
    );
    assert_identical(
        &child.run.unwrap(),
        &here("smib", d).unwrap(),
        "with the analysis",
    );
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let sections: Vec<&str> = text.split("=== ").filter(|s| !s.is_empty()).collect();
    assert_eq!(sections.len(), 2, "{text}");
    let mut sizes = Vec::new();
    for (section, k) in sections.iter().zip([0, 250]) {
        let head = section.lines().next().unwrap();
        let rest = head
            .strip_prefix(&format!("step after {k} accepted steps, h = "))
            .unwrap_or_else(|| panic!("{head}"));
        let (h, rest) = rest.split_once(", theta = ").unwrap();
        let (theta, nx) = rest.split_once(", nx = ").unwrap();
        assert!((h.parse::<f64>().unwrap() - 0.005).abs() < 1e-12, "{head}");
        assert_eq!(theta, "0.5", "{head}");
        sizes.push(nx.parse::<usize>().unwrap());
        for part in [
            "-- pure Newton iteration 0: max |r| ",
            "   max |dx| ",
            "     r  bus Vr",
            "-- Jacobian check along the last correction: max |A v| ",
            "   forward difference - A v, largest: ",
            "   forward - backward difference (kinks), largest: ",
            "-- Jacobian check along the last correction's part in [bus Vr]",
            "-- Jacobian check along the last correction's part in [machine delta]",
        ] {
            assert!(section.contains(part), "no {part:?} in\n{section}");
        }
    }
    assert!(sizes[0] > 4 && sizes[0] == sizes[1], "{sizes:?}");
}

/// `TELLEGEN_TOL=<x>` scales the Newton tolerances by `x / 1e-10`: at 1e-10
/// the run is the default one bit for bit; looser (1e-6) takes fewer
/// iterations and stays within 1e-4 of the default; tighter (1e-11, 1e-12)
/// takes more, and the two tighter runs agree far better with each other
/// than with the default, as solutions converging with the tolerance do.
#[test]
fn tellegen_tol_scales_the_newton_tolerance() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let base = here("grid", d).unwrap();
    let with = |tol: &str| in_child("grid", d, &[("TELLEGEN_TOL", tol)]).run.unwrap();
    assert_identical(&with("1e-10"), &base, "TELLEGEN_TOL=1e-10");
    let loose = with("1e-6");
    assert!(loose.newton_iterations < base.newton_iterations);
    let (dl, k) = largest_diff(&loose, &base);
    assert!(
        dl > 0.0 && dl < 1e-4,
        "1e-6 against the default: {dl:e} in {k}"
    );
    let (t11, t12) = (with("1e-11"), with("1e-12"));
    assert!(t11.newton_iterations > base.newton_iterations);
    assert!(t12.newton_iterations >= t11.newton_iterations);
    let (dd, _) = largest_diff(&t12, &base);
    let (dt, _) = largest_diff(&t12, &t11);
    assert!(
        dt < 1e-2 * dd,
        "1e-12 against 1e-11 {dt:e}, against the default {dd:e}"
    );
}

/// `TELLEGEN_THETA=<θ>` weights the step method: 0.5 is the trapezoidal
/// default bit for bit; 1.0 is backward Euler, which stays stable and close
/// to the trapezoidal run (rotor angle within 0.02 rad, speed within 1e-3 pu
/// through a fault) and damps the swing after it at least as much.
#[test]
fn theta_one_is_backward_euler_close_to_the_trapezoidal_rule() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let trap = here("smib", d).unwrap();
    let half = in_child("smib", d, &[("TELLEGEN_THETA", "0.5")])
        .run
        .unwrap();
    assert_identical(&half, &trap, "TELLEGEN_THETA=0.5");
    let euler = in_child("smib", d, &[("TELLEGEN_THETA", "1.0")])
        .run
        .unwrap();
    assert!(euler.channels.values().flatten().all(|v| v.is_finite()));
    let dd = max_diff(&euler.channels["delta:102"], &trap.channels["delta:102"]);
    let dw = max_diff(&euler.channels["omega:102"], &trap.channels["omega:102"]);
    assert!(dd > 1e-4 && dd < 0.02, "rotor angle {dd:e}");
    assert!(dw > 0.0 && dw < 1e-3, "speed {dw:e}");
    let swing = |r: &Run| {
        r.time
            .iter()
            .zip(&r.channels["omega:102"])
            .filter(|(t, _)| **t >= 1.5)
            .map(|(_, w)| (w - 1.0).abs())
            .fold(0.0, f64::max)
    };
    assert!(
        swing(&euler) <= swing(&trap),
        "{} {}",
        swing(&euler),
        swing(&trap)
    );
}

/// `TELLEGEN_FLOOR=0` switches off taking a Newton iteration that stagnates
/// at the rounding floor: the counter of such steps in `lu_stats` is then 0
/// (it is not without the switch on this case), and the run agrees with the
/// default within 1e-9.
#[test]
fn the_rounding_floor_exit_does_not_change_the_solution() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let base = in_child("pqbrak", d, &[]);
    assert!(stat(&base.lu_stats, "steps taken at the rounding floor ") > 0.0);
    let off = in_child("pqbrak", d, &[("TELLEGEN_FLOOR", "0")]);
    assert_eq!(
        stat(&off.lu_stats, "steps taken at the rounding floor "),
        0.0
    );
    let (dmax, k) = largest_diff(&off.run.unwrap(), &base.run.unwrap());
    assert!(dmax < 1e-9, "{dmax:e} in {k}");
}

/// `TELLEGEN_REINIT_SKIP=1`: once the re-solve with the device states free
/// has met a singular matrix (learnt before the run), later re-solves of the
/// same structure go straight to the fallback. The trace then has no failed
/// re-solve, and the run agrees with the default to Newton's tolerance.
#[test]
fn skipping_the_singular_re_solve_gives_the_same_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let failed_reinits = |lines: &[TraceLine]| {
        lines
            .iter()
            .filter(|l| matches!(l, TraceLine::Fail { why, .. } if why.starts_with("reinit: ")))
            .count()
    };
    let (_, plain) = traced("grid", d, &[]);
    assert!(failed_reinits(&plain) > 0);
    let (run, skipped) = traced("grid", d, &[("TELLEGEN_REINIT_SKIP", "1")]);
    assert_eq!(failed_reinits(&skipped), 0);
    let reinits = skipped
        .iter()
        .filter(|l| matches!(l, TraceLine::Reinit { .. }))
        .count();
    assert_eq!(reinits, 2, "the probe before the run is not traced");
    let (dmax, k) = largest_diff(&run, &here("grid", d).unwrap());
    assert!(dmax < SAME_SOLUTION, "{dmax:e} in {k}");
}

/// `TELLEGEN_KEEP_EPOCH`: `1` keeps factorizations across events although
/// `RmsOptions::keep_factorization` is off, `0` drops them although it is
/// on; either way Newton's residual decides convergence and the run agrees
/// with the option's own run to Newton's tolerance.
#[test]
fn keeping_factorizations_across_events_gives_the_same_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let on = RmsOptions::default();
    let off = RmsOptions {
        keep_factorization: false,
        ..on
    };
    for (options, value) in [(off, "1"), (on, "0")] {
        let run = in_child("grid", options, &[("TELLEGEN_KEEP_EPOCH", value)])
            .run
            .unwrap();
        let (dmax, k) = largest_diff(&run, &here("grid", options).unwrap());
        assert!(
            dmax < SAME_SOLUTION,
            "TELLEGEN_KEEP_EPOCH={value}: {dmax:e} in {k}"
        );
    }
}

/// `TELLEGEN_STALL=<ratio>` (the residual ratio that counts as a stalled
/// iteration) and `TELLEGEN_RELAXED` (keep any factorization until Newton
/// stalls, overriding `RmsOptions::keep_factorization` either way) change
/// when factorizations are refreshed, not the solution: a strict ratio
/// counts more stalls in `lu_stats`, and every run agrees with the run
/// without the switch to Newton's tolerance.
#[test]
fn the_stall_ratio_and_the_relaxed_refresh_give_the_same_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let on = RmsOptions::default();
    let off = RmsOptions {
        keep_factorization: false,
        ..on
    };
    let base = in_child("grid", on, &[]);
    let strict = in_child("grid", on, &[("TELLEGEN_STALL", "0.05")]);
    assert!(stat(&strict.lu_stats, ", stall ") > stat(&base.lu_stats, ", stall "));
    let (dmax, k) = largest_diff(&strict.run.unwrap(), &base.run.unwrap());
    assert!(dmax < SAME_SOLUTION, "TELLEGEN_STALL=0.05: {dmax:e} in {k}");
    for (options, value) in [(off, "1"), (on, "0")] {
        let run = in_child("grid", options, &[("TELLEGEN_RELAXED", value)])
            .run
            .unwrap();
        let (dmax, k) = largest_diff(&run, &here("grid", options).unwrap());
        assert!(
            dmax < SAME_SOLUTION,
            "TELLEGEN_RELAXED={value}: {dmax:e} in {k}"
        );
    }
}

/// `TELLEGEN_PIVOT_TOL=<x>` sets the fraction of a column's largest entry a
/// refactorization's pivot must keep: at 1e12 no pivot passes, every
/// refactorization falls back to a pivoted one (`pivot fails` in
/// `lu_stats`), and the run is bit for bit the one with
/// `RmsOptions::refactor_values` off.
#[test]
fn a_pivot_tolerance_no_pivot_meets_pivots_afresh() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let child = in_child("grid", d, &[("TELLEGEN_PIVOT_TOL", "1e12")]);
    assert!(
        stat(&child.lu_stats, "pivot fails ") > 0.0,
        "{}",
        child.lu_stats
    );
    let afresh = RmsOptions {
        refactor_values: false,
        ..d
    };
    assert_identical(
        &child.run.unwrap(),
        &here("grid", afresh).unwrap(),
        "TELLEGEN_PIVOT_TOL=1e12 against pivoting afresh",
    );
}

/// `TELLEGEN_REGIONS=<map>` splits the multi-machine case into two regions: the residual
/// runs by regions (`region_stats` counts its calls) and the network LU by
/// blocks around a separator (its times in `lu_stats`), and the run agrees
/// with the whole-system one to rounding. `TELLEGEN_NET_ONE=1` factorizes
/// the network as one sparse LU again while the residual stays split: no
/// time in the region LUs, and the run is the whole-system one bit for bit.
#[test]
fn regions_and_net_one_factorize_the_same_network() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let base = here("grid", d).unwrap();
    let (sim, _) = case("grid", d).unwrap();
    let buses = sim.buses();
    let map: BTreeMap<String, u16> = buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.to_string(), u16::from(2 * i >= buses.len())))
        .collect();
    let path = scratch("regions.json");
    std::fs::write(&path, serde_json::to_string(&map).unwrap()).unwrap();
    let regions = ("TELLEGEN_REGIONS", path.to_str().unwrap());
    let split = in_child("grid", d, &[regions]);
    assert!(stat(&split.region_stats, "residual by regions: ") > 0.0);
    assert!(
        stat(&split.lu_stats, "region LUs and A_rr^-1 B_r ") > 0.0,
        "{}",
        split.lu_stats
    );
    let (dmax, k) = largest_diff(&split.run.unwrap(), &base);
    assert!(dmax < 1e-9, "by regions: {dmax:e} in {k}");
    let one = in_child("grid", d, &[regions, ("TELLEGEN_NET_ONE", "1")]);
    let _ = std::fs::remove_file(&path);
    assert!(stat(&one.region_stats, "residual by regions: ") > 0.0);
    assert_eq!(stat(&one.lu_stats, "region LUs and A_rr^-1 B_r "), 0.0);
    assert_eq!(stat(&one.lu_stats, "Schur complement "), 0.0);
    assert_identical(&one.run.unwrap(), &base, "regions with TELLEGEN_NET_ONE=1");
}

/// `TELLEGEN_FREEZE=0` / `1` overrides `RmsOptions::freeze_limits` (which
/// matters on this case: the field voltage rides its ceiling): with `0` a run
/// with the option on is bit for bit the run with it off, with `1` the
/// reverse; any other value leaves the option in charge.
#[test]
fn tellegen_freeze_overrides_the_freeze_limits_option() {
    if shipped_wecc240().is_none() {
        return;
    }
    let held = RmsOptions::default();
    assert!(held.freeze_limits);
    let free = RmsOptions {
        freeze_limits: false,
        ..held
    };
    // The run with the option off in a child too: the option reaches the
    // models through a process-wide flag (see
    // `concurrent_runs_keep_their_own_freeze_limits`), so a run with it off
    // here would change the runs of the tests beside this one.
    let run_held = here("smib", held).unwrap();
    let run_free = in_child("smib", free, &[]).run.unwrap();
    let (dmax, _) = largest_diff(&run_held, &run_free);
    assert!(dmax > 1e-3, "the option does not matter here: {dmax:e}");
    let child = |options, value| {
        in_child("smib", options, &[("TELLEGEN_FREEZE", value)])
            .run
            .unwrap()
    };
    assert_identical(&child(held, "0"), &run_free, "TELLEGEN_FREEZE=0");
    assert_identical(&child(free, "1"), &run_held, "TELLEGEN_FREEZE=1");
    assert_identical(&child(free, "yes"), &run_free, "TELLEGEN_FREEZE=yes");
}

/// The magnitude a load part's law is evaluated at with the PQBRAK break at
/// `vb` rounded off over `vb ± d`, as `TELLEGEN_PQBRAK_SMOOTH` documents
/// it: `max(V, vb)` outside the band, inside it the parabola
/// `vb + (V − vb + d)² / (4 d)`.
fn smoothed_magnitude(v: f64, vb: f64, d: f64) -> f64 {
    if (v - vb).abs() < d {
        vb + (v - vb + d).powi(2) / (4.0 * d)
    } else {
        v.max(vb)
    }
}

/// `TELLEGEN_PQBRAK_SMOOTH=<d>` rounds off the PQBRAK break over `Vb ± d`.
/// A constant-power load draws `P0 (V / u)²`, so each sample gives the
/// magnitude `u` the engine evaluated: through a fault that holds the bus
/// inside the band it is the documented parabola (to 1e-9 in power), never
/// below `max(V, Vb)` nor more than `d / 4` above it, and the parabola meets
/// the plain break with the same value and slope at `Vb ± d`. A bus whose
/// power flow voltage lies within `d` above its break (here at 95 % of it)
/// is not smoothed: its run is the one without the switch bit for bit.
#[test]
fn pqbrak_smoothing_rounds_off_the_break() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let band = 0.1;
    let smooth = ("TELLEGEN_PQBRAK_SMOOTH", "0.1");
    // The documented formula: continuous in value and slope at the band's
    // edges, at most d/4 above the plain break, at the break.
    let vb = 0.7;
    let du = |v: f64| {
        (smoothed_magnitude(v + 1e-7, vb, band) - smoothed_magnitude(v - 1e-7, vb, band)) / 2e-7
    };
    for (edge, value, slope) in [(vb - band, vb, 0.0), (vb + band, vb + band, 1.0)] {
        for side in [-1e-12, 1e-12] {
            assert!((smoothed_magnitude(edge + side, vb, band) - value).abs() < 1e-11);
        }
        assert!((du(edge - 1e-6) - slope).abs() < 1e-4 && (du(edge + 1e-6) - slope).abs() < 1e-4);
    }
    assert!((smoothed_magnitude(vb, vb, band) - (vb + band / 4.0)).abs() < 1e-15);

    let run = in_child("pqbrak", d, &[smooth]).run.unwrap();
    let plain = here("pqbrak", d).unwrap();
    let (vm, pl) = (&run.channels["vm:102"], &run.channels["pl:102"]);
    let p0 = pl[0];
    assert!(
        vm[0] - vb >= band,
        "the bus starts above the band: {}",
        vm[0]
    );
    let mut inside = 0;
    for (k, (&v, &p)) in vm.iter().zip(pl).enumerate() {
        let u = smoothed_magnitude(v, vb, band);
        assert!(
            (p - p0 * (v / u).powi(2)).abs() < 1e-9,
            "t = {}: V {v}, P {p}",
            run.time[k]
        );
        let u_seen = v * (p0 / p).sqrt();
        let floor = v.max(vb);
        assert!(
            u_seen > floor - 1e-9 && u_seen < floor + band / 4.0 + 1e-9,
            "t = {}: V {v}, u {u_seen}",
            run.time[k]
        );
        if (v - vb).abs() < band {
            inside += 1;
        }
    }
    assert!(inside >= 10, "{inside} samples inside the band");
    let (dmax, _) = largest_diff(&run, &plain);
    assert!(dmax > 1e-3, "the smoothing changes nothing: {dmax:e}");

    let near = in_child("pqbrak-near", d, &[smooth]).run.unwrap();
    let near_plain = here("pqbrak-near", d).unwrap();
    assert_identical(&near, &near_plain, "a bus starting within d of its break");
    let (vm, pl) = (&near.channels["vm:102"], &near.channels["pl:102"]);
    let vb = 0.95 * vm[0];
    assert!(
        vm.iter().any(|&v| v < vb - band),
        "the fault takes the bus below the break"
    );
    for (&v, &p) in vm.iter().zip(pl) {
        assert!(
            (p - pl[0] * (v / v.max(vb)).powi(2)).abs() < 1e-9,
            "V {v}, P {p}"
        );
    }
}

/// The statistics after a run, in a fresh process: `lu_stats` counts the
/// run's steps, its Newton iterations within steps (the run's count less
/// the re-solves after its two events) and the re-solves' factorizations,
/// and the wall time of the residual (its timer) and, with the
/// `protection` feature, of the steps (`time_add`); it ends with
/// `region_stats`, which counts no call without regions. `set_clock` is a
/// no-op natively: after handing in a clock that reads zero, the times still
/// come from `Instant` and the run is unchanged.
#[test]
fn the_statistics_lines_count_the_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let d = RmsOptions::default();
    let child = in_child("grid", d, &[(CHILD_SET_CLOCK, "1")]);
    let run = child.run.unwrap();
    assert_identical(&run, &here("grid", d).unwrap(), "after set_clock");
    let s = &child.lu_stats;
    assert_eq!(stat(s, "); steps ") as usize, run.steps, "{s}");
    let iterations = stat(s, ", newton iterations ") as usize;
    assert!(
        iterations >= run.steps && iterations < run.newton_iterations,
        "{s}"
    );
    assert!(stat(s, "factorizations: reinit ") >= 2.0, "{s}");
    assert!(stat(s, "in steps: residual ") > 0.0, "{s}");
    #[cfg(feature = "protection")]
    assert!(stat(s, "s; steps ") > 0.0, "{s}");
    assert!(
        child
            .region_stats
            .starts_with("residual by regions: 0 calls"),
        "{}",
        child.region_stats
    );
    assert!(s.trim_end().ends_with(child.region_stats.trim_end()), "{s}");
}

/// Runs, in a child process of its own, a run with `freeze_limits` on while
/// runs with it off start beside it on another thread, and fails when the
/// run differs from the same run alone.
#[test]
#[ignore = "run by concurrent_runs_keep_their_own_freeze_limits in a child process"]
fn child_freeze_race() {
    if std::env::var_os(CHILD_OUT).is_none() {
        return;
    }
    let held = RmsOptions::default();
    let free = RmsOptions {
        freeze_limits: false,
        ..held
    };
    let (sim_held, request) = case("smib", held).unwrap();
    let (sim_free, _) = case("smib", free).unwrap();
    let alone = Run::from(sim_held.run(&request).unwrap());
    let stop = std::sync::atomic::AtomicBool::new(false);
    let started = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = sim_free.run(&request);
                started.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
        while !started.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::yield_now();
        }
        let beside: Vec<Run> = (0..20)
            .map(|_| Run::from(sim_held.run(&request).unwrap()))
            .collect();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        for run in &beside {
            assert_identical(
                run,
                &alone,
                "a run with freeze_limits on beside runs with it off",
            );
        }
    });
}

/// `RmsOptions::freeze_limits` affects only the simulation it is given to
/// (as `RmsOptions` states for every option): a run with it on is the same
/// bit for bit while runs with it off start beside it in the same process.
/// The option reaches the models through one process-wide flag that each run
/// sets as it starts (`models::set_freeze_limits`), so the run beside takes
/// the other setting partway. Run in a child process, so the runs with the
/// option off cannot reach the tests beside this one.
#[test]
#[ignore = "bug: freeze_limits is a process-wide flag set at the start of each run, so concurrent runs with different settings change each other's results"]
fn concurrent_runs_keep_their_own_freeze_limits() {
    if shipped_wecc240().is_none() {
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "child_freeze_race",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_OUT, "-")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
