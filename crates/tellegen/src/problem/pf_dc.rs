//! DC power flow: the second problem on the [`Dc`] formulation. Where the OPF
//! optimizes the injections, a power flow takes them as given and solves the network
//! physics for the angles — for DC the grounded linear system
//! `L theta = generation - fixed_withdrawal`, one sparse faer LU over
//! PowerIO's grounded Laplacian, with PowerIO's fixed nodal withdrawal
//! `p_d + g_s + p_shift - p_hvdc`. Gated
//! behind the faer-pulling `sensitivity` feature.

use powerio_matrix::calc_reference_constrained_system;

use crate::formulation::{Dc, Formulation};
#[cfg(test)]
use crate::model::csr_row;
use crate::model::DcNetwork;
use crate::solve::solve_sparse;

/// The assembled (grounded) linear system of a power flow, `A x = rhs`. Produced
/// by [`build_dc_pf`] and solved by [`crate::solve::solve_sparse`]. `triplets` are
/// the `(row, col, value)` entries of the square `dim x dim` operator (duplicates
/// summed), PowerIO's reference constrained system
/// (`calc_reference_constrained_system`). `full_of_reduced[r]` is the dense bus
/// of reduced row `r`; a reference bus has no row and angle zero.
#[non_exhaustive]
pub struct DcPfSystem {
    pub(crate) dim: usize,
    pub(crate) triplets: Vec<(usize, usize, f64)>,
    pub(crate) rhs: Vec<f64>,
    pub(crate) full_of_reduced: Vec<usize>,
}

impl DcPfSystem {
    /// Bundle the grounded operator (as summed `(row, col, value)` triplets),
    /// the right-hand side, and the reduced to dense bus map into a
    /// `dim x dim` system.
    pub fn new(
        dim: usize,
        triplets: Vec<(usize, usize, f64)>,
        rhs: Vec<f64>,
        full_of_reduced: Vec<usize>,
    ) -> Self {
        DcPfSystem {
            dim,
            triplets,
            rhs,
            full_of_reduced,
        }
    }
}

/// Primal solution of a DC power flow: the bus angles that carry the given
/// injections, the branch flows they imply, and the recovered slack power.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DcPfSolution {
    /// Bus voltage angles (radians); zero at every reference bus by the grounding.
    pub va: Vec<f64>,
    /// Branch active-power flows (per unit), `f = Bf va + flow_offset`:
    /// PowerIO's `calc_branch_flow`, the same affine flow definition the OPF
    /// uses.
    pub f: Vec<f64>,
    /// Recovered generation at each reference bus (per unit), in the order of
    /// [`DcNetwork::ref_buses`]: the slack power that closes its island's
    /// balance, `(L va + fixed_withdrawal)[ref]`. The generation passed for a
    /// reference bus is ignored, so this is computed, not echoed.
    #[cfg(test)]
    pub ref_injections: Vec<f64>,
}

impl DcPfSolution {
    /// Bundle the angles, flows, and recovered slack injections.
    pub(crate) fn new(va: Vec<f64>, f: Vec<f64>, ref_injections: Vec<f64>) -> Self {
        #[cfg(not(test))]
        let _ = ref_injections;
        DcPfSolution {
            va,
            f,
            #[cfg(test)]
            ref_injections,
        }
    }
}

/// A formulation that can assemble a power flow system — the dispatch point the
/// generic [`build_dc_pf`] calls, the power flow analogue of
/// [`OpfFormulation`](super::OpfFormulation). Not sealed.
pub trait DcPfFormulation: Formulation {
    /// Assemble the power flow system for `model` at the given bus generation.
    /// `generation[i]` is the per-unit real-power generation at dense bus `i`;
    /// the model's own demand and fixed withdrawals sit on the other side. It
    /// must have length `model.n`. The reference-bus entries are ignored — each
    /// island's slack bus absorbs whatever closes its balance.
    fn assemble_pf(&self, model: &DcNetwork, generation: &[f64]) -> DcPfSystem;
}

/// Build the power flow system for `model` at `generation` under formulation `f`.
/// Generic over the formulation, like [`build_opf`](super::build_opf); the runtime
/// `match` lives in `api`, above this. `generation` must have length `model.n`.
pub fn build_dc_pf<F: DcPfFormulation>(f: &F, model: &DcNetwork, generation: &[f64]) -> DcPfSystem {
    f.assemble_pf(model, generation)
}

impl DcPfFormulation for Dc {
    fn assemble_pf(&self, dc: &DcNetwork, generation: &[f64]) -> DcPfSystem {
        // PowerIO's reference constrained system: its Laplacian with every
        // reference row and column removed, and at each retained bus the
        // generation less PowerIO's fixed nodal withdrawal (demand, shunt,
        // phase shift, less the fixed HVDC injection). One reference per
        // island leaves every island's block nonsingular. Tellegen fixes each
        // reference angle at zero, the PowerModels convention its DC OPF
        // shares (the DC preparation states no reference angle).
        let withdrawal = dc.current_fixed_withdrawal();
        let injection: Vec<f64> = generation
            .iter()
            .zip(&withdrawal)
            .map(|(generation, withdrawal)| generation - withdrawal)
            .collect();
        let reference_va = vec![0.0; dc.ref_buses.len()];
        let system = calc_reference_constrained_system(
            &dc.laplacian,
            &dc.ref_buses,
            &reference_va,
            &injection,
        );
        let triplets = system
            .matrix
            .iter()
            .map(|(&value, (row, col))| (row, col, value))
            .collect();
        DcPfSystem::new(
            system.matrix.rows(),
            triplets,
            system.rhs,
            system.retained_rows,
        )
    }
}

/// Read the solved angles back into branch flows and the recovered slack
/// generation. Flows are PowerIO's `calc_branch_flow`, the same affine
/// definition as the OPF, `f = Bf theta + flow_offset`; each slack's
/// generation is `(L theta + fixed_withdrawal)[ref]`.
fn read_dc_pf(dc: &DcNetwork, theta: &[f64]) -> DcPfSolution {
    let f = dc.branch_flow(theta);
    #[cfg(test)]
    let ref_injections = {
        let withdrawal = dc.current_fixed_withdrawal();
        dc.ref_buses
            .iter()
            .map(|&r| {
                withdrawal[r]
                    + csr_row(&dc.laplacian, r)
                        .map(|(bus, value)| value * theta[bus])
                        .sum::<f64>()
            })
            .collect()
    };
    #[cfg(not(test))]
    let ref_injections = Vec::new();
    DcPfSolution::new(theta.to_vec(), f, ref_injections)
}

/// Solve the DC power flow for `model` at the bus `generation`: build the
/// grounded system over [`Dc`], solve it with one sparse faer LU, and read back
/// the angles, flows, and slack power. `generation` must have length `model.n`.
pub fn dc_pf(model: &DcNetwork, generation: &[f64]) -> Result<DcPfSolution, String> {
    if generation.len() != model.n {
        return Err(format!(
            "generation length {} != bus count {}",
            generation.len(),
            model.n
        ));
    }
    let sys = build_dc_pf(&Dc::new(), model, generation);
    let reduced = if sys.dim == 0 {
        Vec::new()
    } else {
        solve_sparse(sys.dim, &sys.triplets, &sys.rhs)?
    };
    let mut theta = vec![0.0; model.n];
    for (&bus, value) in sys.full_of_reduced.iter().zip(reduced) {
        theta[bus] = value;
    }
    Ok(read_dc_pf(model, &theta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case3;
    use crate::problem::dc_opf;

    fn approx(a: f64, b: f64, tol: f64, what: &str) {
        assert!((a - b).abs() < tol, "{what}: expected {b}, got {a}");
    }

    /// The bus generation that leaves `net` as each bus's net injection once
    /// the bus serves its own demand.
    fn net_injection_generation(dc: &DcNetwork, net: &[f64]) -> Vec<f64> {
        net.iter()
            .zip(&dc.prep.p_d)
            .map(|(net, d)| net + d)
            .collect()
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
        // A net injection of `[0, 1, 0]` is the generation that also serves
        // each bus's own demand.
        let generation = net_injection_generation(&dc, &[0.0, 1.0, 0.0]);
        let sol = dc_pf(&dc, &generation).expect("dc power flow");
        let w = 0.1 / (0.01 * 0.01 + 0.1 * 0.1);

        approx(sol.va[0], 0.0, 1e-12, "va[ref]");
        approx(sol.va[1], 2.0 / (3.0 * w), 1e-9, "va[1]");
        approx(sol.va[2], 1.0 / (3.0 * w), 1e-9, "va[2]");

        approx(sol.f[0], -2.0 / 3.0, 1e-9, "f[0] (1->2)");
        approx(sol.f[1], -1.0 / 3.0, 1e-9, "f[1] (1->3)");
        approx(sol.f[2], 1.0 / 3.0, 1e-9, "f[2] (2->3)");

        approx(sol.ref_injections[0], -1.0, 1e-9, "slack injection");

        // Kirchhoff at every bus: net injection = sum of outgoing branch flows.
        let net: Vec<f64> = (0..dc.n)
            .map(|i| csr_row(&dc.incidence, i).map(|(e, a)| a * sol.f[e]).sum())
            .collect();
        approx(net[0], -1.0, 1e-9, "balance at slack");
        approx(net[1], 1.0, 1e-9, "balance at bus1");
        approx(net[2], 0.0, 1e-9, "balance at bus2");
    }

    #[test]
    fn dc_power_flow_reproduces_opf_dispatch() {
        // The OPF solves for an optimal dispatch and the angles that carry it.
        // Feeding that dispatch back as fixed injections must reproduce exactly the
        // same angles and flows, because build_dc_pf and build_opf share one B-theta
        // formulation and ground at the same reference. This ties the new problem
        // to the already-validated one.
        let dc = parse_case3();
        let opf = dc_opf(&dc).expect("dc opf");

        let mut generation = opf.psh.clone();
        for j in 0..dc.k {
            generation[dc.gen_bus[j]] += opf.pg[j];
        }

        let pf = dc_pf(&dc, &generation).expect("dc power flow");
        for i in 0..dc.n {
            approx(pf.va[i], opf.va[i], 1e-6, "va vs opf");
        }
        for e in 0..dc.m {
            approx(pf.f[e], opf.f[e], 1e-6, "flow vs opf");
        }
    }

    #[test]
    fn opening_a_branch_reroutes_the_flow() {
        // Taking the 2-3 line out of service drops it from PowerIO's
        // preparation, so the 1 pu injected at bus 2 (dense 1) returns to the
        // slack entirely on the 1-2 line and the 1-3 line feeds a dead leaf.
        let mut net = crate::model::parse_matpower(crate::model::CASE3).expect("parse case3");
        net.branches_mut()[2].in_service = false; // source row 2 is the 2-3 line
        let dc = DcNetwork::from_network(&net).expect("build without the 2-3 line");
        assert_eq!(dc.m, 2);
        let generation = net_injection_generation(&dc, &[0.0, 1.0, 0.0]);
        let sol = dc_pf(&dc, &generation).expect("dc power flow");
        approx(sol.f[0], -1.0, 1e-9, "1-2 line carries the full injection");
        approx(sol.f[1], 0.0, 1e-9, "1-3 line feeds a dead leaf");
        approx(
            sol.ref_injections[0],
            -1.0,
            1e-9,
            "slack absorbs the injection",
        );
    }

    #[test]
    fn generation_length_is_validated() {
        let dc = parse_case3();
        let err = dc_pf(&dc, &[0.0, 1.0]).expect_err("wrong-length generation must error");
        assert!(err.contains("generation length"), "unexpected error: {err}");
    }
}
