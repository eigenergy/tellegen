//! Problem builders: the problem axis (`opf` and `pf`), each written once as a
//! function generic over a [`Formulation`](crate::formulation::Formulation).
//!
//! [`build_opf`] turns a [`DcNetwork`] and a formulation into an [`OpfProgram`] in
//! Clarabel's standard form; [`crate::solve::run`] solves it and a
//! formulation-specific readout maps the raw primal/dual vectors back into named
//! blocks. [`build_dc_pf`](pf_dc::build_dc_pf) is the power flow analogue. Formulations
//! assemble their programs through the shared [`ProgramBuilder`], which owns the
//! sparse `(P, q, A, b)` accumulation so each formulation writes only its own blocks.
//!
//! The DC OPF is the convex QP below — variables, objective, and constraints, with the
//! named dual on each constraint. The reported objective also includes `sum(cc)`, each
//! generator's constant (no-load) cost term, added back on at readout since a constant
//! cannot move the argmin and so is left out of the QP itself:
//!
//! ```text
//! min  sum(cq g^2 + cl g) + (tau^2/2) ||f||^2 + sum(c_shed psh)
//! s.t. G_inc g + psh - d = B theta        (nu_bal)   <- LMP
//!      f = W A theta                       (nu_flow)
//!      -fmax <= f <= fmax                  (lam_lb, lam_ub)
//!      gmin <= g <= gmax                   (rho_lb, rho_ub)
//!      0 <= psh <= max(d, 0)               (mu_lb, mu_ub)
//!      sw .* angmin <= sw .* (A theta) <= sw .* angmax  (gamma_lb, gamma_ub)
//!      theta[ref] = 0                       (eta_ref)
//! ```

use clarabel::algebra::CscMatrix;
use clarabel::solver::SupportedConeT;

use crate::formulation::Formulation;
use crate::model::DcNetwork;

#[cfg(feature = "conic")]
mod conic;
mod dc;
#[cfg(feature = "sensitivity")]
mod pf_ac;
#[cfg(feature = "sensitivity")]
mod pf_dc;

#[cfg(feature = "conic")]
pub(crate) use conic::SocWrLayout;
#[cfg(feature = "conic")]
pub use conic::{build_conic_opf, socwr_opf, ConicOpfFormulation, SocWrSolution};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use dc::dc_opf;
pub(crate) use dc::dc_opf_cancellable;
pub use dc::DcOpfSolution;
#[cfg(feature = "sensitivity")]
pub(crate) use pf_ac::{ac_injections, ac_jacobian};
#[cfg(feature = "sensitivity")]
pub use pf_ac::{ac_pf, AcPfFormulation, AcPfLayout, AcPfSolution};
#[cfg(feature = "sensitivity")]
pub use pf_dc::{build_dc_pf, dc_pf, DcPfFormulation, DcPfSolution, DcPfSystem};

/// A sparse convex program in Clarabel's standard form
/// `min 1/2 x'Px + q'x  s.t. Ax + s = b, s in K`. Produced by [`build_opf`] and
/// consumed by [`crate::solve::run`]. Formulations construct one through a
/// [`ProgramBuilder`].
#[non_exhaustive]
pub struct OpfProgram {
    pub(crate) p: CscMatrix<f64>,
    pub(crate) q: Vec<f64>,
    pub(crate) a: CscMatrix<f64>,
    pub(crate) b: Vec<f64>,
    pub(crate) cones: Vec<SupportedConeT<f64>>,
}

impl OpfProgram {
    /// Bundle the quadratic objective `(P, q)`, the constraint system `(A, b)`, and
    /// the cone partition into a program. `cones` lists the cones in row order.
    pub fn new(
        p: CscMatrix<f64>,
        q: Vec<f64>,
        a: CscMatrix<f64>,
        b: Vec<f64>,
        cones: Vec<SupportedConeT<f64>>,
    ) -> Self {
        OpfProgram { p, q, a, b, cones }
    }
}

/// Accumulator for a convex program in Clarabel's standard form. A formulation
/// scatters its objective and constraint entries by layout offset — `quad`/`lin`
/// for the objective `(P, q)`, `a`/`rhs` for the constraint system `(A, b)` — and
/// [`finish`](ProgramBuilder::finish) assembles the sparse matrices with the given
/// cone partition. Zero entries are dropped. Shared by the DC and conic OPF
/// assemblies so the triplet bookkeeping lives in one place.
pub(crate) struct ProgramBuilder {
    nvar: usize,
    ncon: usize,
    pi: Vec<usize>,
    pj: Vec<usize>,
    pv: Vec<f64>,
    q: Vec<f64>,
    ai: Vec<usize>,
    aj: Vec<usize>,
    av: Vec<f64>,
    b: Vec<f64>,
}

impl ProgramBuilder {
    /// A builder for a program with `nvar` variables and `ncon` constraint rows.
    pub(crate) fn new(nvar: usize, ncon: usize) -> Self {
        ProgramBuilder {
            nvar,
            ncon,
            pi: Vec::new(),
            pj: Vec::new(),
            pv: Vec::new(),
            q: vec![0.0; nvar],
            ai: Vec::new(),
            aj: Vec::new(),
            av: Vec::new(),
            b: vec![0.0; ncon],
        }
    }

    /// Diagonal objective-Hessian entry `P[c, c] += v` (skips zeros). Both
    /// formulations have a diagonal `P`.
    pub(crate) fn quad(&mut self, c: usize, v: f64) {
        if v != 0.0 {
            self.pi.push(c);
            self.pj.push(c);
            self.pv.push(v);
        }
    }

    /// Linear objective coefficient `q[c] += v`.
    pub(crate) fn lin(&mut self, c: usize, v: f64) {
        self.q[c] += v;
    }

    /// Constraint-matrix entry `A[r, c] += v` (skips zeros; duplicates sum at
    /// assembly).
    pub(crate) fn a(&mut self, r: usize, c: usize, v: f64) {
        if v != 0.0 {
            self.ai.push(r);
            self.aj.push(c);
            self.av.push(v);
        }
    }

    /// Set the right-hand side of constraint row `r` to `v`.
    pub(crate) fn rhs(&mut self, r: usize, v: f64) {
        self.b[r] = v;
    }

    /// Assemble the sparse `(P, A)` and bundle them with `q`, `b`, and `cones`.
    pub(crate) fn finish(self, cones: Vec<SupportedConeT<f64>>) -> OpfProgram {
        let p = CscMatrix::new_from_triplets(self.nvar, self.nvar, self.pi, self.pj, self.pv);
        let a = CscMatrix::new_from_triplets(self.ncon, self.nvar, self.ai, self.aj, self.av);
        OpfProgram::new(p, self.q, a, self.b, cones)
    }
}

/// A formulation that can assemble an optimal power flow program. The dispatch point
/// the generic [`build_opf`] calls. Not sealed.
pub trait OpfFormulation: Formulation {
    /// Assemble the OPF program (Clarabel standard form) for `model`.
    fn assemble_opf(&self, model: &DcNetwork) -> OpfProgram;
}

/// Build the OPF program for `model` under formulation `f`. Generic over the
/// formulation so each gets its own monomorphized assembly; the runtime `match`
/// (string -> concrete formulation) lives in `api`, above this.
pub fn build_opf<F: OpfFormulation>(f: &F, model: &DcNetwork) -> OpfProgram {
    f.assemble_opf(model)
}
