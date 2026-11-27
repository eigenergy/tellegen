//! The model switches of the RMS engine: process-wide model options a native
//! caller sets before it builds a system (`tellegen::rms::set_*`). For each:
//!
//! - off (its default) it has no effect: a run is bit for bit the run
//!   without it;
//! - on, a flat start stays flat (`f(x0)` vanishes and nothing moves);
//! - on, a disturbance moves the system the way the switch documents.
//!
//! Besides, the run's relief of a collapse (a network the solver cannot
//! re-solve after an event) is checked end to end.
//!
//! The switches are globals of the process, so every test here holds one
//! lock and turns every switch off when it starts and when it ends, also when
//! it fails. The networks are small and written inline.

#![cfg(feature = "rms-experimental")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tellegen::rms::{
    self, InputSignal, LoadModel, LowVoltageLoad, RmsEvent, RmsOptions, RmsRequest, RmsResult,
    RmsSimulation,
};

// ---------------------------------------------------------------------------
// The switches: one test at a time, every switch off around it.

static SWITCHES: Mutex<()> = Mutex::new(());

/// Holds the lock on the switches; every switch is off while it lives
/// (turned off on creation and again on drop, also when a test panics).
struct Switches {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for Switches {
    fn drop(&mut self) {
        all_off();
    }
}

/// Takes the lock on the switches (also after a test that failed holding it).
fn switches() -> Switches {
    let lock = SWITCHES.lock().unwrap_or_else(PoisonError::into_inner);
    all_off();
    Switches { _lock: lock }
}

/// Every switch to its off value.
fn all_off() {
    rms::set_load_recovery(0.0, 0.0);
    rms::set_load_saturation(0.0, 0.0);
    rms::set_load_rise(0.0);
    rms::set_uel(HashMap::new());
    rms::set_vcomp(HashMap::new());
    rms::set_plant_q_exponent(HashMap::new());
    rms::set_gast_rate(0.0);
    rms::set_hvdc_emulation(Vec::new());
}

// ---------------------------------------------------------------------------
// Inline networks.

const RAW_TAIL: &str = " 0 /End of Branch data, Begin Transformer data
 0 /End of Transformer data, Begin Area interchange data
 0 /End of Area interchange data, Begin Two-terminal dc line data
 0 /End of Two-terminal dc line data, Begin VSC dc line data
 0 /End of VSC dc line data, Begin Impedance correction table data
 0 /End of Impedance correction table data, Begin Multi-terminal dc line data
 0 /End of Multi-terminal dc line data, Begin Multi-section line data
 0 /End of Multi-section line data, Begin Zone data
 0 /End of Zone data, Begin Inter-area transfer data
 0 /End of Inter-area transfer data, Begin Owner data
 0 /End of Owner data, Begin FACTS device data
 0 /End of FACTS device data, Begin Switched shunt data
 0 /End of Switched shunt data, Begin GNE device data
 0 /End of GNE device data
Q
";

/// A bus: number, type (1 load, 2 generator, 3 slack), voltage.
type Bus = (usize, u8, f64);
/// A generator: bus, PG (MW), VS (pu), MBASE (MVA), ZSORCE reactance (pu).
type Gen = (usize, f64, f64, f64, f64);
/// A constant-power load: bus, PL (MW), QL (Mvar).
type Load = (usize, f64, f64);
/// A line: from, to, circuit, reactance (pu).
type Line = (usize, usize, &'static str, f64);

/// A PSS/E RAW case on 100 MVA, 60 Hz.
fn raw_case(buses: &[Bus], gens: &[Gen], loads: &[Load], lines: &[Line]) -> String {
    let mut r = String::from("0,   100.00,  32, 0, 1, 60.00     / model switch test case\n\n\n");
    for &(b, kind, v) in buses {
        r.push_str(&format!(
            "{b},'B{b}', 230.0000,{kind},   1,   1,   1,{v:.5},   0.0000\n"
        ));
    }
    r.push_str(" 0 /End of Bus data, Begin Load data\n");
    for &(b, p, q) in loads {
        r.push_str(&format!(
            "{b},'1 ',1,   1,   1, {p:.3}, {q:.3}, 0.000, 0.000, 0.000, 0.000,   1,1\n"
        ));
    }
    r.push_str(" 0 /End of Load data, Begin Fixed shunt data\n");
    r.push_str(" 0 /End of Fixed shunt data, Begin Generator data\n");
    for &(b, pg, vs, mbase, zx) in gens {
        r.push_str(&format!(
            "{b},'1 ', {pg:.3}, 0.000, 9999.000, -9999.000,{vs:.5}, 0, {mbase:.3}, 0.00000E+0, {zx:.5E}, \
             0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 9999.000, -9999.000,   1,1.0000\n"
        ));
    }
    r.push_str(" 0 /End of Generator data, Begin Branch data\n");
    for &(f, t, ckt, x) in lines {
        r.push_str(&format!(
            "{f}, {t},'{ckt} ', 0.00000E+0, {x:.5E}, 0.00000, 9999.00, 9999.00, 9999.00, \
             0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000\n"
        ));
    }
    r.push_str(RAW_TAIL);
    r
}

/// The network of a RAW text.
fn network(raw: &str) -> powerio::BalancedNetwork {
    let src = powerio::Source::from_memory("case.raw", raw.as_bytes().to_vec()).unwrap();
    let module = powerio::parse(src).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    net.clone()
}

/// A round-rotor unit with a static exciter at `bus` (machine and exciter
/// data of a typical thermal unit).
fn genrou_sexs(bus: usize) -> String {
    format!(
        "{bus} 'GENROU' 1 6.0 0.05 1.0 0.05 4.26 0.0 1.4 1.35 0.3 0.6 0.2 0.1 0.03 0.4 /\n\
         {bus} 'SEXS' 1 0.1 10.0 100.0 0.1 0.0 3.0 /\n"
    )
}

/// TGOV1: R 0.05, T1 0.5 s, VMAX 1, VMIN 0, T2 3 s, T3 10 s.
fn tgov1(bus: usize) -> String {
    format!("{bus} 'TGOV1' 1 0.05 0.5 1.0 0.0 3.0 10.0 0.0 /\n")
}

/// GAST: R 0.08, T1 0.3 s, T2 0.3 s, T3 3 s, AT 1, KT 2, VMAX 1, VMIN 0.
fn gast(bus: usize) -> String {
    format!("{bus} 'GAST' 1 0.08 0.3 0.3 3.0 1.0 2.0 1.0 0.0 0.0 /\n")
}

/// A renewable plant (REGCA1, REECB1, REPCA1) in reactive power control:
/// the plant controller's PI (Kp 0.5, Ki 5, no deadband) sets the reactive
/// current command.
fn plant(bus: usize) -> String {
    format!(
        "{bus} 'REGCA1' 1 1 0.02 10.0 0.9 0.4 1.22 1.2 0.9 0.5 -1.3 0.02 0.0 100.0 -100.0 0.7 /\n\
         {bus} 'REECB1' 1 0 0 1 0 0 -99.0 99.0 0.02 0.0 0.0 0.0 1.1 -1.1 0.0 0.05 0.44 -0.44 \
         1.05 0.9 0.0 0.01 10.0 60.0 0.02 99.0 -99.0 1.0 0.0 1.11 0.02 /\n\
         {bus} 'REPCA1' 1 0 0 0 '0 ' 0 0 1 0.02 0.5 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 \
         0.0 0.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0 /\n"
    )
}

/// The infinite bus 101 (a classical machine without inertia).
const INFINITE_101: &str = "101 'GENCLS' 1 0.0 0.0 /\n";

/// A feeder: the infinite bus 101 feeds the load bus 103 (150 MW, 30 Mvar)
/// over two lines; the unit at 102 (80 MW on 100 MVA, at 1.02 pu) feeds it
/// over a third. `governor` is the unit's governor record.
fn feeder(governor: &str, model: LoadModel) -> RmsSimulation {
    let raw = raw_case(
        &[(101, 3, 1.0), (102, 2, 1.02), (103, 1, 1.0)],
        &[(101, 0.0, 1.0, 100.0, 1e-5), (102, 80.0, 1.02, 100.0, 0.2)],
        &[(103, 150.0, 30.0)],
        &[
            (101, 103, "1", 0.1),
            (101, 103, "2", 0.1),
            (102, 103, "1", 0.05),
        ],
    );
    let dyr = format!("{INFINITE_101}{}{governor}", genrou_sexs(102));
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    RmsSimulation::new(&network(&raw), &dyr, model).unwrap()
}

/// A radial load: the infinite bus 101 feeds 100 MW and 20 Mvar of
/// constant power at 103 over two lines of 0.15 pu. Tripping one lowers the
/// load bus voltage at once, from about 0.96 to 0.87 pu at full demand,
/// with nothing to regulate it back.
fn radial() -> RmsSimulation {
    let raw = raw_case(
        &[(101, 3, 1.0), (103, 1, 1.0)],
        &[(101, 0.0, 1.0, 100.0, 1e-5)],
        &[(103, 100.0, 20.0)],
        &[(101, 103, "1", 0.15), (101, 103, "2", 0.15)],
    );
    let dyr = powerio::parse_dyr(INFINITE_101).unwrap();
    RmsSimulation::new(&network(&raw), &dyr, LoadModel::ConstantPower).unwrap()
}

/// The feeder with a renewable plant at 104 (50 MW on 100 MVA, at 1.01 pu,
/// so it starts delivering reactive power) instead of the unit at 102.
fn plant_feeder() -> RmsSimulation {
    let raw = raw_case(
        &[(101, 3, 1.0), (103, 1, 1.0), (104, 2, 1.01)],
        &[(101, 0.0, 1.0, 100.0, 1e-5), (104, 50.0, 1.01, 100.0, 0.2)],
        &[(103, 150.0, 30.0)],
        &[
            (101, 103, "1", 0.1),
            (101, 103, "2", 0.1),
            (104, 103, "1", 0.05),
        ],
    );
    let dyr = format!("{INFINITE_101}{}", plant(104));
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    RmsSimulation::new(&network(&raw), &dyr, LoadModel::ConstantPower).unwrap()
}

/// Two areas joined by one weak tie: the classical machines at 1 and 2
/// (500 MVA, H = 4 s, no damping) feed the loads at 3 and 4 (250 MW each)
/// over short lines; the tie 3-4 has 0.3 pu. Its one oscillatory mode is
/// the inter-area mode of the two machines.
fn two_areas() -> RmsSimulation {
    let raw = raw_case(
        &[(1, 3, 1.0), (2, 2, 1.0), (3, 1, 1.0), (4, 1, 1.0)],
        &[(1, 0.0, 1.0, 500.0, 0.3), (2, 200.0, 1.0, 500.0, 0.3)],
        &[(3, 250.0, 0.0), (4, 250.0, 0.0)],
        &[(1, 3, "1", 0.02), (2, 4, "1", 0.02), (3, 4, "1", 0.3)],
    );
    let dyr = "1 'GENCLS' 1 4.0 0.0 /\n2 'GENCLS' 1 4.0 0.0 /\n";
    let dyr = powerio::parse_dyr(dyr).unwrap();
    RmsSimulation::new(&network(&raw), &dyr, LoadModel::ConstantImpedance).unwrap()
}

// ---------------------------------------------------------------------------
// Runs and comparisons.

/// A run to `t_end` with steps and samples of 5 ms.
fn run(sim: &RmsSimulation, t_end: f64, events: Vec<RmsEvent>) -> RmsResult {
    sim.run(&RmsRequest {
        t_end,
        dt: 0.005,
        output_step: 0.005,
        events,
        ..RmsRequest::default()
    })
    .unwrap()
}

/// Every channel of `a` equals `b`'s bit for bit.
fn assert_bit_identical(a: &RmsResult, b: &RmsResult, what: &str) {
    assert_eq!(a.time, b.time, "{what}: time");
    assert_eq!(
        a.channels.keys().collect::<Vec<_>>(),
        b.channels.keys().collect::<Vec<_>>(),
        "{what}: channels"
    );
    for (key, s) in &a.channels {
        let t = &b.channels[key];
        let same = s.iter().zip(t).all(|(x, y)| x.to_bits() == y.to_bits());
        assert!(same, "{what}: {key} differs");
    }
}

/// The largest difference between `a` and `b` over every channel.
fn largest_difference(a: &RmsResult, b: &RmsResult) -> (f64, String) {
    let mut worst = (0.0_f64, String::new());
    for (key, s) in &a.channels {
        let d = s
            .iter()
            .zip(&b.channels[key])
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        if !(d <= worst.0) {
            worst = (d, key.clone());
        }
    }
    worst
}

/// The largest move of any channel from its start value.
fn largest_move(r: &RmsResult) -> (f64, String) {
    let mut worst = (0.0_f64, String::new());
    for (key, s) in &r.channels {
        let d = s.iter().map(|x| (x - s[0]).abs()).fold(0.0, f64::max);
        if !(d <= worst.0) {
            worst = (d, key.clone());
        }
    }
    worst
}

/// The simulation starts at rest and a run without events stays there.
fn assert_flat(sim: &RmsSimulation, what: &str) {
    assert!(
        sim.initial_residual() < 1e-8,
        "{what}: f(x0) = {:e}",
        sim.initial_residual()
    );
    let (d, key) = largest_move(&run(sim, 1.0, Vec::new()));
    assert!(
        d < 1e-8,
        "{what}: {key} moves by {d:e} without a disturbance"
    );
}

/// Line 101-103 circuit 1 opens at `time`.
fn line_trip(time: f64) -> Vec<RmsEvent> {
    vec![RmsEvent::BranchTrip {
        time,
        from: 101,
        to: 103,
        circuit: "1".into(),
    }]
}

/// The reference `signal` of the unit at `bus` stepped by `step` from its
/// start value at `time`.
fn set_reference(
    sim: &RmsSimulation,
    time: f64,
    bus: usize,
    signal: InputSignal,
    step: f64,
) -> Vec<RmsEvent> {
    let g = sim.generators().into_iter().find(|g| g.bus == bus).unwrap();
    let start = match signal {
        InputSignal::VRef => g.v_ref,
        InputSignal::PRef => g.p_ref,
        InputSignal::Vs => 0.0,
    };
    vec![RmsEvent::SetReference {
        time,
        bus,
        machine_id: "1".into(),
        signal,
        value: start + step,
    }]
}

/// The sample index at time `t`.
fn at(r: &RmsResult, t: f64) -> usize {
    r.time.partition_point(|&s| s < t - 1e-9)
}

// ---------------------------------------------------------------------------
// Load recovery.

/// Load recovery off (`alpha = 0`, whatever the time constant) leaves a run
/// through a line trip bit for bit as it was.
#[test]
fn load_recovery_off_changes_nothing() {
    let _s = switches();
    let base = run(&radial(), 2.0, line_trip(0.1));
    rms::set_load_recovery(0.0, 5.0);
    let off = run(&radial(), 2.0, line_trip(0.1));
    assert_bit_identical(&base, &off, "alpha = 0");
}

/// With load recovery on, the start is a steady state and stays one: the
/// reference voltage starts at the power flow voltage.
#[test]
fn load_recovery_starts_flat() {
    let _s = switches();
    rms::set_load_recovery(2.0, 1.0);
    assert_flat(&radial(), "load recovery");
}

/// After a line trip lowers the load bus voltage, a constant-power load with
/// recovery (`alpha = 2`, `t = 1 s`) first draws less, `P0 (V / Vf)^2` with
/// the reference `Vf` still near the old voltage, and then returns to `P0`
/// as `Vf` follows the voltage with the time constant `t`: the reference
/// recovered from the samples obeys `Vf += dt / t (V - Vf)` after every step,
/// the demand rises monotonically, a third or so of the dip is left after
/// one time constant, and nearly none after five. Without recovery the load
/// draws `P0` throughout.
#[test]
fn load_recovery_drops_with_the_voltage_and_recovers_with_its_time_constant() {
    let _s = switches();
    let (alpha, tc, dt, t_trip) = (2.0, 1.0, 0.005, 0.1);
    let off = run(&radial(), 6.0, line_trip(t_trip));
    let p0 = off.channels["pl:103"][0];
    let held = off.channels["pl:103"]
        .iter()
        .map(|p| (p - p0).abs())
        .fold(0.0, f64::max);
    assert!(held < 1e-9, "without recovery the load moves by {held:e}");

    rms::set_load_recovery(alpha, tc);
    let on = run(&radial(), 6.0, line_trip(t_trip));
    let (pl, vm) = (&on.channels["pl:103"], &on.channels["vm:103"]);
    assert!((pl[0] - p0).abs() < 1e-12);
    let k0 = at(&on, t_trip);
    // Before the trip nothing moves.
    assert!(pl[..=k0].iter().all(|p| (p - p0).abs() < 1e-9));
    // The reference voltage the samples imply, and its law step by step.
    let vf: Vec<f64> = pl
        .iter()
        .zip(vm)
        .map(|(p, v)| v * (p0 / p).powf(1.0 / alpha))
        .collect();
    let a = dt / tc;
    for k in k0..pl.len() - 1 {
        let expected = vf[k] + a * (vm[k + 1] - vf[k]);
        assert!(
            (vf[k + 1] - expected).abs() < 1e-9,
            "t = {}: Vf {} against {expected}",
            on.time[k + 1],
            vf[k + 1]
        );
    }
    // The dip right after the trip, then a monotonic recovery.
    let dip = p0 - pl[k0 + 1];
    assert!(dip > 0.02 * p0, "the demand dips by {dip}");
    assert!(
        pl[k0 + 1..].windows(2).all(|w| w[1] >= w[0] - 1e-12),
        "the demand recovers monotonically"
    );
    let left = |t: f64| (p0 - pl[at(&on, t)]) / dip;
    eprintln!(
        "dip {dip:.4} pu, left after 1 t: {:.3}, after 5 t: {:.4}",
        left(t_trip + tc),
        left(t_trip + 5.0 * tc)
    );
    assert!(
        (0.2..0.5).contains(&left(t_trip + tc)),
        "{}",
        left(t_trip + tc)
    );
    assert!(
        left(t_trip + 5.0 * tc) < 0.02,
        "{}",
        left(t_trip + 5.0 * tc)
    );
}

// ---------------------------------------------------------------------------
// Load saturation and the one-sided rise of load reactive power.

/// Load saturation off (`c = 0`) leaves a run through a voltage rise bit for
/// bit as it was.
#[test]
fn load_saturation_off_changes_nothing() {
    let _s = switches();
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::VRef, 0.05);
    let base = run(&sim, 1.0, events.clone());
    rms::set_load_saturation(0.0, 0.9);
    let off = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 1.0, events);
    assert_bit_identical(&base, &off, "c = 0");
}

/// With saturation on and its knee below the start voltage, the start is
/// still a steady state: the knee never lies below a bus's own power flow
/// voltage.
#[test]
fn load_saturation_starts_flat() {
    let _s = switches();
    rms::set_load_saturation(0.5, 0.9);
    assert_flat(
        &feeder(&tgov1(102), LoadModel::ConstantPower),
        "load saturation",
    );
}

/// A voltage rise (the unit's voltage reference up 10 %) makes a saturating
/// load draw extra reactive power `c P0 ((V - Vk) / 0.1)^2` above the knee
/// `Vk` (here 0.5 % above the start voltage), sample by sample, and nothing
/// below it.
#[test]
fn load_saturation_draws_reactive_power_above_the_knee() {
    let _s = switches();
    let c = 2.0;
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::VRef, 0.1);
    let v0 = run(&sim, 0.005, Vec::new()).channels["vm:103"][0];
    let knee = v0 + 0.005;
    rms::set_load_saturation(c, knee);
    let r = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 2.0, events);
    check_extra_reactive_power(&r, c, knee, "saturation");
}

/// `ql:103` is `q0 + c p0 ((V - Vk) / 0.1)^2` above the knee `Vk` (at least
/// the start voltage) and `q0` below it, and the extra reaches 5 % of `q0`
/// at least.
fn check_extra_reactive_power(r: &RmsResult, c: f64, knee: f64, what: &str) {
    let (pl, ql, vm) = (
        &r.channels["pl:103"],
        &r.channels["ql:103"],
        &r.channels["vm:103"],
    );
    let (p0, q0) = (pl[0], ql[0]);
    let knee = knee.max(vm[0]);
    let mut most = 0.0_f64;
    let mut below = 0usize;
    for k in 0..ql.len() {
        let x = ((vm[k] - knee) / 0.1).max(0.0);
        if x == 0.0 {
            below += 1;
        }
        let expected = q0 + c * p0 * x * x;
        assert!(
            (ql[k] - expected).abs() < 1e-9,
            "{what}, t = {}: Q {} against {expected} at V {}",
            r.time[k],
            ql[k],
            vm[k]
        );
        most = most.max(ql[k] - q0);
    }
    eprintln!("{what}: extra Q up to {most:.4} pu, {below} samples below the knee");
    assert!(
        most > 0.05 * q0,
        "{what}: the extra reactive power peaks at {most}"
    );
}

/// The rise of load reactive power off (`c = 0`) leaves a run bit for bit as
/// it was; on, with its zero at the start voltage, the start is flat.
#[test]
fn load_rise_off_changes_nothing_and_on_starts_flat() {
    let _s = switches();
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::VRef, 0.05);
    let base = run(&sim, 1.0, events.clone());
    rms::set_load_rise(0.0);
    let off = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 1.0, events);
    assert_bit_identical(&base, &off, "c = 0");
    rms::set_load_rise(0.5);
    assert_flat(&feeder(&tgov1(102), LoadModel::ConstantPower), "load rise");
}

/// The rise is one-sided: above the start voltage (the unit's voltage
/// reference up 10 %) a load draws the extra `c P0 ((V - V0) / 0.1)^2`; in a
/// dip (a line trip on the radial load) it draws nothing extra, and the run
/// is bit for bit the run without the rise.
#[test]
fn load_rise_acts_above_the_start_voltage_only() {
    let _s = switches();
    let c = 2.0;
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let raise = set_reference(&sim, 0.1, 102, InputSignal::VRef, 0.1);
    let dip_off = run(&radial(), 1.0, line_trip(0.1));
    rms::set_load_rise(c);
    let r = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 2.0, raise);
    check_extra_reactive_power(&r, c, 0.0, "rise");
    let dip_on = run(&radial(), 1.0, line_trip(0.1));
    let v = &dip_on.channels["vm:103"];
    assert!(v[at(&dip_on, 0.1) + 1..].iter().all(|&x| x < v[0]));
    assert_bit_identical(&dip_off, &dip_on, "a dip with the rise on");
}

// ---------------------------------------------------------------------------
// Under-excitation limiter.

/// A limiter at a bus without a unit, or one whose limit is never reached,
/// leaves a run bit for bit as it was.
#[test]
fn uel_off_or_not_reached_changes_nothing() {
    let _s = switches();
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::VRef, -0.05);
    let base = run(&sim, 1.0, events.clone());
    rms::set_uel(HashMap::from([(103, [0.0, 5.0])]));
    let elsewhere = run(
        &feeder(&tgov1(102), LoadModel::ConstantPower),
        1.0,
        events.clone(),
    );
    assert_bit_identical(&base, &elsewhere, "a limiter at a bus without a unit");
    rms::set_uel(HashMap::from([(102, [5.0, 5.0])]));
    let unreached = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 1.0, events);
    assert_bit_identical(&base, &unreached, "a limiter never reached");
}

/// With a limiter on a unit that starts within its limit, the start is a
/// steady state.
#[test]
fn uel_starts_flat() {
    let _s = switches();
    rms::set_uel(HashMap::from([(102, [0.05, 5.0])]));
    assert_flat(&feeder(&tgov1(102), LoadModel::ConstantPower), "UEL");
}

/// A unit that already absorbs reactive power beyond its limit at the power
/// flow (here at 0.97 pu against the infinite bus, about -0.26 pu with a
/// limit of -0.05 pu) also starts at rest: the limiter's output at the start
/// is in the voltage reference the initialization solves for.
#[test]
fn uel_reached_at_the_start_starts_flat() {
    let _s = switches();
    let raw = raw_case(
        &[(101, 3, 1.0), (102, 2, 0.97), (103, 1, 1.0)],
        &[(101, 0.0, 1.0, 100.0, 1e-5), (102, 80.0, 0.97, 100.0, 0.2)],
        &[(103, 50.0, 0.0)],
        &[
            (101, 103, "1", 0.1),
            (101, 103, "2", 0.1),
            (102, 103, "1", 0.05),
        ],
    );
    let dyr = format!("{INFINITE_101}{}{}", genrou_sexs(102), tgov1(102));
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    let plain = RmsSimulation::new(&network(&raw), &dyr, LoadModel::ConstantPower).unwrap();
    let q0 = run(&plain, 0.005, Vec::new()).channels["qe:102"][0];
    assert!(q0 < -0.2, "the unit starts at Q = {q0}");
    rms::set_uel(HashMap::from([(102, [0.05, 5.0])]));
    let sim = RmsSimulation::new(&network(&raw), &dyr, LoadModel::ConstantPower)
        .expect("a unit beyond its limit at the start is built");
    assert_flat(&sim, "UEL reached at the start");
    // With a load compensator, instantaneous or through its lag, as well.
    for t in [0.0, 1.0] {
        rms::set_vcomp(HashMap::from([(102, [0.01, 0.05, t])]));
        let sim = RmsSimulation::new(&network(&raw), &dyr, LoadModel::ConstantPower)
            .expect("a unit beyond its limit at the start is built");
        assert_flat(
            &sim,
            &format!("UEL reached at the start, compensator T = {t}"),
        );
    }
}

/// The voltage reference lowered by 10 % drives the unit to absorb reactive
/// power beyond the limit `q_lim Vt^2`; the limiter raises the field and
/// holds the absorption smaller than without it.
#[test]
fn uel_limits_the_absorption_of_reactive_power() {
    let _s = switches();
    let q_lim = 0.05;
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::VRef, -0.1);
    let off = run(&sim, 5.0, events.clone());
    rms::set_uel(HashMap::from([(102, [q_lim, 5.0])]));
    let on = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 5.0, events);
    let end = |r: &RmsResult, key: &str| *r.channels[key].last().unwrap();
    let (q_off, q_on) = (end(&off, "qe:102"), end(&on, "qe:102"));
    let vt = end(&on, "vm:102");
    eprintln!(
        "Q without the limiter {q_off:.4}, with {q_on:.4}, limit {:.4}",
        -q_lim * vt * vt
    );
    assert!(
        q_off < -q_lim * vt * vt,
        "without the limiter the unit absorbs {q_off}"
    );
    assert!(
        q_on > q_off + 0.5 * (-q_lim * vt * vt - q_off),
        "{q_on} against {q_off}"
    );
    assert!(end(&on, "efd:102") > end(&off, "efd:102"));
}

// ---------------------------------------------------------------------------
// AVR load compensator.

/// A compensator at a bus without a unit leaves a run bit for bit as it was.
#[test]
fn vcomp_elsewhere_changes_nothing() {
    let _s = switches();
    let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
    let events = load_step();
    let base = run(&sim, 1.0, events.clone());
    rms::set_vcomp(HashMap::from([(103, [0.0, 0.05, 1.0])]));
    let elsewhere = run(&feeder(&tgov1(102), LoadModel::ConstantPower), 1.0, events);
    assert_bit_identical(&base, &elsewhere, "a compensator at a bus without a unit");
}

/// The load at 103 up by half at 0.1 s.
fn load_step() -> Vec<RmsEvent> {
    vec![RmsEvent::LoadChange {
        time: 0.1,
        bus: 103,
        scale: 1.5,
    }]
}

/// With a compensator, instantaneous (`t = 0`) or through a lag (`t > 0`,
/// one more state, `vcomp_lag`), the start is a steady state: the voltage
/// reference is set for the compensated voltage and the lag starts at the
/// reactive current.
#[test]
fn vcomp_starts_flat_with_and_without_a_time_constant() {
    let _s = switches();
    for t in [0.0, 1.0] {
        rms::set_vcomp(HashMap::from([(102, [0.01, 0.05, t])]));
        let sim = feeder(&tgov1(102), LoadModel::ConstantPower);
        let states = &sim
            .generators()
            .into_iter()
            .find(|g| g.bus == 102)
            .unwrap()
            .states;
        assert_eq!(
            states.iter().any(|s| s == "vcomp_lag"),
            t > 0.0,
            "{states:?}"
        );
        assert_flat(&sim, &format!("compensator with t = {t}"));
    }
}

/// A positive `xc` gives reactive droop: after a load step the unit raises
/// its reactive power and its terminal voltage settles lower than without
/// the compensator. Through a lag of 1 s the same droop sets in later: the
/// end is close to the instantaneous one, and shortly after the step the
/// voltage is still close to the uncompensated one.
#[test]
fn vcomp_gives_reactive_droop() {
    let _s = switches();
    let xc = 0.05;
    let off = run(
        &feeder(&tgov1(102), LoadModel::ConstantPower),
        8.0,
        load_step(),
    );
    rms::set_vcomp(HashMap::from([(102, [0.0, xc, 0.0])]));
    let now = run(
        &feeder(&tgov1(102), LoadModel::ConstantPower),
        8.0,
        load_step(),
    );
    rms::set_vcomp(HashMap::from([(102, [0.0, xc, 1.0])]));
    let lag = run(
        &feeder(&tgov1(102), LoadModel::ConstantPower),
        8.0,
        load_step(),
    );
    let end = |r: &RmsResult, key: &str| *r.channels[key].last().unwrap();
    let q_rise = end(&off, "qe:102") - off.channels["qe:102"][0];
    assert!(q_rise > 0.05, "the unit raises Q by {q_rise}");
    let droop = end(&off, "vm:102") - end(&now, "vm:102");
    eprintln!(
        "droop {droop:.5}, lag end {:.5}",
        end(&off, "vm:102") - end(&lag, "vm:102")
    );
    assert!(droop > 0.2 * xc * q_rise, "droop {droop}");
    let lag_droop = end(&off, "vm:102") - end(&lag, "vm:102");
    assert!(
        (lag_droop - droop).abs() < 0.1 * droop,
        "{lag_droop} against {droop}"
    );
    let k = at(&off, 0.2);
    let v = |r: &RmsResult| r.channels["vm:102"][k];
    assert!(
        (v(&lag) - v(&off)).abs() < 0.5 * (v(&now) - v(&off)).abs(),
        "0.1 s after the step: off {}, instantaneous {}, lag {}",
        v(&off),
        v(&now),
        v(&lag)
    );
}

// ---------------------------------------------------------------------------
// GAST valve rate.

/// GAST's valve rate limit off (rate 0) leaves a run bit for bit as it was;
/// on, the start is flat.
#[test]
fn gast_rate_off_changes_nothing_and_on_starts_flat() {
    let _s = switches();
    let sim = feeder(&gast(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::PRef, 0.2);
    let base = run(&sim, 1.0, events.clone());
    rms::set_gast_rate(0.0);
    let off = run(&feeder(&gast(102), LoadModel::ConstantPower), 1.0, events);
    assert_bit_identical(&base, &off, "rate 0");
    rms::set_gast_rate(0.05);
    assert_flat(&feeder(&gast(102), LoadModel::ConstantPower), "GAST rate");
}

/// After a step of the power reference by 0.2 pu the fuel valve opens at
/// most at the set rate (0.05 pu/s), so the mechanical torque, a lag of the
/// valve, rises no faster; without the limit it rises several times faster.
#[test]
fn gast_rate_limits_the_valve() {
    let _s = switches();
    let rate = 0.05;
    let sim = feeder(&gast(102), LoadModel::ConstantPower);
    let events = set_reference(&sim, 0.1, 102, InputSignal::PRef, 0.2);
    let off = run(&sim, 3.0, events.clone());
    rms::set_gast_rate(rate);
    let on = run(&feeder(&gast(102), LoadModel::ConstantPower), 3.0, events);
    let fastest = |r: &RmsResult| {
        let tm = &r.channels["tm:102"];
        tm.windows(2)
            .map(|w| (w[1] - w[0]) / 0.005)
            .fold(0.0, f64::max)
    };
    eprintln!(
        "fastest rise of tm: off {:.4}, on {:.4}",
        fastest(&off),
        fastest(&on)
    );
    assert!(fastest(&on) <= rate * 1.01, "{}", fastest(&on));
    assert!(fastest(&off) > 3.0 * rate, "{}", fastest(&off));
    let rise = |r: &RmsResult| r.channels["tm:102"].last().unwrap() - r.channels["tm:102"][0];
    assert!(
        rise(&on) < rise(&off) - 0.05,
        "{} against {}",
        rise(&on),
        rise(&off)
    );
    assert!(rise(&on) > 0.5 * rate * 2.9, "{}", rise(&on));
}

// ---------------------------------------------------------------------------
// Voltage exponent of the plants' reactive power set point.

/// An exponent for a bus without a plant leaves a run bit for bit as it was;
/// for the plant, the start is flat (its set point is unchanged at the start
/// voltage).
#[test]
fn plant_q_exponent_off_changes_nothing_and_on_starts_flat() {
    let _s = switches();
    let base = run(&plant_feeder(), 1.0, load_step());
    rms::set_plant_q_exponent(HashMap::from([(103, 2.0)]));
    let elsewhere = run(&plant_feeder(), 1.0, load_step());
    assert_bit_identical(&base, &elsewhere, "an exponent for a bus without a plant");
    rms::set_plant_q_exponent(HashMap::from([(104, 2.0)]));
    assert_flat(&plant_feeder(), "plant Q exponent");
}

/// A sustained voltage dip (the load at 103 up by half) lowers the plant's
/// reactive set point with `(V / V0)^n`: with `n = 2` the plant settles at
/// `Q0 (V / V0)^2`, below the `Q0` it returns to without the exponent.
#[test]
fn plant_q_exponent_lowers_the_set_point_with_the_voltage() {
    let _s = switches();
    let off = run(&plant_feeder(), 5.0, load_step());
    rms::set_plant_q_exponent(HashMap::from([(104, 2.0)]));
    let on = run(&plant_feeder(), 5.0, load_step());
    let q0 = off.channels["qe:104"][0];
    let end = |r: &RmsResult, key: &str| *r.channels[key].last().unwrap();
    let (q_off, q_on, v0, v) = (
        end(&off, "qe:104"),
        end(&on, "qe:104"),
        on.channels["vm:104"][0],
        end(&on, "vm:104"),
    );
    let expected = q0 * (v / v0).powi(2);
    eprintln!(
        "Q0 {q0:.5}, end without {q_off:.5}, with {q_on:.5} ({expected:.5}), V {v0:.5} -> {v:.5}"
    );
    assert!(q0 > 0.05, "the plant starts at Q = {q0}");
    assert!(v < v0 - 0.005, "the voltage dips from {v0} to {v}");
    assert!(
        (q_off - q0).abs() < 1e-4,
        "without the exponent Q returns to {q_off}"
    );
    assert!(
        (q_on - expected).abs() < 1e-4,
        "with it Q settles at {q_on}"
    );
}

// ---------------------------------------------------------------------------
// HVDC links in AC emulation.

/// The load at 4 up by a fifth at 0.1 s.
fn tie_load_step() -> Vec<RmsEvent> {
    vec![RmsEvent::LoadChange {
        time: 0.1,
        bus: 4,
        scale: 1.2,
    }]
}

/// Without links (none, or one naming buses the case lacks) a run is bit for
/// bit the run without emulation; a link with `K = 0` changes it at the
/// rounding level only (its zero Jacobian entries change the factorization).
#[test]
fn hvdc_emulation_without_links_changes_nothing() {
    let _s = switches();
    let base = run(&two_areas(), 2.0, tie_load_step());
    rms::set_hvdc_emulation(Vec::new());
    assert_bit_identical(&base, &run(&two_areas(), 2.0, tie_load_step()), "no links");
    rms::set_hvdc_emulation(vec![(3, 99, 50.0, 0.0)]);
    assert_bit_identical(
        &base,
        &run(&two_areas(), 2.0, tie_load_step()),
        "a link to a missing bus",
    );
    rms::set_hvdc_emulation(vec![(3, 4, 0.0, 0.0)]);
    let (d, key) = largest_difference(&base, &run(&two_areas(), 2.0, tie_load_step()));
    eprintln!("K = 0: largest difference {d:e} in {key}");
    assert!(d < 1e-9, "K = 0: {key} differs by {d:e}");
}

/// With a link the start is unchanged (the link moves nothing at its start
/// angle) and stays at rest.
#[test]
fn hvdc_emulation_starts_flat() {
    let _s = switches();
    rms::set_hvdc_emulation(vec![(3, 4, 50.0, 0.0)]);
    assert_flat(&two_areas(), "HVDC emulation");
}

/// The inter-area mode's frequency rises with `K`: the link adds
/// synchronizing power between the areas, as a parallel AC line would.
#[test]
fn hvdc_emulation_raises_the_inter_area_frequency() {
    let _s = switches();
    let mut last = 0.0;
    for k in [0.0, 10.0, 50.0] {
        if k > 0.0 {
            rms::set_hvdc_emulation(vec![(3, 4, k, 0.0)]);
        }
        let eig = two_areas().eigenvalues().unwrap();
        let f = eig.iter().map(|e| e[1].abs()).fold(0.0, f64::max) / (2.0 * std::f64::consts::PI);
        eprintln!("K = {k} MW/deg: inter-area mode at {f:.4} Hz");
        assert!(f > last * 1.01, "K = {k}: {f} Hz after {last} Hz");
        last = f;
    }
}

/// The power the link moves out of bus 3, from the balance at that bus (no
/// unit there): what its lines and its load do not take.
fn link_power(r: &RmsResult) -> Vec<f64> {
    let c = |key: &str| &r.channels[key];
    let (a, b, l) = (c("p_from:3-4"), c("p_to:1-3"), c("pl:3"));
    (0..r.time.len()).map(|k| -(a[k] + b[k] + l[k])).collect()
}

/// After a load step the link moves `K (Δθ − Δθ0)` from 3 to 4, `K` in MW
/// per degree of the angle of 3 against 4 and `Δθ0` that angle at the
/// start; limited to `p_max`, it never moves more, and the unlimited link
/// takes part of the tie line's flow.
#[test]
fn hvdc_emulation_moves_k_times_the_angle_change() {
    let _s = switches();
    let k_mw_deg = 50.0;
    let none = run(&two_areas(), 2.0, tie_load_step());
    rms::set_hvdc_emulation(vec![(3, 4, k_mw_deg, 0.0)]);
    let free = run(&two_areas(), 2.0, tie_load_step());
    rms::set_hvdc_emulation(vec![(3, 4, k_mw_deg, 1.0)]);
    let limited = run(&two_areas(), 2.0, tie_load_step());
    let k = k_mw_deg * 180.0 / std::f64::consts::PI / 100.0;
    let expected = |r: &RmsResult| -> Vec<f64> {
        let (a, b) = (&r.channels["va:3"], &r.channels["va:4"]);
        (0..r.time.len())
            .map(|i| k * ((a[i] - b[i]) - (a[0] - b[0])))
            .collect()
    };
    let (moved, want) = (link_power(&free), expected(&free));
    let largest = want.iter().fold(0.0_f64, |m, p| m.max(p.abs()));
    assert!(largest > 0.05, "the link moves up to {largest} pu");
    for i in 0..moved.len() {
        assert!(
            (moved[i] - want[i]).abs() < 1e-6,
            "t = {}: {} against {}",
            free.time[i],
            moved[i],
            want[i]
        );
    }
    let (moved, want) = (link_power(&limited), expected(&limited));
    for i in 0..moved.len() {
        let w = want[i].clamp(-0.01, 0.01);
        assert!(
            (moved[i] - w).abs() < 1e-6,
            "t = {}: {} against {w}",
            limited.time[i],
            moved[i]
        );
    }
    assert!(
        moved.iter().any(|p| (p.abs() - 0.01).abs() < 1e-9),
        "the limit is reached"
    );
    let tie = |r: &RmsResult| *r.channels["p_from:3-4"].last().unwrap();
    let tie0 = none.channels["p_from:3-4"][0];
    assert!(
        (tie(&free) - tie0).abs() < (tie(&none) - tie0).abs(),
        "tie flow change: {} with the link, {} without",
        tie(&free) - tie0,
        tie(&none) - tie0
    );
}

// ---------------------------------------------------------------------------
// Relief in a collapse.

/// A tripped tie leaves a pocket (the unit at 102 and the load at 103)
/// whose constant-power load the unit cannot carry: the re-solve fails, the
/// run trips the pocket's unit (relief level 0), the dead pocket loses its
/// load, and the run goes on to its end with the rest of the network
/// untouched. The constant-power load keeps its law at low voltage here
/// (with the PQBRAK conversion the pocket would have a low-voltage solution).
#[test]
fn a_pocket_that_cannot_be_solved_is_relieved() {
    let _s = switches();
    let raw = raw_case(
        &[
            (101, 3, 1.0),
            (102, 2, 1.0),
            (103, 1, 1.0),
            (105, 1, 1.0),
            (106, 1, 1.0),
        ],
        &[(101, 0.0, 1.0, 100.0, 1e-5), (102, 20.0, 1.0, 50.0, 0.5)],
        &[(103, 150.0, 0.0), (106, 20.0, 0.0)],
        &[
            (101, 103, "1", 0.05),
            (102, 103, "1", 0.1),
            (101, 105, "1", 0.05),
            (105, 106, "1", 0.05),
        ],
    );
    let dyr = format!("{INFINITE_101}102 'GENCLS' 1 3.0 0.0 /\n");
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    let options = RmsOptions {
        low_voltage_load: LowVoltageLoad::Exact,
        ..RmsOptions::default()
    };
    let sim = RmsSimulation::with_options(&network(&raw), &dyr, LoadModel::ConstantPower, options)
        .unwrap();
    let r = sim.run(&RmsRequest {
        t_end: 0.5,
        dt: 0.005,
        output_step: 0.005,
        events: line_trip(0.1),
        ..RmsRequest::default()
    });
    let r = r.unwrap();
    let has = |text: &str| r.events.iter().any(|e| e.contains(text));
    assert!(has("1 unit(s) in small pockets"), "{:?}", r.events);
    assert!(
        has("1 dead bus(es) lose their load (150.0 MW"),
        "{:?}",
        r.events
    );
    assert!(!has("relief level"), "level 0 suffices: {:?}", r.events);
    assert_eq!(*r.time.last().unwrap(), 0.5);
    let k = at(&r, 0.1) + 1;
    assert!(r.channels["pl:103"][k..].iter().all(|&p| p == 0.0));
    let (pl, vm) = (&r.channels["pl:106"], &r.channels["vm:106"]);
    assert!(pl.iter().all(|&p| (p - 0.2).abs() < 1e-9));
    // The infinite bus has a small source impedance: its voltage moves a
    // little as its outflow drops.
    assert!(vm.iter().all(|&v| (v - vm[0]).abs() < 1e-3));
}

/// A load bus whose constant-power demand (700 MW) cannot be carried after a
/// line trip: no pocket to drop, so the run takes the bus the solver's
/// failure names (relief level 1) and that bus loses its load; the run goes
/// on to its end with the bus unloaded at about the infinite bus's voltage.
#[test]
fn a_bus_that_cannot_be_solved_loses_its_load() {
    let _s = switches();
    let raw = raw_case(
        &[(101, 3, 1.0), (103, 1, 1.0)],
        &[(101, 0.0, 1.0, 100.0, 1e-5)],
        &[(103, 700.0, 0.0)],
        &[(101, 103, "1", 0.1), (101, 103, "2", 0.1)],
    );
    let dyr = powerio::parse_dyr(INFINITE_101).unwrap();
    let options = RmsOptions {
        low_voltage_load: LowVoltageLoad::Exact,
        ..RmsOptions::default()
    };
    let sim = RmsSimulation::with_options(&network(&raw), &dyr, LoadModel::ConstantPower, options)
        .unwrap();
    let r = sim.run(&RmsRequest {
        t_end: 0.5,
        dt: 0.005,
        output_step: 0.005,
        events: line_trip(0.1),
        ..RmsRequest::default()
    });
    let r = r.unwrap();
    assert!(
        r.events.iter().any(|e| e.contains(
            "(relief level 1, bus 103); 0 unit(s) trip and 1 bus(es) lose their load (700.0 MW"
        )),
        "{:?}",
        r.events
    );
    assert_eq!(*r.time.last().unwrap(), 0.5);
    let k = at(&r, 0.1) + 1;
    assert!(r.channels["pl:103"][k..].iter().all(|&p| p == 0.0));
    assert!(r.channels["vm:103"][k..]
        .iter()
        .all(|&v| (v - 1.0).abs() < 1e-3));
}
