//! Full AC OPF differentiable system: the [`AcOpfKkt`] wrapper over a solved nonlinear
//! AC OPF, differentiating its KKT by the implicit function theorem.
//!
//! At a local optimum the active-set KKT is the saddle-point system
//! ```text
//!   [ ∇²L   Jᵀ ] [ dx     ]     [ ∂(∇L)/∂p          ]
//!   [ J     0  ] [ dλ_act ] = − [ ∂(active cons)/∂p ]
//! ```
//! where `∇²L` is the Lagrangian Hessian, `J` stacks the gradients of the equalities and
//! the *active* inequalities and bounds, and `λ_act` are their multipliers. The Hessian
//! and the constraint Jacobian are exactly what the [`AcOpfModel`] builds for the solve
//! (so they are already verified by the objective matching the published AC reference);
//! this wrapper reuses them and adds the active-set bookkeeping. The KKT vector is
//! `z = [x, λ_eq, λ_active]`, with the nodal prices sitting in the `λ_eq` block.
//!
//! Demand enters only the power-balance equality right-hand side, so `∂K/∂pd` is a
//! selector on the balance rows — the same shape as the DC and conic engines.

use faer::Mat;
use interiors::{Lambda, NonlinearConstraint};
use sparsetools::csr::CSR;

use crate::model::AcNetwork;
use crate::problem::{AcOpfModel, AcOpfSolution};

use super::{
    Axis, Differentiable, ElementId, End, Operand, Parameter, Power, Selector, SensError,
    SolveSpec, VoltageKind,
};

/// A constraint/bound is treated as active when its multiplier exceeds this.
const ACTIVE_TOL: f64 = 1e-7;
/// Tikhonov term for the saddle-point KKT factorization.
const ACOPF_KKT_EPS: f64 = 1e-9;

/// Flatten a CSR into `(row, col, value)` triplets.
fn csr_triplets(m: &CSR<usize, f64>) -> Vec<(usize, usize, f64)> {
    let (rp, ci, v) = (m.rowptr(), m.colidx(), m.values());
    let mut out = Vec::with_capacity(m.nnz());
    for r in 0..m.rows() {
        for idx in rp[r]..rp[r + 1] {
            out.push((r, ci[idx], v[idx]));
        }
    }
    out
}

/// A solved AC OPF as a differentiable KKT system. Builds the saddle-point Jacobian once
/// at construction (Lagrangian Hessian + active-constraint gradients) and exposes the
/// operands and parameter columns the shared [`sensitivity`](super::sensitivity) driver
/// reads. Borrows the network.
#[non_exhaustive]
pub struct AcOpfKkt<'a> {
    model: AcOpfModel<'a>,
    /// KKT Jacobian `[[∇²L, Jᵀ], [J, 0]]` over `z = [x, λ_eq, λ_active]`.
    g: Vec<(usize, usize, f64)>,
    /// The primal `x` at the solution, for parameter columns that read it (the cost
    /// column carries `2·pg`).
    x: Vec<f64>,
    nvar: usize,
    dim: usize,
}

impl<'a> AcOpfKkt<'a> {
    /// Wrap a solved AC OPF, assembling the active-set KKT Jacobian. Rebuilds the model
    /// from `net`, re-derives the Lagrangian Hessian and constraint Jacobian at the
    /// solution, and stacks the equality and active-constraint gradients.
    pub fn new(net: &'a AcNetwork, sol: &AcOpfSolution) -> Result<Self, SensError> {
        let model = AcOpfModel::new(net);
        let nvar = model.lay.nvar();
        let ng = model.lay.ng();
        let nh = model.lay.nh();

        // The KKT indexes `sol.x` and the dual vectors with the layout rebuilt from `net`, so
        // a solution paired with a different network (different bus/branch/generator count)
        // would index out of range or, worse, silently linearize against the wrong layout.
        // Reject the mismatch instead of trusting the caller to pass a matched pair.
        if sol.x.len() != nvar
            || sol.eq_dual.len() != ng
            || sol.ineq_dual.len() != nh
            || sol.lin_l_dual.len() != net.m
            || sol.lin_u_dual.len() != net.m
            || sol.bnd_l_dual.len() != nvar
            || sol.bnd_u_dual.len() != nvar
        {
            return Err(SensError::Assembly(
                "AC OPF solution does not match the network layout".into(),
            ));
        }
        let x = &sol.x;

        // Reconstruct the multipliers the Lagrangian Hessian needs.
        let lambda = Lambda {
            eq_non_lin: sol.eq_dual.clone(),
            ineq_non_lin: sol.ineq_dual.clone(),
            mu_l: sol.lin_l_dual.clone(),
            mu_u: sol.lin_u_dual.clone(),
            lower: sol.bnd_l_dual.clone(),
            upper: sol.bnd_u_dual.clone(),
        };

        // Lagrangian Hessian and constraint Jacobians at the solution.
        let hess = model.hess(x, &lambda, 1.0);
        let (_h, _g, dh, dg) = model.gh(x, true);
        let dg = dg.ok_or_else(|| SensError::Assembly("AC OPF dg missing".into()))?;
        let dh = dh.ok_or_else(|| SensError::Assembly("AC OPF dh missing".into()))?;

        // Assign KKT rows: x in [0, nvar), the equality duals in [nvar, nvar+ng), then a
        // row per active inequality / bound below.
        let mut next = nvar + ng;
        let thermal_row: Vec<Option<usize>> = (0..nh)
            .map(|i| {
                (sol.ineq_dual[i].abs() > ACTIVE_TOL).then(|| {
                    let r = next;
                    next += 1;
                    r
                })
            })
            .collect();
        // Active angle-difference limits (one row per branch, either side binding).
        let angle_row: Vec<Option<usize>> = (0..net.m)
            .map(|e| {
                // Gate by the switching state, matching the gradient assembly below: an
                // open branch (sw == 0) carries no angle constraint, so a stale dual
                // must not allocate a structurally-zero KKT row and column.
                let active = net.sw[e] != 0.0
                    && (sol.lin_l_dual[e].abs() > ACTIVE_TOL
                        || sol.lin_u_dual[e].abs() > ACTIVE_TOL);
                active.then(|| {
                    let r = next;
                    next += 1;
                    r
                })
            })
            .collect();
        // Active variable bounds (lower or upper binding).
        let bound_row: Vec<Option<usize>> = (0..nvar)
            .map(|v| {
                let active =
                    sol.bnd_l_dual[v].abs() > ACTIVE_TOL || sol.bnd_u_dual[v].abs() > ACTIVE_TOL;
                active.then(|| {
                    let r = next;
                    next += 1;
                    r
                })
            })
            .collect();
        let dim = next;

        // Assemble the symmetric saddle-point Jacobian.
        let mut g: Vec<(usize, usize, f64)> = Vec::new();
        // Stationarity block: the Lagrangian Hessian.
        g.extend(csr_triplets(&hess));
        // Equality gradients: dg is (nvar × ng) = Jᵀ_eq. Place Jᵀ in the stationarity
        // columns and J in the constraint rows.
        for (var, eq, v) in csr_triplets(&dg) {
            g.push((var, nvar + eq, v));
            g.push((nvar + eq, var, v));
        }
        // Active inequality gradients: dh is (nvar × nh) = Jᵀ_ineq.
        for (var, i, v) in csr_triplets(&dh) {
            if let Some(r) = thermal_row[i] {
                g.push((var, r, v));
                g.push((r, var, v));
            }
        }
        // Active angle-difference gradients: ∂(va_f − va_t). `e` indexes `angle_row` and
        // the branch endpoint arrays together.
        #[allow(clippy::needless_range_loop)]
        for e in 0..net.m {
            if let Some(r) = angle_row[e] {
                // Gate by sw to match the model's angle constraint (open branch => 0 row).
                let sw = net.sw[e];
                let (vf, vt) = (model.lay.va(net.br_from[e]), model.lay.va(net.br_to[e]));
                g.push((vf, r, sw));
                g.push((r, vf, sw));
                g.push((vt, r, -sw));
                g.push((r, vt, -sw));
            }
        }
        // Active variable bounds: ∂(x_v) = e_v.
        for (v, row) in bound_row.iter().enumerate() {
            if let Some(r) = *row {
                g.push((v, r, 1.0));
                g.push((r, v, 1.0));
            }
        }

        Ok(AcOpfKkt {
            model,
            g,
            x: x.clone(),
            nvar,
            dim,
        })
    }

    fn net(&self) -> &AcNetwork {
        self.model.net
    }
}

impl Differentiable for AcOpfKkt<'_> {
    fn formulation(&self) -> &'static str {
        "acopf"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn jacobian(&self) -> Vec<(usize, usize, f64)> {
        self.g.clone()
    }

    fn parameter_len(&self, p: Parameter) -> Option<usize> {
        match p {
            // Demand enters the power-balance right-hand side, per bus.
            Parameter::Demand(_) => Some(self.net().n),
            // Generation cost coefficients enter the objective, per generator.
            Parameter::Cost(_) => Some(self.net().k),
            _ => None,
        }
    }

    /// The natural `+∂K/∂p` column (the driver applies the leading minus):
    /// - **demand** sits in the power-balance equality, so the column is a `+1` on that
    ///   balance constraint's KKT row;
    /// - **cost** sits in the objective gradient, so it lands on the `pg` stationarity row
    ///   (`2·pg` for the quadratic coefficient, `1` for the linear).
    fn parameter_jacobian(&self, p: Parameter, idx: &[usize]) -> Result<Mat<f64>, SensError> {
        let lay = &self.model.lay;
        let mut rhs = Mat::<f64>::zeros(self.dim, idx.len());
        match p {
            Parameter::Demand(power) => {
                for (c, &bus) in idx.iter().enumerate() {
                    let con = match power {
                        Power::Active => lay.r_pbal(bus),
                        Power::Reactive => lay.r_qbal(bus),
                    };
                    rhs[(self.nvar + con, c)] = 1.0;
                }
            }
            Parameter::Cost(term) => {
                for (c, &g) in idx.iter().enumerate() {
                    rhs[(lay.pg(g), c)] = match term {
                        super::CostTerm::Quadratic => 2.0 * self.x[lay.pg(g)],
                        super::CostTerm::Linear => 1.0,
                    };
                }
            }
            other => {
                return Err(SensError::InvalidInput(format!(
                    "acopf does not support parameter {other:?}"
                )))
            }
        }
        Ok(rhs)
    }

    fn operand_len(&self, o: Operand) -> Option<usize> {
        let net = self.net();
        match o {
            Operand::Price(_) | Operand::Voltage(VoltageKind::Magnitude | VoltageKind::Angle) => {
                Some(net.n)
            }
            Operand::Dispatch(_) => Some(net.k),
            Operand::Flow { .. } => Some(net.m),
            _ => None,
        }
    }

    fn operand_selector(&self, o: Operand) -> Result<Selector, SensError> {
        let lay = &self.model.lay;
        let net = self.net();
        let unit =
            |rows: Vec<usize>, n: usize, sign: f64| Selector::new(rows, (0..n).collect(), sign);
        Ok(match o {
            // Nodal prices are the balance equality duals (z-rows nvar + balance row). The
            // MIPS equality multiplier is already the positive marginal price (verified to
            // match the SOCWR price), so the reporting sign is +1.
            Operand::Price(Power::Active) => unit(
                (0..net.n).map(|i| self.nvar + lay.r_pbal(i)).collect(),
                net.n,
                1.0,
            ),
            Operand::Price(Power::Reactive) => unit(
                (0..net.n).map(|i| self.nvar + lay.r_qbal(i)).collect(),
                net.n,
                1.0,
            ),
            Operand::Dispatch(Power::Active) => {
                unit((0..net.k).map(|g| lay.pg(g)).collect(), net.k, 1.0)
            }
            Operand::Dispatch(Power::Reactive) => {
                unit((0..net.k).map(|g| lay.qg(g)).collect(), net.k, 1.0)
            }
            Operand::Voltage(VoltageKind::Magnitude) => {
                unit((0..net.n).map(|i| lay.vm(i)).collect(), net.n, 1.0)
            }
            Operand::Voltage(VoltageKind::Angle) => {
                unit((0..net.n).map(|i| lay.va(i)).collect(), net.n, 1.0)
            }
            Operand::Flow {
                power: Power::Active,
                end: End::From,
            } => unit((0..net.m).map(|e| lay.pf(e)).collect(), net.m, 1.0),
            Operand::Flow {
                power: Power::Active,
                end: End::To,
            } => unit((0..net.m).map(|e| lay.pt(e)).collect(), net.m, 1.0),
            Operand::Flow {
                power: Power::Reactive,
                end: End::From,
            } => unit((0..net.m).map(|e| lay.qf(e)).collect(), net.m, 1.0),
            Operand::Flow {
                power: Power::Reactive,
                end: End::To,
            } => unit((0..net.m).map(|e| lay.qt(e)).collect(), net.m, 1.0),
            other => {
                return Err(SensError::InvalidInput(format!(
                    "acopf does not support operand {other:?}"
                )))
            }
        })
    }

    fn solve_spec(&self) -> SolveSpec {
        SolveSpec::new(ACOPF_KKT_EPS, 12, 1e-13)
    }

    fn element_id(&self, axis: Axis, index: usize) -> ElementId {
        let net = self.net();
        match axis {
            Axis::Bus => ElementId::Bus(net.bus_ids[index]),
            Axis::Branch => ElementId::Branch(net.branch_ids[index]),
            Axis::Generator => ElementId::Generator(net.gen_ids[index]),
        }
    }

    fn unit_scale(&self, o: Operand, p: Parameter) -> f64 {
        super::served_unit_scale(o, p, self.net().base_mva)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case9_ac;
    use crate::problem::acopf;
    use crate::sens::{sensitivity, CostTerm, Mode};

    const PD: Parameter = Parameter::Demand(Power::Active);
    const QD: Parameter = Parameter::Demand(Power::Reactive);
    const VM: Operand = Operand::Voltage(VoltageKind::Magnitude);
    const PG: Operand = Operand::Dispatch(Power::Active);
    const PRICE: Operand = Operand::Price(Power::Active);

    fn l2(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Read the operand vector straight off a solved AC OPF, in the engine's per-unit
    /// units and signs. The price is the raw, un-negated active-balance multiplier —
    /// already the positive marginal price — read with selector sign +1 (unlike the conic
    /// engine, which negates its balance dual).
    fn operand_vec(sol: &AcOpfSolution, op: Operand) -> Vec<f64> {
        match op {
            VM => sol.vm.clone(),
            PG => sol.pg.clone(),
            PRICE => sol.lmp.clone(),
            Operand::Voltage(VoltageKind::Angle) => sol.va.clone(),
            _ => unreachable!(),
        }
    }

    /// Central finite difference of `op` w.r.t. demand at bus `b`, re-solving the AC OPF.
    fn fd_col(net: &AcNetwork, op: Operand, par: Parameter, b: usize, eps: f64) -> Vec<f64> {
        let (mut np, mut nm) = (net.clone(), net.clone());
        match par {
            Parameter::Demand(Power::Active) => {
                np.pd[b] += eps;
                nm.pd[b] -= eps;
            }
            Parameter::Demand(Power::Reactive) => {
                np.qd[b] += eps;
                nm.qd[b] -= eps;
            }
            _ => unreachable!(),
        }
        let sp = operand_vec(&acopf(&np).expect("solve +eps"), op);
        let sm = operand_vec(&acopf(&nm).expect("solve -eps"), op);
        (0..sp.len())
            .map(|i| (sp[i] - sm[i]) / (2.0 * eps))
            .collect()
    }

    #[test]
    fn adjoint_equals_forward() {
        let net = parse_case9_ac();
        let sol = acopf(&net).expect("acopf");
        let sys = AcOpfKkt::new(&net, &sol).expect("kkt");
        let buses: Vec<usize> = (0..net.n).collect();
        for op in [PRICE, VM, PG, Operand::Voltage(VoltageKind::Angle)] {
            for par in [PD, QD] {
                let fwd = sensitivity(&sys, op, par, Some(&buses), Mode::Forward).expect("fwd");
                let adj = sensitivity(&sys, op, par, Some(&buses), Mode::Adjoint).expect("adj");
                for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                    for (a, b) in rf.iter().zip(ra.iter()) {
                        assert!(
                            (a - b).abs() < 1e-6,
                            "{op:?}/{par:?}: forward {a} adjoint {b}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn voltage_dispatch_price_match_central_differences() {
        let net = parse_case9_ac();
        let sol = acopf(&net).expect("acopf");
        let sys = AcOpfKkt::new(&net, &sol).expect("kkt");
        let buses: Vec<usize> = (0..net.n).collect();
        let eps = 1e-5;
        for op in [VM, PG, PRICE] {
            let m = sensitivity(&sys, op, PD, Some(&buses), Mode::Forward).expect("analytic");
            // Per-column 2-norm relative error vs central differences; near-zero analytic
            // columns only have to confirm the FD likewise finds nothing.
            for (c, &b) in buses.iter().enumerate() {
                let fd = fd_col(&net, op, PD, b, eps);
                let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fd[i]).collect();
                let (anorm, dnorm) = (l2(&an), l2(&diff));
                if anorm < 1e-4 {
                    assert!(
                        l2(&fd) < 1e-2,
                        "{op:?} d/d(pd[{b}]): analytic ~0 but FD {}",
                        l2(&fd)
                    );
                    continue;
                }
                assert!(
                    dnorm / anorm < 5e-3,
                    "{op:?} d/d(pd[{b}]): rel {} (||a||={anorm} ||diff||={dnorm})",
                    dnorm / anorm
                );
            }
        }
    }

    /// Central finite difference of `op` w.r.t. a cost coefficient at generator `g`.
    fn fd_cost_col(net: &AcNetwork, op: Operand, term: CostTerm, g: usize, eps: f64) -> Vec<f64> {
        let (mut np, mut nm) = (net.clone(), net.clone());
        match term {
            CostTerm::Quadratic => {
                np.cq[g] += eps;
                nm.cq[g] -= eps;
            }
            CostTerm::Linear => {
                np.cl[g] += eps;
                nm.cl[g] -= eps;
            }
        }
        let sp = operand_vec(&acopf(&np).expect("solve +eps"), op);
        let sm = operand_vec(&acopf(&nm).expect("solve -eps"), op);
        (0..sp.len())
            .map(|i| (sp[i] - sm[i]) / (2.0 * eps))
            .collect()
    }

    #[test]
    fn cost_sensitivity_matches_central_differences() {
        let net = parse_case9_ac();
        let sol = acopf(&net).expect("acopf");
        let sys = AcOpfKkt::new(&net, &sol).expect("kkt");
        let gens: Vec<usize> = (0..net.k).collect();
        let eps = 1e-2;
        for term in [CostTerm::Linear, CostTerm::Quadratic] {
            let par = Parameter::Cost(term);
            for op in [PG, PRICE] {
                let m = sensitivity(&sys, op, par, Some(&gens), Mode::Forward).expect("analytic");
                for (c, &g) in gens.iter().enumerate() {
                    let fd = fd_cost_col(&net, op, term, g, eps);
                    let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                    let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fd[i]).collect();
                    let (anorm, dnorm) = (l2(&an), l2(&diff));
                    if anorm < 1e-3 {
                        assert!(
                            l2(&fd) < 5e-2,
                            "{op:?}/{term:?} d/d(cost[{g}]): analytic ~0 but FD {}",
                            l2(&fd)
                        );
                        continue;
                    }
                    assert!(
                        dnorm / anorm < 1e-2,
                        "{op:?}/{term:?} d/d(cost[{g}]): rel {} (||a||={anorm})",
                        dnorm / anorm
                    );
                }
            }
        }
    }

    /// With the angle-difference limits clamped tight enough to bind at the optimum, the
    /// `angle_row` active-set assembly is exercised — the unperturbed case9 binds no
    /// thermal or angle limit, so that path is otherwise untested. Adjoint must equal
    /// forward, and the analytic columns must match central differences.
    #[test]
    fn active_angle_limit_matches_central_differences() {
        let mut net = parse_case9_ac();
        for e in 0..net.m {
            net.angmin[e] = net.angmin[e].max(-0.08);
            net.angmax[e] = net.angmax[e].min(0.08);
        }
        let sol = acopf(&net).expect("acopf");
        let sys = AcOpfKkt::new(&net, &sol).expect("kkt");
        // The premise of the test: at least one angle-difference limit must bind.
        let bound = (0..net.m)
            .filter(|&e| sol.lin_l_dual[e].abs() > 1e-7 || sol.lin_u_dual[e].abs() > 1e-7)
            .count();
        assert!(bound > 0, "expected an angle-difference limit to bind");

        let buses: Vec<usize> = (0..net.n).collect();
        let va = Operand::Voltage(VoltageKind::Angle);
        // Adjoint == forward exercises the assembled angle rows in both directions.
        for op in [VM, va, PG] {
            let fwd = sensitivity(&sys, op, PD, Some(&buses), Mode::Forward).expect("fwd");
            let adj = sensitivity(&sys, op, PD, Some(&buses), Mode::Adjoint).expect("adj");
            for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                for (a, b) in rf.iter().zip(ra.iter()) {
                    assert!((a - b).abs() < 1e-6, "{op:?}: forward {a} adjoint {b}");
                }
            }
        }
        // Central-difference parity. The tolerance is looser than the unbinding case: a
        // demand step can graze the active-set boundary, so the FD column is noisier.
        let eps = 1e-5;
        for op in [VM, va, PG] {
            let m = sensitivity(&sys, op, PD, Some(&buses), Mode::Forward).expect("analytic");
            for (c, &b) in buses.iter().enumerate() {
                let fd = fd_col(&net, op, PD, b, eps);
                let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fd[i]).collect();
                let (anorm, dnorm) = (l2(&an), l2(&diff));
                if anorm < 1e-4 {
                    continue;
                }
                assert!(
                    dnorm / anorm < 3e-2,
                    "{op:?} d/d(pd[{b}]) with angle limit binding: rel {}",
                    dnorm / anorm
                );
            }
        }
    }
}
