//! Small-signal analysis of the RMS system at its initial operating point:
//! modes with participation factors, and frequency responses.
//!
//! The system `M ẋ = f(x, u)` linearizes to the descriptor form
//! `M δẋ = J δx + E δu`, `δy = C δx`. Modes are the eigenvalues of the
//! reduced matrix `A = M_d⁻¹ (J_dd − J_da J_aa⁻¹ J_ad)` over the differential
//! states (PowerSimulationsDynamics.jl `small_signal.jl`), with participation
//! factors `p_ki = |v_ki| |w_ik|` normalized per mode, `W = V⁻¹`. A frequency
//! response solves the descriptor system directly,
//! `H(jω) = C (jω M − J)⁻¹ E`, so algebraic states need no elimination.

use faer::linalg::solvers::Solve;
use faer::{c64, Mat};
use serde::{Deserialize, Serialize};

use super::system::DynSystem;

/// A generator reference that drives a frequency response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSignal {
    /// Exciter voltage reference (field voltage for a fixed exciter).
    VRef,
    /// Governor power reference (mechanical torque for a fixed governor).
    PRef,
}

/// A measured quantity of a frequency response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputSignal {
    /// Rotor speed of the generator.
    Omega,
    /// Rotor angle of the generator.
    Delta,
    /// Field voltage of the generator.
    Efd,
    /// Electrical active power of the generator (system base).
    Pe,
    /// Voltage magnitude of a bus.
    Vm,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BodeSpec {
    pub input_bus: usize,
    #[serde(default = "one")]
    pub input_machine: String,
    pub input: InputSignal,
    pub output: OutputSignal,
    /// Generator bus for machine outputs, any bus for `vm`.
    pub output_bus: usize,
    #[serde(default = "one")]
    pub output_machine: String,
}

fn one() -> String {
    "1".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinearRequest {
    #[serde(default)]
    pub bode: Vec<BodeSpec>,
    #[serde(default = "f_min")]
    pub f_min: f64,
    #[serde(default = "f_max")]
    pub f_max: f64,
    #[serde(default = "points")]
    pub points: usize,
    /// Participation factors kept per mode.
    #[serde(default = "top")]
    pub participation_top: usize,
}

fn f_min() -> f64 {
    0.01
}
fn f_max() -> f64 {
    20.0
}
fn points() -> usize {
    240
}
fn top() -> usize {
    6
}

impl Default for LinearRequest {
    fn default() -> Self {
        Self {
            bode: Vec::new(),
            f_min: f_min(),
            f_max: f_max(),
            points: points(),
            participation_top: top(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Participation {
    pub state: String,
    pub factor: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mode {
    pub re: f64,
    pub im: f64,
    pub freq_hz: f64,
    /// Damping ratio `−re/|λ|` (1 for a real mode).
    pub damping: f64,
    pub participation: Vec<Participation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BodeCurve {
    pub label: String,
    pub freq_hz: Vec<f64>,
    pub mag_db: Vec<f64>,
    pub phase_deg: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinearResult {
    pub states: Vec<String>,
    pub modes: Vec<Mode>,
    pub bode: Vec<BodeCurve>,
}

/// Names of every global state, `bus:quantity` for buses and
/// `G<bus>[ 'id'].<state>` for devices.
pub(crate) fn state_names(sys: &DynSystem) -> Vec<String> {
    let mut names = vec![String::new(); sys.nx];
    for (b, number) in sys.bus_numbers.iter().enumerate() {
        names[b] = format!("bus {number} Vr");
        names[sys.n + b] = format!("bus {number} Vi");
    }
    for d in &sys.devices {
        let tag = if d.machine_id == "1" {
            format!("G{}", d.bus_number)
        } else {
            format!("G{} '{}'", d.bus_number, d.machine_id)
        };
        for (k, s) in d.state_names().into_iter().enumerate() {
            names[d.offset + k] = format!("{tag} {s}");
        }
    }
    for h in &sys.held {
        let Some(p) = &h.plant else { continue };
        let tag = if h.machine_id == "1" {
            format!("R{}", h.bus_number)
        } else {
            format!("R{} '{}'", h.bus_number, h.machine_id)
        };
        for (k, s) in p.state_names().into_iter().enumerate() {
            names[h.offset + k] = format!("{tag} {s}");
        }
    }
    names
}

/// Largest system (states, network voltages included) whose eigenvalues and
/// modes are computed: the linearization is a dense `nx × nx` matrix (0.3 GB
/// at this size) and its eigenvalue problem costs `O(nx³)`, more than a
/// browser module can hold for continental-scale cases.
pub(crate) const MAX_EIGEN_STATES: usize = 6000;

/// `Err` with the reason when the system is too large for the dense
/// eigenvalue analysis.
pub(crate) fn check_eigen_size(sys: &DynSystem) -> Result<(), String> {
    if sys.nx > MAX_EIGEN_STATES {
        return Err(format!(
            "eigenvalues not computed: the linearized system has {} states, more than the {MAX_EIGEN_STATES} a dense eigenvalue analysis is run for",
            sys.nx
        ));
    }
    Ok(())
}

fn dense_jacobian(sys: &DynSystem, x: &[f64]) -> Mat<f64> {
    let mut j = Mat::<f64>::zeros(sys.nx, sys.nx);
    for (r, c, v) in sys.jacobian(x) {
        j[(r, c)] += v;
    }
    j
}

/// The reduced state matrix over the differential states and their indices.
pub(crate) fn reduced_a(sys: &DynSystem, x: &[f64]) -> (Mat<f64>, Vec<usize>) {
    let j = dense_jacobian(sys, x);
    let diff: Vec<usize> = (0..sys.nx).filter(|&i| sys.mass[i] != 0.0).collect();
    let alg_all: Vec<usize> = (0..sys.nx).filter(|&i| sys.mass[i] == 0.0).collect();
    // As PowerSimulationsDynamics.jl (`_reduce_jacobian`): an algebraic state
    // whose row and column in the algebraic block are all zero (the input
    // stage of a filterless IEEEST, which only constrains differential
    // states) is dropped before the algebraic block is inverted.
    let alg: Vec<usize> = alg_all
        .iter()
        .copied()
        .filter(|&i| {
            alg_all
                .iter()
                .any(|&k| j[(i, k)] != 0.0 || j[(k, i)] != 0.0)
        })
        .collect();
    let pick = |rows: &[usize], cols: &[usize]| {
        Mat::<f64>::from_fn(rows.len(), cols.len(), |i, k| j[(rows[i], cols[k])])
    };
    let fdd = pick(&diff, &diff);
    let reduced = if alg.is_empty() {
        fdd
    } else {
        let faa = pick(&alg, &alg);
        let fad = pick(&alg, &diff);
        let fda = pick(&diff, &alg);
        let z = faa.full_piv_lu().solve(&fad);
        &fdd - &fda * &z
    };
    let a = Mat::<f64>::from_fn(diff.len(), diff.len(), |i, k| {
        reduced[(i, k)] / sys.mass[diff[i]]
    });
    (a, diff)
}

/// Modes with participation factors.
pub(crate) fn modes(
    sys: &DynSystem,
    x: &[f64],
    keep: usize,
) -> Result<(Vec<String>, Vec<Mode>), String> {
    check_eigen_size(sys)?;
    let names = state_names(sys);
    let (a, diff) = reduced_a(sys, x);
    let n = diff.len();
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let evd = faer::linalg::solvers::Eigen::new_from_real(a.as_ref())
        .map_err(|e| format!("eigen decomposition failed: {e:?}"))?;
    let v = evd.U();
    let s = evd.S().column_vector();
    let v_owned = Mat::<c64>::from_fn(n, n, |i, k| v[(i, k)]);
    let ident = Mat::<c64>::from_fn(n, n, |i, k| {
        if i == k {
            c64::new(1.0, 0.0)
        } else {
            c64::new(0.0, 0.0)
        }
    });
    let w = v_owned.full_piv_lu().solve(&ident);
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let lambda = s[k];
        let mut p: Vec<(usize, f64)> = (0..n)
            .map(|i| (i, v_owned[(i, k)].norm() * w[(k, i)].norm()))
            .collect();
        let total: f64 = p.iter().map(|(_, f)| f).sum::<f64>().max(1e-300);
        p.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mag = lambda.norm();
        out.push(Mode {
            re: lambda.re,
            im: lambda.im,
            freq_hz: lambda.im.abs() / (2.0 * std::f64::consts::PI),
            damping: if mag > 1e-12 { -lambda.re / mag } else { 1.0 },
            participation: p
                .into_iter()
                .take(keep)
                .map(|(i, f)| Participation {
                    state: names[diff[i]].clone(),
                    factor: f / total,
                })
                .collect(),
        });
    }
    out.sort_by(|a, b| {
        a.damping
            .total_cmp(&b.damping)
            .then(b.freq_hz.total_cmp(&a.freq_hz))
    });
    Ok((diff.iter().map(|&i| names[i].clone()).collect(), out))
}

/// Value of an output at `x` (by evaluation, so derived outputs like the
/// field voltage and electrical power are exact).
fn output_value(sys: &DynSystem, x: &[f64], spec: &BodeSpec) -> Result<f64, String> {
    let n = sys.n;
    if spec.output == OutputSignal::Vm {
        let b = sys
            .dense_bus(spec.output_bus)
            .ok_or_else(|| format!("no bus {}", spec.output_bus))?;
        return Ok(x[b].hypot(x[n + b]));
    }
    let d = sys
        .devices
        .iter()
        .find(|d| d.bus_number == spec.output_bus && d.machine_id == spec.output_machine.trim())
        .ok_or_else(|| {
            format!(
                "no dynamic generator {} '{}'",
                spec.output_bus, spec.output_machine
            )
        })?;
    let [_, s, ..] = d.layout();
    Ok(match spec.output {
        OutputSignal::Omega => x[d.offset + s + 1],
        OutputSignal::Delta => x[d.offset + s],
        OutputSignal::Efd | OutputSignal::Pe => {
            let k = d.n_states();
            let mut f = vec![0.0; k];
            let (vr, vi) = (x[d.bus], x[n + d.bus]);
            let o = d.eval(&x[d.offset..d.offset + k], vr, vi, sys.omega_sys(x), &mut f);
            if spec.output == OutputSignal::Efd {
                o.vf
            } else {
                vr * o.ir + vi * o.ii
            }
        }
        OutputSignal::Vm => unreachable!(),
    })
}

fn signal_name(spec: &BodeSpec) -> String {
    let gen = |bus: usize, id: &str| {
        if id.trim() == "1" {
            format!("G{bus}")
        } else {
            format!("G{bus} '{id}'")
        }
    };
    let input = match spec.input {
        InputSignal::VRef => "V_ref",
        InputSignal::PRef => "P_ref",
    };
    let output = match spec.output {
        OutputSignal::Omega => format!("{} ω", gen(spec.output_bus, &spec.output_machine)),
        OutputSignal::Delta => format!("{} δ", gen(spec.output_bus, &spec.output_machine)),
        OutputSignal::Efd => format!("{} Efd", gen(spec.output_bus, &spec.output_machine)),
        OutputSignal::Pe => format!("{} Pe", gen(spec.output_bus, &spec.output_machine)),
        OutputSignal::Vm => format!("bus {} |V|", spec.output_bus),
    };
    format!(
        "{} {input} → {output}",
        gen(spec.input_bus, &spec.input_machine)
    )
}

/// Frequency responses of the descriptor system.
pub(crate) fn bode(
    sys: &DynSystem,
    x: &[f64],
    req: &LinearRequest,
) -> Result<Vec<BodeCurve>, String> {
    if req.bode.is_empty() {
        return Ok(Vec::new());
    }
    if sys.nx > 2500 {
        return Err(format!(
            "frequency response is limited to 2500 states, this system has {}",
            sys.nx
        ));
    }
    let nx = sys.nx;
    let jac = sys.jacobian(x);
    let mut f0 = vec![0.0; nx];
    sys.residual(x, &mut f0, None);
    let points = req.points.clamp(2, 2000);
    let (lo, hi) = (
        req.f_min.max(1e-4).ln(),
        req.f_max.max(req.f_min * 1.0001).ln(),
    );
    let freqs: Vec<f64> = (0..points)
        .map(|k| (lo + (hi - lo) * k as f64 / (points - 1) as f64).exp())
        .collect();
    let mut curves = Vec::new();
    for spec in &req.bode {
        // E: ∂f/∂u by a forward difference of the reference.
        let di = sys
            .devices
            .iter()
            .position(|d| {
                d.bus_number == spec.input_bus && d.machine_id == spec.input_machine.trim()
            })
            .ok_or_else(|| {
                format!(
                    "no dynamic generator {} '{}'",
                    spec.input_bus, spec.input_machine
                )
            })?;
        let mut perturbed = sys.clone();
        let h = 1e-6;
        match spec.input {
            InputSignal::VRef => perturbed.devices[di].v_ref += h,
            InputSignal::PRef => perturbed.devices[di].p_ref += h,
        }
        let mut f1 = vec![0.0; nx];
        perturbed.residual(x, &mut f1, None);
        let e: Vec<f64> = (0..nx).map(|i| (f1[i] - f0[i]) / h).collect();
        // C: ∂y/∂x by forward differences.
        let y0 = output_value(sys, x, spec)?;
        let mut xp = x.to_vec();
        let mut c = vec![0.0; nx];
        for i in 0..nx {
            let step = 1e-7 * x[i].abs().max(1.0);
            xp[i] += step;
            c[i] = (output_value(sys, &xp, spec)? - y0) / step;
            xp[i] = x[i];
        }
        let mut curve = BodeCurve {
            label: signal_name(spec),
            freq_hz: freqs.clone(),
            mag_db: Vec::with_capacity(points),
            phase_deg: Vec::with_capacity(points),
        };
        let mut last_phase: Option<f64> = None;
        for &f in &freqs {
            // (jωM − J) is as sparse as the Jacobian: a sparse complex LU per
            // frequency keeps large systems (WECC 240: ~1,800 states) cheap.
            let s = c64::new(0.0, 2.0 * std::f64::consts::PI * f);
            let mut trip: Vec<faer::sparse::Triplet<usize, usize, c64>> = jac
                .iter()
                .map(|&(r, k, v)| faer::sparse::Triplet::new(r, k, c64::new(-v, 0.0)))
                .collect();
            for i in 0..nx {
                if sys.mass[i] != 0.0 {
                    trip.push(faer::sparse::Triplet::new(i, i, s * sys.mass[i]));
                }
            }
            let a = faer::sparse::SparseColMat::<usize, c64>::try_new_from_triplets(nx, nx, &trip)
                .map_err(|e| format!("assembling jωM − J: {e:?}"))?;
            let lu = a
                .sp_lu()
                .map_err(|e| format!("factorizing jωM − J at {f} Hz: {e:?}"))?;
            let mut xs = Mat::<c64>::from_fn(nx, 1, |r, _| c64::new(e[r], 0.0));
            lu.solve_in_place(xs.as_mut());
            let mut hval = c64::new(0.0, 0.0);
            for i in 0..nx {
                hval += xs[(i, 0)] * c[i];
            }
            curve.mag_db.push(20.0 * hval.norm().max(1e-300).log10());
            // Unwrap the phase along the sweep.
            let mut ph = hval.arg().to_degrees();
            if let Some(prev) = last_phase {
                while ph - prev > 180.0 {
                    ph -= 360.0;
                }
                while ph - prev < -180.0 {
                    ph += 360.0;
                }
            }
            last_phase = Some(ph);
            curve.phase_deg.push(ph);
        }
        curves.push(curve);
    }
    Ok(curves)
}
