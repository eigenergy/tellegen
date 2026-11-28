//! Discrete events acting on the simulated network: a branch trip or close,
//! a bus or line fault and its clearing, a load change, a new controller
//! reference, a generator trip, a breaker at one branch end. Each event
//! changes the system in place and returns the line the run's event log
//! shows; the caller then rebuilds the admittance and mass matrices and
//! re-solves the network.
//!
//! @block Events

use super::system::{self, DynSystem, C64};
use super::{topology, InputSignal, RmsEvent};

fn find_branch(sys: &DynSystem, from: usize, to: usize, circuit: &str) -> Result<usize, String> {
    sys.branch_keys
        .iter()
        .position(|(f, t, c)| {
            ((*f == from && *t == to) || (*f == to && *t == from)) && c.trim() == circuit.trim()
        })
        .ok_or_else(|| format!("no branch {from}-{to} circuit {circuit}"))
}

/// Apply `event` to the system (and, for a generator trip, freeze the
/// unit's states from `x`); returns the line for the event log.
// @block Apply one event
pub(super) fn apply_event(
    sys: &mut DynSystem,
    x: &mut [f64],
    event: &RmsEvent,
) -> Result<String, String> {
    let bus = |sys: &DynSystem, b: usize| sys.dense_bus(b).ok_or_else(|| format!("no bus {b}"));
    Ok(match event {
        RmsEvent::BranchTrip {
            time,
            from,
            to,
            circuit,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            sys.closed[e] = false;
            format!("t = {time} s: branch {from}-{to} circuit {circuit} opened")
        }
        RmsEvent::BranchClose {
            time,
            from,
            to,
            circuit,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            sys.closed[e] = true;
            format!("t = {time} s: branch {from}-{to} circuit {circuit} closed")
        }
        RmsEvent::BusFault {
            time,
            bus: b,
            r,
            x: xf,
        } => {
            let k = bus(sys, *b)?;
            sys.fault_y[k] = fault_admittance(*r, *xf)?;
            format!("t = {time} s: fault at bus {b} through {r} + j{xf} pu")
        }
        RmsEvent::ClearFault { time, bus: b } => {
            let k = bus(sys, *b)?;
            sys.fault_y[k] = C64::new(0.0, 0.0);
            format!("t = {time} s: fault at bus {b} cleared")
        }
        RmsEvent::LoadChange {
            time,
            bus: b,
            scale,
        } => {
            let k = bus(sys, *b)?;
            if !(scale.is_finite() && *scale >= 0.0) {
                return Err(format!(
                    "load scale {scale} is not a finite number >= 0 (a negative scale turns loads into sources)"
                ));
            }
            sys.loads[k].scale(*scale, *scale);
            format!("t = {time} s: load at bus {b} scaled by {scale}")
        }
        RmsEvent::SetReference {
            time,
            bus: b,
            machine_id,
            signal,
            value,
        } => {
            if !value.is_finite() {
                return Err(format!("reference value {value} is not finite"));
            }
            let d = sys
                .devices
                .iter_mut()
                .find(|d| d.bus_number == *b && d.machine_id == machine_id.trim())
                .ok_or_else(|| format!("no dynamic generator {b} '{machine_id}'"))?;
            let name = match signal {
                InputSignal::VRef => {
                    d.v_ref = *value;
                    "V_ref"
                }
                InputSignal::PRef => {
                    d.p_ref = *value;
                    "P_ref"
                }
                InputSignal::Vs => {
                    d.vs_ext = *value;
                    "V_s"
                }
            };
            format!("t = {time} s: generator {b} '{machine_id}' {name} = {value}")
        }
        RmsEvent::GeneratorTrip {
            time,
            bus: b,
            machine_id,
        } => {
            if let Some(d) = sys
                .devices
                .iter_mut()
                .find(|d| d.bus_number == *b && d.machine_id == machine_id.trim())
            {
                d.tripped = Some(x[d.offset..d.offset + d.n_states()].to_vec());
            } else if let Some(s) = sys
                .sources
                .iter_mut()
                .find(|s| s.bus_number == *b && s.machine_id == machine_id.trim())
            {
                s.y = C64::new(0.0, 0.0);
                s.e = C64::new(0.0, 0.0);
            } else if let Some(k) = sys
                .held
                .iter()
                .position(|u| u.bus_number == *b && u.machine_id == machine_id.trim())
            {
                topology::trip_held(sys, k);
            } else {
                return Err(format!("no generator {b} '{machine_id}'"));
            }
            format!("t = {time} s: generator {b} '{machine_id}' tripped")
        }
        RmsEvent::LineFault {
            time,
            from,
            to,
            circuit,
            location,
            r,
            x: xf,
            permanent,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            if !(0.0..=1.0).contains(location) {
                return Err(format!("fault location {location} must lie in [0, 1]"));
            }
            // Measured from the branch's own from end.
            let alpha = if sys.branch_keys[e].0 == *from {
                *location
            } else {
                1.0 - location
            };
            sys.branch_faults[e] = Some(system::BranchFault {
                alpha,
                y: fault_admittance(*r, *xf)?,
                permanent: *permanent,
            });
            format!(
                "t = {time} s: {} fault at {:.0} % of {from}-{to} circuit {circuit} through {r} + j{xf} pu",
                if *permanent { "permanent" } else { "transient" },
                location * 100.0
            )
        }
        RmsEvent::ClearLineFault {
            time,
            from,
            to,
            circuit,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            sys.branch_faults[e] = None;
            format!("t = {time} s: fault inside {from}-{to} circuit {circuit} cleared")
        }
        RmsEvent::BreakerOpen {
            time,
            from,
            to,
            circuit,
            at,
        }
        | RmsEvent::BreakerClose {
            time,
            from,
            to,
            circuit,
            at,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            let k = if sys.branch_keys[e].0 == *at {
                0
            } else if sys.branch_keys[e].1 == *at {
                1
            } else {
                return Err(format!("bus {at} is not an end of {from}-{to}"));
            };
            let open = matches!(event, RmsEvent::BreakerOpen { .. });
            sys.end_open[e][k] = open;
            format!(
                "t = {time} s: breaker of {from}-{to} circuit {circuit} at bus {at} {}",
                if open { "opened" } else { "closed" }
            )
        }
    })
}

/// The admittance of a fault through `r + jx`; a fault needs a finite,
/// nonzero impedance.
// @block Fault admittance
fn fault_admittance(r: f64, x: f64) -> Result<C64, String> {
    let z = C64::new(r, x);
    if r < 0.0 || x < 0.0 {
        return Err(format!(
            "a fault impedance needs r >= 0 and x >= 0, not {r} + j{x} (a negative part makes the fault a source)"
        ));
    }
    if !(z.norm() > 0.0 && z.norm().is_finite()) {
        return Err(format!(
            "a fault needs a finite, nonzero impedance r + jx, not {r} + j{x} (use a small reactance for a bolted fault)"
        ));
    }
    Ok(C64::new(1.0, 0.0) / z)
}
