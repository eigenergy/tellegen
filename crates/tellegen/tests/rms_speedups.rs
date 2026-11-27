//! The speed-ups of the RMS engine, each switched off against the default,
//! on a short bus fault on the 240-bus example case that ships with the code
//! (`RmsOptions`):
//!
//! - `threads` off (everything on one thread) gives the default run bit for
//!   bit;
//! - `factorize_by_blocks` off (one sparse LU over all unknowns) solves the
//!   same Newton systems in another order;
//! - `refactor_values` off (every factorization pivoted afresh) changes
//!   only the rounding of the factors;
//! - `keep_factorization` off (a fixed refresh schedule) changes Newton's
//!   iterates, while its stopping test still decides convergence;
//!
//! so each of the last three agrees with the default within the tolerance
//! stated at its test. Without the shipped case data (the published crate)
//! every test here is skipped.

#![cfg(feature = "rms")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use tellegen::rms::{LoadModel, RmsEvent, RmsOptions, RmsRequest, RmsResult, RmsSimulation};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/wecc240.rs"
));

/// The shipped 240-bus case with `options`, or `None` without its data.
fn sim_with(options: RmsOptions) -> Option<RmsSimulation> {
    let dir = shipped_wecc240()?;
    let net = shipped_wecc240_network()?;
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    Some(RmsSimulation::with_options(&net, &dyr, LoadModel::ConstantImpedance, options).unwrap())
}

/// A 0.1 s bus fault from 0.1 s, run to 0.4 s.
fn request() -> RmsRequest {
    RmsRequest {
        t_end: 0.4,
        dt: 0.005,
        output_step: 0.005,
        load_model: Some(LoadModel::ConstantImpedance),
        events: vec![
            RmsEvent::BusFault {
                time: 0.1,
                bus: 1101,
                r: 0.0,
                x: 0.05,
            },
            RmsEvent::ClearFault {
                time: 0.2,
                bus: 1101,
            },
        ],
        small_signal: false,
        record: None,
    }
}

/// The default run and the run with `off` applied to the default options.
fn default_and(off: impl Fn(&mut RmsOptions)) -> Option<(RmsResult, RmsResult)> {
    let mut options = RmsOptions::default();
    off(&mut options);
    let base = sim_with(RmsOptions::default())?.run(&request()).unwrap();
    let other = sim_with(options)?.run(&request()).unwrap();
    assert_eq!(base.time, other.time);
    assert_eq!(
        base.channels.keys().collect::<Vec<_>>(),
        other.channels.keys().collect::<Vec<_>>()
    );
    Some((base, other))
}

/// The largest difference of two runs, each channel against its own largest
/// magnitude (at least 1), and the channel where it is.
fn largest_difference(a: &RmsResult, b: &RmsResult) -> (f64, String) {
    let mut worst = (0.0, String::new());
    for (k, va) in &a.channels {
        let vb = &b.channels[k];
        let scale = vb.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
        for (x, y) in va.iter().zip(vb) {
            let d = (x - y).abs() / scale;
            // NaN on either side counts as the largest difference.
            if !(d <= worst.0) {
                worst = (d, k.clone());
            }
        }
    }
    worst
}

/// `threads` off: the same run bit for bit, the same steps and Newton
/// iterations.
#[test]
fn threads_off_gives_the_default_run_bit_for_bit() {
    let Some((base, other)) = default_and(|o| o.threads = false) else {
        return;
    };
    for (k, v) in &base.channels {
        let w = &other.channels[k];
        assert!(
            v.iter().zip(w).all(|(x, y)| x.to_bits() == y.to_bits()),
            "{k}"
        );
    }
    assert_eq!(
        (base.steps, base.newton_iterations),
        (other.steps, other.newton_iterations)
    );
}

/// `factorize_by_blocks` off: the step matrices are factorized as one
/// sparse LU instead of unit by unit onto the network, the same linear
/// systems solved in another order ("the corrections agree to rounding"):
/// the runs agree to rounding, within 1e-9 of each channel's largest
/// magnitude (about 1e-11 seen).
#[test]
fn factorization_by_blocks_off_agrees_to_rounding() {
    let Some((base, other)) = default_and(|o| o.factorize_by_blocks = false) else {
        return;
    };
    let (d, k) = largest_difference(&base, &other);
    assert!(d <= 1e-9, "{d:e} in {k}");
}

/// `refactor_values` off: every factorization pivots afresh instead of
/// refactorizing in the pivot order of its pattern's first one, which
/// changes the factors' rounding only: the runs agree to rounding, within
/// 1e-9 of each channel's largest magnitude (about 6e-12 seen).
#[test]
fn refactor_values_off_agrees_to_rounding() {
    let Some((base, other)) = default_and(|o| o.refactor_values = false) else {
        return;
    };
    let (d, k) = largest_difference(&base, &other);
    assert!(d <= 1e-9, "{d:e} in {k}");
}

/// `keep_factorization` off: the factorization is refreshed on a fixed
/// schedule and after every event, so Newton takes other iterates, but its
/// stopping test (1e-10 on each step's iterate) decides convergence either
/// way: the runs agree within that tolerance carried over the run's 80
/// steps and into the channels computed from the states, within 1e-7 of
/// each channel's largest magnitude (about 3e-9 seen).
#[test]
fn keep_factorization_off_agrees_within_newtons_tolerance() {
    let Some((base, other)) = default_and(|o| o.keep_factorization = false) else {
        return;
    };
    let (d, k) = largest_difference(&base, &other);
    assert!(d <= 1e-7, "{d:e} in {k}");
}
