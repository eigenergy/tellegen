//! DC power flow: the second problem on the [`Dc`] formulation. Where the OPF
//! optimizes the injections, a power flow takes them as given and solves the network
//! physics for the angles — for DC the grounded linear system `B theta = injection`,
//! one sparse faer LU. Gated behind the faer-pulling `sensitivity` feature.

use crate::formulation::{Dc, Formulation};
use crate::model::DcNetwork;
use crate::solve::solve_sparse;

/// The assembled (grounded) linear system of a power flow, `A x = rhs`. Produced
/// by [`build_pf`] and solved by [`crate::solve::solve_sparse`]. `triplets` are
/// the `(row, col, value)` entries of the square `dim x dim` operator (duplicates
/// summed). Formulations construct one through [`Self::new`].
#[non_exhaustive]
pub struct PfSystem {
    pub(crate) dim: usize,
    pub(crate) triplets: Vec<(usize, usize, f64)>,
    pub(crate) rhs: Vec<f64>,
}

impl PfSystem {
    /// Bundle the operator (as summed `(row, col, value)` triplets) and the
    /// right-hand side into a `dim x dim` system.
    pub fn new(dim: usize, triplets: Vec<(usize, usize, f64)>, rhs: Vec<f64>) -> Self {
        PfSystem { dim, triplets, rhs }
    }
}

/// Primal solution of a DC power flow: the bus angles that carry the given
/// injections, the branch flows they imply, and the recovered slack power.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DcPfSolution {
    /// Bus voltage angles (radians); `va[ref] = 0` by the grounding.
    pub va: Vec<f64>,
    /// Branch active-power flows (per unit), `f[e] = -b[e] sw[e] (va_from - va_to)`,
    /// the same flow definition the OPF uses.
    pub f: Vec<f64>,
    /// Recovered reference-bus net injection (per unit): the slack power that
    /// closes the balance, `(B va)[ref]`. The injection passed for the reference
    /// bus is ignored, so this is computed, not echoed.
    pub ref_injection: f64,
}

impl DcPfSolution {
    /// Bundle the angles, flows, and recovered slack injection.
    pub fn new(va: Vec<f64>, f: Vec<f64>, ref_injection: f64) -> Self {
        DcPfSolution {
            va,
            f,
            ref_injection,
        }
    }
}

/// A formulation that can assemble a power flow system — the dispatch point the
/// generic [`build_pf`] calls, the power flow analogue of
/// [`OpfFormulation`](super::OpfFormulation). Not sealed.
pub trait PfFormulation: Formulation {
    /// Assemble the power flow system for `model` at the given bus injections.
    /// `injection[i]` is the net per-unit real-power injection at dense bus `i`
    /// (generation minus load); it must have length `model.n`. The reference-bus
    /// entry is ignored — the slack bus absorbs whatever closes the balance.
    fn assemble_pf(&self, model: &DcNetwork, injection: &[f64]) -> PfSystem;
}

/// Build the power flow system for `model` at `injection` under formulation `f`.
/// Generic over the formulation, like [`build_opf`](super::build_opf); the runtime
/// `match` lives in `api`, above this. `injection` must have length `model.n`.
pub fn build_pf<F: PfFormulation>(f: &F, model: &DcNetwork, injection: &[f64]) -> PfSystem {
    f.assemble_pf(model, injection)
}

impl PfFormulation for Dc {
    fn assemble_pf(&self, dc: &DcNetwork, injection: &[f64]) -> PfSystem {
        let n = dc.n;
        let r = dc.ref_bus;
        // Ground the singular susceptance Laplacian: drop the reference row and
        // column and put a 1 on the reference diagonal, so the system enforces
        // `theta[ref] = 0` and the reduced Laplacian carries the rest. `B[i,ref]`
        // multiplies `theta[ref] = 0`, so dropping the column changes nothing.
        let mut triplets: Vec<(usize, usize, f64)> = Vec::new();
        for (row, col, v) in dc.susceptance_coo() {
            if row == r || col == r {
                continue;
            }
            triplets.push((row, col, v));
        }
        triplets.push((r, r, 1.0));
        // rhs = injection, with the reference entry zeroed to match its identity row.
        let mut rhs = injection.to_vec();
        rhs[r] = 0.0;
        PfSystem::new(n, triplets, rhs)
    }
}

/// Read the solved angles back into branch flows and the recovered slack
/// injection. Flows use the same `f = -b sw (theta_from - theta_to)` definition
/// as the OPF; the slack injection is `(B theta)[ref]`.
fn read_dc_pf(dc: &DcNetwork, theta: &[f64]) -> DcPfSolution {
    let f: Vec<f64> = (0..dc.m)
        .map(|e| {
            let w = -dc.b[e] * dc.sw[e];
            w * (theta[dc.br_from[e]] - theta[dc.br_to[e]])
        })
        .collect();
    let r = dc.ref_bus;
    let mut ref_injection = 0.0;
    for (row, col, v) in dc.susceptance_coo() {
        if row == r {
            ref_injection += v * theta[col];
        }
    }
    DcPfSolution::new(theta.to_vec(), f, ref_injection)
}

/// Solve the DC power flow for `model` at `injection`: build the grounded system
/// over [`Dc`], solve it with one sparse faer LU, and read back the angles, flows,
/// and slack power. `injection` must have length `model.n`.
pub fn dc_pf(model: &DcNetwork, injection: &[f64]) -> Result<DcPfSolution, String> {
    if injection.len() != model.n {
        return Err(format!(
            "injection length {} != bus count {}",
            injection.len(),
            model.n
        ));
    }
    let sys = build_pf(&Dc::new(), model, injection);
    let theta = solve_sparse(sys.dim, &sys.triplets, &sys.rhs)?;
    Ok(read_dc_pf(model, &theta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case3;
    use crate::problem::dcopf;

    fn approx(a: f64, b: f64, tol: f64, what: &str) {
        assert!((a - b).abs() < tol, "{what}: expected {b}, got {a}");
    }

    #[test]
    fn dc_power_flow_matches_hand_computation() {
        // case3 is a triangle of three identical lines (r = 0.01, x = 0.1), with
        // bus 1 (dense 0) the slack/reference. Inject +1 pu at bus 2 (dense 1) and
        // nothing elsewhere; the slack absorbs it. The reduced 2x2 Laplacian solve
        // gives, with w = -b = x / (r^2 + x^2):
        //   va = [0, 2/(3w), 1/(3w)],  f = [-2/3, -1/3, 1/3],  slack = -1.
        // The flows are independent of w, so they pin the assembly exactly.
        let dc = parse_case3();
        let injection = vec![0.0, 1.0, 0.0];
        let sol = dc_pf(&dc, &injection).expect("dc power flow");
        let w = 0.1 / (0.01 * 0.01 + 0.1 * 0.1);

        approx(sol.va[0], 0.0, 1e-12, "va[ref]");
        approx(sol.va[1], 2.0 / (3.0 * w), 1e-9, "va[1]");
        approx(sol.va[2], 1.0 / (3.0 * w), 1e-9, "va[2]");

        approx(sol.f[0], -2.0 / 3.0, 1e-9, "f[0] (1->2)");
        approx(sol.f[1], -1.0 / 3.0, 1e-9, "f[1] (1->3)");
        approx(sol.f[2], 1.0 / 3.0, 1e-9, "f[2] (2->3)");

        approx(sol.ref_injection, -1.0, 1e-9, "slack injection");

        // Kirchhoff at every bus: net injection = sum of outgoing branch flows.
        let mut net = vec![0.0; dc.n];
        for e in 0..dc.m {
            net[dc.br_from[e]] += sol.f[e];
            net[dc.br_to[e]] -= sol.f[e];
        }
        approx(net[0], -1.0, 1e-9, "balance at slack");
        approx(net[1], 1.0, 1e-9, "balance at bus1");
        approx(net[2], 0.0, 1e-9, "balance at bus2");
    }

    #[test]
    fn dc_power_flow_reproduces_opf_dispatch() {
        // The OPF solves for an optimal dispatch and the angles that carry it.
        // Feeding that dispatch back as fixed injections must reproduce exactly the
        // same angles and flows, because build_pf and build_opf share one B-theta
        // formulation and ground at the same reference. This ties the new problem
        // to the already-validated one.
        let dc = parse_case3();
        let opf = dcopf(&dc).expect("dc opf");

        let mut injection: Vec<f64> = (0..dc.n).map(|i| opf.psh[i] - dc.demand[i]).collect();
        for j in 0..dc.k {
            injection[dc.gen_bus[j]] += opf.pg[j];
        }

        let pf = dc_pf(&dc, &injection).expect("dc power flow");
        for i in 0..dc.n {
            approx(pf.va[i], opf.va[i], 1e-6, "va vs opf");
        }
        for e in 0..dc.m {
            approx(pf.f[e], opf.f[e], 1e-6, "flow vs opf");
        }
    }

    #[test]
    fn opening_a_branch_reroutes_the_flow() {
        // Switching the 2-3 line out (sw = 0) drops it from the susceptance
        // Laplacian, so the 1 pu injected at bus 2 (dense 1) returns to the slack
        // entirely on the 1-2 line; the 1-3 line feeds a now-dead leaf and the
        // open 2-3 line carries nothing.
        let mut dc = parse_case3();
        dc.sw[2] = 0.0; // dense branch 2 is the 2-3 line
        let sol = dc_pf(&dc, &[0.0, 1.0, 0.0]).expect("dc power flow");
        approx(sol.f[0], -1.0, 1e-9, "1-2 line carries the full injection");
        approx(sol.f[1], 0.0, 1e-9, "1-3 line feeds a dead leaf");
        approx(sol.f[2], 0.0, 1e-9, "open 2-3 line carries no flow");
        approx(sol.ref_injection, -1.0, 1e-9, "slack absorbs the injection");
    }

    #[test]
    fn injection_length_is_validated() {
        let dc = parse_case3();
        let err = dc_pf(&dc, &[0.0, 1.0]).expect_err("wrong-length injection must error");
        assert!(err.contains("injection length"), "unexpected error: {err}");
    }
}
