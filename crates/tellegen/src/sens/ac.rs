//! AC power flow differentiable system: the [`AcNewton`] wrapper over a converged
//! Newton solution, exposing the bus voltage operands and the demand parameter.
//!
//! At a converged solution the power flow residual `F(x; pd) = calc(x) − spec(pd)`
//! is zero, with `x = [va(non-slack); vm(PQ)]` under slack/PV/PQ bus typing and
//! `spec_P = pg − pd`. The Newton Jacobian `J = ∂calc/∂x` is exactly what the solve
//! assembles, and the real demand enters only the real power rows, so `∂F/∂pd` is a
//! selector. The implicit function theorem gives `dx/dpd = −J⁻¹ (∂F/∂pd)`, run forward
//! or adjoint by the shared [`crate::sens::sensitivity`] driver; the converged `J` is
//! nonsingular, so the solve needs no regularization.
//!
//! The slack bus has no row, PV buses hold their magnitude, so the angle operand ranges
//! over the non-slack buses and the magnitude operand over the PQ buses; each result row
//! names its bus through the matrix metadata.

use faer::Mat;

use num_complex::Complex;

use crate::model::AcNetwork;
use crate::problem::{ac_injections, ac_jacobian, AcPfLayout, AcPfSolution};

use super::{
    Axis, Differentiable, ElementId, End, Operand, Parameter, Power, Selector, SensError,
    SolveSpec, VoltageKind,
};

/// A converged AC power flow as a differentiable Newton system. Assembles the polar
/// Jacobian once at construction and records the free buses (every non-slack bus, the
/// rows of the reduced system). Borrows the network.
#[non_exhaustive]
pub struct AcNewton<'a> {
    net: &'a AcNetwork,
    layout: AcPfLayout,
    jac: Vec<(usize, usize, f64)>,
    /// Converged bus voltages, kept for the branch-flow operand Jacobian.
    vm: Vec<f64>,
    va: Vec<f64>,
}

impl<'a> AcNewton<'a> {
    /// Wrap a converged AC power flow, assembling the polar Newton Jacobian once. The layout
    /// is built from the solution's final bus typing, so a generator bus that hit its
    /// reactive limit (switched PV→PQ during the solve) is differentiated as the PQ bus it
    /// became — its released magnitude carries a sensitivity and its reactive demand enters
    /// the system, which a fixed default typing would miss.
    pub fn new(net: &'a AcNetwork, sol: &AcPfSolution) -> Self {
        let layout = AcPfLayout::with_kinds(net, &sol.kinds);
        let ybus = net.ybus();
        let (_, _, i_bus) = ac_injections(&ybus, &sol.vm, &sol.va);
        let jac = ac_jacobian(&ybus, &sol.vm, &sol.va, &i_bus, &layout);
        AcNewton {
            net,
            layout,
            jac,
            vm: sol.vm.clone(),
            va: sol.va.clone(),
        }
    }

    /// Buses carrying an angle unknown (every non-slack bus), in row order.
    fn angle_buses(&self) -> Vec<usize> {
        (0..self.net.n)
            .filter(|&b| self.layout.va_col(b).is_some())
            .collect()
    }
    /// Buses carrying a magnitude unknown (the PQ buses), in row order.
    fn magnitude_buses(&self) -> Vec<usize> {
        (0..self.net.n)
            .filter(|&b| self.layout.vm_col(b).is_some())
            .collect()
    }

    /// The reduced-system rows for the angle (`magnitude = false`) or magnitude operand,
    /// with the buses they name: angles over the non-slack buses, magnitudes over the PQ
    /// buses.
    fn voltage_rows(&self, magnitude: bool) -> Selector {
        let buses = if magnitude {
            self.magnitude_buses()
        } else {
            self.angle_buses()
        };
        let rows = buses
            .iter()
            .map(|&b| {
                if magnitude {
                    self.layout.vm_col(b).unwrap()
                } else {
                    self.layout.va_col(b).unwrap()
                }
            })
            .collect();
        Selector::new(rows, buses, 1.0)
    }

    /// The from/to complex-power partials of branch `e` w.r.t. `(va_f, vm_f, va_t,
    /// vm_t)` — the pi-model `dSbr/dV` at the converged voltages. With `S_f = V_f
    /// conj(Y_ff V_f + Y_ft V_t)` and the polar partials `∂V_k/∂va_k = j V_k`,
    /// `∂V_k/∂vm_k = V_k/vm_k`, the cross terms collapse to the forms below. Returns
    /// `([∂S_f/∂…], [∂S_t/∂…])`.
    fn flow_partials(&self, e: usize) -> ([Complex<f64>; 4], [Complex<f64>; 4]) {
        let (f, t) = (self.net.br_from[e], self.net.br_to[e]);
        let vf = Complex::from_polar(self.vm[f], self.va[f]);
        let vt = Complex::from_polar(self.vm[t], self.va[t]);
        let vhat_f = Complex::from_polar(1.0, self.va[f]);
        let vhat_t = Complex::from_polar(1.0, self.va[t]);
        // Per-branch pi-model admittance (scaled by the switching state, so an open
        // branch carries no flow) — shared with `AcNetwork::ybus`.
        let (yff, yft, ytf, ytt) = self.net.branch_admittance(e);
        let i_f = yff * vf + yft * vt;
        let i_t = ytf * vf + ytt * vt;
        let j = Complex::<f64>::i();
        let from = [
            j * vf * (yft * vt).conj(),                       // ∂S_f/∂va_f
            vhat_f * i_f.conj() + vf * (yff * vhat_f).conj(), // ∂S_f/∂vm_f
            -j * vf * (yft * vt).conj(),                      // ∂S_f/∂va_t
            vf * (yft * vhat_t).conj(),                       // ∂S_f/∂vm_t
        ];
        let to = [
            -j * vt * (ytf * vf).conj(),                      // ∂S_t/∂va_f
            vt * (ytf * vhat_f).conj(),                       // ∂S_t/∂vm_f
            j * vt * (ytf * vf).conj(),                       // ∂S_t/∂va_t
            vhat_t * i_t.conj() + vt * (ytt * vhat_t).conj(), // ∂S_t/∂vm_t
        ];
        (from, to)
    }

    /// The branch-flow operand's linear map: for each branch, the gradient of the
    /// active/reactive flow at the chosen end w.r.t. the free voltage state. The slack
    /// bus has no free row, so it drops out.
    fn flow_map(&self, power: Power, end: End) -> Selector {
        let pick = |c: Complex<f64>| match power {
            Power::Active => c.re,
            Power::Reactive => c.im,
        };
        let map = (0..self.net.m)
            .map(|e| {
                let (from, to) = self.flow_partials(e);
                let p4 = match end {
                    End::From => from,
                    End::To => to,
                };
                let (f, t) = (self.net.br_from[e], self.net.br_to[e]);
                let mut entries = Vec::new();
                for (bus, dva, dvm) in [(f, p4[0], p4[1]), (t, p4[2], p4[3])] {
                    if let Some(c) = self.layout.va_col(bus) {
                        let a = pick(dva);
                        if a != 0.0 {
                            entries.push((c, a));
                        }
                    }
                    if let Some(c) = self.layout.vm_col(bus) {
                        let m = pick(dvm);
                        if m != 0.0 {
                            entries.push((c, m));
                        }
                    }
                }
                entries
            })
            .collect();
        Selector::linear(map, (0..self.net.m).collect(), 1.0)
    }
}

impl Differentiable for AcNewton<'_> {
    fn formulation(&self) -> &'static str {
        "ac"
    }

    fn dim(&self) -> usize {
        self.layout.dim()
    }

    fn jacobian(&self) -> Vec<(usize, usize, f64)> {
        self.jac.clone()
    }

    fn parameter_len(&self, p: Parameter) -> Option<usize> {
        match p {
            // A power flow has no objective or inequalities; demand is the only
            // parameter, ranging over every bus (the slack column is structurally
            // zero, the slack bus has no balance row).
            Parameter::Demand(_) => Some(self.net.n),
            _ => None,
        }
    }

    /// `∂F/∂pd` (or `∂F/∂qd`) for the requested buses: a `+1` on the real (or reactive)
    /// power row of each free demand bus. The slack bus has no row, so its column is
    /// zero. The driver applies the leading minus of `dx/dp = −J⁻¹ ∂F/∂p`.
    fn parameter_jacobian(&self, p: Parameter, idx: &[usize]) -> Result<Mat<f64>, SensError> {
        let power = match p {
            Parameter::Demand(power) => power,
            _ => {
                return Err(SensError::InvalidInput(format!(
                    "ac does not support parameter {p:?}"
                )))
            }
        };
        let mut rhs = Mat::<f64>::zeros(self.layout.dim(), idx.len());
        for (c, &bus) in idx.iter().enumerate() {
            // Active demand enters the P-mismatch row of any non-slack bus; reactive
            // demand the Q-mismatch row of a PQ bus only (a PV bus absorbs it in its
            // free reactive output, the slack in the reference). Absent rows = zero column.
            let row = match power {
                Power::Active => self.layout.p_row(bus),
                Power::Reactive => self.layout.q_row(bus),
            };
            if let Some(r) = row {
                rhs[(r, c)] = 1.0;
            }
        }
        Ok(rhs)
    }

    fn operand_len(&self, o: Operand) -> Option<usize> {
        match o {
            Operand::Voltage(VoltageKind::Magnitude) => Some(self.magnitude_buses().len()),
            Operand::Voltage(VoltageKind::Angle) => Some(self.angle_buses().len()),
            Operand::Flow { .. } => Some(self.net.m),
            _ => None,
        }
    }

    fn operand_selector(&self, o: Operand) -> Result<Selector, SensError> {
        match o {
            Operand::Voltage(VoltageKind::Magnitude) => Ok(self.voltage_rows(true)),
            Operand::Voltage(VoltageKind::Angle) => Ok(self.voltage_rows(false)),
            Operand::Flow { power, end } => Ok(self.flow_map(power, end)),
            _ => Err(SensError::InvalidInput(format!(
                "ac does not support operand {o:?}"
            ))),
        }
    }

    fn solve_spec(&self) -> SolveSpec {
        // The converged Newton Jacobian is nonsingular: a plain LU, no regularization.
        SolveSpec::new(0.0, 0, 0.0)
    }

    fn element_id(&self, axis: Axis, index: usize) -> ElementId {
        match axis {
            Axis::Bus => ElementId::Bus(self.net.bus_ids[index]),
            Axis::Branch => ElementId::Branch(self.net.branch_ids[index]),
            Axis::Generator => ElementId::Generator(self.net.gen_ids[index]),
        }
    }

    fn unit_scale(&self, o: Operand, p: Parameter) -> f64 {
        super::served_unit_scale(o, p, self.net.base_mva)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulation::AcPolar;
    use crate::model::parse_case9_ac;
    use crate::problem::ac_pf;
    use crate::sens::{sensitivity, Mode};

    const DEMAND: Parameter = Parameter::Demand(Power::Active);
    const VM: Operand = Operand::Voltage(VoltageKind::Magnitude);
    const VA: Operand = Operand::Voltage(VoltageKind::Angle);

    fn l2(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Central finite difference of `(vm, va)` w.r.t. demand `pd` at bus `p`,
    /// re-solving the AC power flow at each perturbed point. Indexed by dense bus.
    fn fd_voltage(net: &AcNetwork, p: usize, eps: f64) -> (Vec<f64>, Vec<f64>) {
        let mut np = net.clone();
        np.pd[p] += eps;
        let mut nm = net.clone();
        nm.pd[p] -= eps;
        let sp = ac_pf(&AcPolar::new(), &np).expect("solve +eps");
        let sm = ac_pf(&AcPolar::new(), &nm).expect("solve -eps");
        let dvm = (0..net.n)
            .map(|i| (sp.vm[i] - sm.vm[i]) / (2.0 * eps))
            .collect();
        let dva = (0..net.n)
            .map(|i| (sp.va[i] - sm.va[i]) / (2.0 * eps))
            .collect();
        (dvm, dva)
    }

    #[test]
    fn voltage_sensitivity_matches_central_differences() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        let buses: Vec<usize> = (0..net.n).collect();
        let sys = AcNewton::new(&net, &sol);
        // values[o][c] = d(v_{rows[o].index}) / d(pd_{buses[c]}); rows are free buses.
        let mag = sensitivity(&sys, VM, DEMAND, Some(&buses), Mode::Forward).expect("vm sens");
        let ang = sensitivity(&sys, VA, DEMAND, Some(&buses), Mode::Forward).expect("va sens");

        // Guard against a vacuous parity check: the demand at the load buses must
        // move the angles substantially, so the relative-error path runs on real
        // sensitivities rather than skipping every near-zero column.
        let max_dva_col = (0..buses.len())
            .map(|c| {
                l2(&(0..ang.values.len())
                    .map(|o| ang.values[o][c])
                    .collect::<Vec<_>>())
            })
            .fold(0.0_f64, f64::max);
        assert!(
            max_dva_col > 0.05,
            "AC voltage sensitivities look trivial: max ||d(va)/d(pd) col|| = {max_dva_col}"
        );

        let eps = 1e-4;
        for (c, &p) in buses.iter().enumerate() {
            let (fd_vm, fd_va) = fd_voltage(&net, p, eps);
            // Per-column 2-norm relative error over the free-bus rows, the same parity
            // metric the DC engine uses. The finite difference is restricted to those
            // buses through the row metadata. A column whose analytic norm is below the
            // FD noise floor (the slack column, identically zero) only has to confirm
            // the finite difference likewise finds nothing.
            for (m, fd, what) in [(&mag, &fd_vm, "dvm"), (&ang, &fd_va, "dva")] {
                let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                let fdr: Vec<f64> = m.rows.iter().map(|r| fd[r.index]).collect();
                let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fdr[i]).collect();
                let (anorm, dnorm) = (l2(&an), l2(&diff));
                if anorm < 1e-6 {
                    assert!(
                        l2(&fdr) < 1e-5,
                        "{what} d/d(pd[{p}]): analytic ~0 but FD finds {}",
                        l2(&fdr)
                    );
                    continue;
                }
                let rel = dnorm / anorm;
                assert!(
                    rel < 1e-3,
                    "{what} d/d(pd[{p}]): rel {rel} (||a||={anorm} ||diff||={dnorm})"
                );
            }
        }
    }

    #[test]
    fn adjoint_equals_forward() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        let buses: Vec<usize> = (0..net.n).collect();
        let sys = AcNewton::new(&net, &sol);
        for op in [VM, VA] {
            let fwd = sensitivity(&sys, op, DEMAND, Some(&buses), Mode::Forward).expect("forward");
            let adj = sensitivity(&sys, op, DEMAND, Some(&buses), Mode::Adjoint).expect("adjoint");
            for o in 0..fwd.values.len() {
                for c in 0..buses.len() {
                    assert!(
                        (fwd.values[o][c] - adj.values[o][c]).abs() < 1e-10,
                        "{op:?}[{o}][{c}]: forward {} adjoint {}",
                        fwd.values[o][c],
                        adj.values[o][c]
                    );
                }
            }
        }
    }

    #[test]
    fn unsupported_ac_requests_surface_errors() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        let sys = AcNewton::new(&net, &sol);
        // A power flow has no nodal-price operand and no limit parameter.
        assert!(matches!(
            sensitivity(
                &sys,
                Operand::Price(Power::Active),
                DEMAND,
                None,
                Mode::Auto
            ),
            Err(SensError::Unsupported { .. })
        ));
        assert!(matches!(
            sensitivity(&sys, VM, Parameter::LineLimit, None, Mode::Auto),
            Err(SensError::Unsupported { .. })
        ));
    }

    // --- C5: reactive demand and branch flows -----------------------------------

    const QD: Parameter = Parameter::Demand(Power::Reactive);
    const FLOWS: [Operand; 4] = [
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        },
        Operand::Flow {
            power: Power::Reactive,
            end: End::From,
        },
        Operand::Flow {
            power: Power::Active,
            end: End::To,
        },
        Operand::Flow {
            power: Power::Reactive,
            end: End::To,
        },
    ];

    /// Per-branch `(pf, qf, pt, qt)` at the converged voltages, the pi-model branch
    /// flows the analytic operand differentiates (independent re-derivation for the FD).
    fn branch_flows(net: &AcNetwork, vm: &[f64], va: &[f64]) -> [Vec<f64>; 4] {
        let mut out = [
            vec![0.0; net.m],
            vec![0.0; net.m],
            vec![0.0; net.m],
            vec![0.0; net.m],
        ];
        // `e` is the branch index across several aligned arrays, not a slice walk.
        #[allow(clippy::needless_range_loop)]
        for e in 0..net.m {
            let (f, t) = (net.br_from[e], net.br_to[e]);
            let vf = Complex::from_polar(vm[f], va[f]);
            let vt = Complex::from_polar(vm[t], va[t]);
            let (yff, yft, ytf, ytt) = net.branch_admittance(e);
            let sf = vf * (yff * vf + yft * vt).conj();
            let st = vt * (ytf * vf + ytt * vt).conj();
            out[0][e] = sf.re;
            out[1][e] = sf.im;
            out[2][e] = st.re;
            out[3][e] = st.im;
        }
        out
    }

    fn flow_index(power: Power, end: End) -> usize {
        match (power, end) {
            (Power::Active, End::From) => 0,
            (Power::Reactive, End::From) => 1,
            (Power::Active, End::To) => 2,
            (Power::Reactive, End::To) => 3,
        }
    }

    /// Central finite difference of `(vm, va)` w.r.t. reactive demand `qd` at bus `p`.
    fn fd_voltage_qd(net: &AcNetwork, p: usize, eps: f64) -> (Vec<f64>, Vec<f64>) {
        let mut np = net.clone();
        np.qd[p] += eps;
        let mut nm = net.clone();
        nm.qd[p] -= eps;
        let sp = ac_pf(&AcPolar::new(), &np).expect("solve +eps");
        let sm = ac_pf(&AcPolar::new(), &nm).expect("solve -eps");
        let dvm = (0..net.n)
            .map(|i| (sp.vm[i] - sm.vm[i]) / (2.0 * eps))
            .collect();
        let dva = (0..net.n)
            .map(|i| (sp.va[i] - sm.va[i]) / (2.0 * eps))
            .collect();
        (dvm, dva)
    }

    /// Central finite difference of a branch flow quantity w.r.t. real demand at bus `p`.
    fn fd_flow(net: &AcNetwork, p: usize, eps: f64, which: usize) -> Vec<f64> {
        let mut np = net.clone();
        np.pd[p] += eps;
        let mut nm = net.clone();
        nm.pd[p] -= eps;
        let sp = ac_pf(&AcPolar::new(), &np).expect("solve +eps");
        let sm = ac_pf(&AcPolar::new(), &nm).expect("solve -eps");
        let fp = branch_flows(net, &sp.vm, &sp.va);
        let fm = branch_flows(net, &sm.vm, &sm.va);
        (0..net.m)
            .map(|e| (fp[which][e] - fm[which][e]) / (2.0 * eps))
            .collect()
    }

    #[test]
    fn reactive_demand_voltage_matches_central_differences() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        let buses: Vec<usize> = (0..net.n).collect();
        let sys = AcNewton::new(&net, &sol);
        let mag = sensitivity(&sys, VM, QD, Some(&buses), Mode::Forward).expect("vm/qd");
        let ang = sensitivity(&sys, VA, QD, Some(&buses), Mode::Forward).expect("va/qd");
        let eps = 1e-4;
        for (c, &p) in buses.iter().enumerate() {
            let (fd_vm, fd_va) = fd_voltage_qd(&net, p, eps);
            for (m, fd, what) in [(&mag, &fd_vm, "dvm"), (&ang, &fd_va, "dva")] {
                let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                let fdr: Vec<f64> = m.rows.iter().map(|r| fd[r.index]).collect();
                let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fdr[i]).collect();
                let (anorm, dnorm) = (l2(&an), l2(&diff));
                if anorm < 1e-6 {
                    assert!(
                        l2(&fdr) < 1e-5,
                        "{what} d/d(qd[{p}]): analytic ~0 but FD {}",
                        l2(&fdr)
                    );
                    continue;
                }
                assert!(
                    dnorm / anorm < 1e-3,
                    "{what} d/d(qd[{p}]): rel {} (||a||={anorm})",
                    dnorm / anorm
                );
            }
        }
    }

    #[test]
    fn branch_flow_matches_central_differences() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        let buses: Vec<usize> = (0..net.n).collect();
        let sys = AcNewton::new(&net, &sol);
        let eps = 1e-4;
        for &op in &FLOWS {
            let Operand::Flow { power, end } = op else {
                unreachable!()
            };
            let m = sensitivity(&sys, op, DEMAND, Some(&buses), Mode::Forward).expect("flow sens");
            // One row per branch, each naming its source branch id.
            assert_eq!(m.values.len(), net.m);
            for (e, r) in m.rows.iter().enumerate() {
                assert!(matches!(r.element, ElementId::Branch(id) if id == net.branch_ids[e]));
            }
            for (c, &p) in buses.iter().enumerate() {
                let fd = fd_flow(&net, p, eps, flow_index(power, end));
                let an: Vec<f64> = (0..net.m).map(|e| m.values[e][c]).collect();
                let diff: Vec<f64> = (0..net.m).map(|e| an[e] - fd[e]).collect();
                let (anorm, dnorm) = (l2(&an), l2(&diff));
                if anorm < 1e-6 {
                    assert!(
                        l2(&fd) < 1e-5,
                        "{op:?} d/d(pd[{p}]): analytic ~0 but FD {}",
                        l2(&fd)
                    );
                    continue;
                }
                assert!(
                    dnorm / anorm < 1e-3,
                    "{op:?} d/d(pd[{p}]): rel {} (||a||={anorm})",
                    dnorm / anorm
                );
            }
        }
    }

    #[test]
    fn reactive_and_flow_adjoint_equals_forward() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        let buses: Vec<usize> = (0..net.n).collect();
        let sys = AcNewton::new(&net, &sol);
        let mut ops = vec![VM, VA];
        ops.extend_from_slice(&FLOWS);
        for op in ops {
            for par in [DEMAND, QD] {
                let fwd = sensitivity(&sys, op, par, Some(&buses), Mode::Forward).expect("fwd");
                let adj = sensitivity(&sys, op, par, Some(&buses), Mode::Adjoint).expect("adj");
                for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                    for (a, b) in rf.iter().zip(ra.iter()) {
                        assert!(
                            (a - b).abs() < 1e-10,
                            "{op:?}/{par:?}: forward {a} adjoint {b}"
                        );
                    }
                }
            }
        }
    }
}
