//! Time integration of `M ẋ = f(x)` with the implicit trapezoidal rule, the
//! simultaneous-implicit scheme of production transient-stability programs.
//!
//! Each step solves, by Newton's method on a faer sparse LU,
//!
//! ```text
//! differential rows (M_ii ≠ 0):  M_ii (x_i − x_i,n) − h/2 (f_i(x) + f_i(x_n)) = 0
//! algebraic rows    (M_ii = 0):  f_i(x) = 0
//! ```
//!
//! A discrete event (branch trip, fault, load change, generator trip) is
//! applied at its exact time; the algebraic variables are then re-solved with
//! the differential states held (the same Newton system with `h = 0`), so the
//! step after the event starts from a consistent point.
//!
//! Newton is kept cheap without changing its converged solution:
//! - it starts from a predictor instead of `x_n`, so the first correction is
//!   small and a reused factorization keeps converging ([`Stepper::predict`]);
//! - a factorization is reused while the step size (to rounding), the method
//!   and the system epoch are unchanged, and its symbolic part for every
//!   matrix with a sparsity pattern seen lately;
//! - it stops when the correction, extrapolated with the observed
//!   contraction rate, is below the tolerance (the stopping test of DASSL and
//!   IDA), instead of waiting for one more correction below it;
//! - the step works in buffers kept across steps ([`Stepper`]).

use std::sync::Arc;

use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::lu::{factorize_symbolic_lu, LuRef, NumericLu, SymbolicLu};
use faer::sparse::{SparseColMat, Triplet};
use faer::{ColMut, Conj};

use super::schur::BlockLu;
use super::system::DynSystem;

/// The sparsity pattern of a step matrix with its symbolic LU (the column
/// ordering and the structure of the factors), shared by every numeric
/// factorization of a matrix with exactly this pattern.
struct Pattern {
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    lu: SymbolicLu<usize>,
}

/// The symbolic factorizations of the last few patterns, newest first. The
/// pattern follows the Jacobian's nonzeros (a finite difference that is zero
/// at one point, a limiter at its stop, is not stored), so after an event it
/// moves between a few variants. A factorization depends only on its own
/// matrix, never on what was cached: a run and its replay from a checkpoint
/// factorize alike.
#[derive(Default)]
pub(super) struct Patterns(Vec<Arc<Pattern>>);

impl Patterns {
    const KEPT: usize = 4;

    /// The symbolic factorization for a matrix with this structure.
    fn of(
        &mut self,
        sym: faer::sparse::SymbolicSparseColMatRef<'_, usize>,
    ) -> Result<Arc<Pattern>, faer::sparse::FaerError> {
        let (col_ptr, row_idx) = (sym.col_ptr(), sym.row_idx());
        if let Some(k) = self
            .0
            .iter()
            .position(|p| p.col_ptr == col_ptr && p.row_idx == row_idx)
        {
            let p = self.0.remove(k);
            self.0.insert(0, Arc::clone(&p));
            return Ok(p);
        }
        let p = Arc::new(Pattern {
            col_ptr: col_ptr.to_vec(),
            row_idx: row_idx.to_vec(),
            lu: factorize_symbolic_lu(sym, Default::default())?,
        });
        self.0.truncate(Self::KEPT - 1);
        self.0.insert(0, Arc::clone(&p));
        Ok(p)
    }
}

/// A sparse LU of a square matrix.
pub(super) struct SparseLu {
    pattern: Arc<Pattern>,
    numeric: NumericLu<usize, f64>,
    /// Scratch of a solve.
    scratch: MemBuffer,
}

impl SparseLu {
    /// Factorize the `n × n` matrix of `trip` (repeated entries add up),
    /// the symbolic part from `patterns` when the matrix has a pattern seen
    /// before.
    pub(super) fn new(
        n: usize,
        trip: &[Triplet<usize, usize, f64>],
        patterns: &mut Patterns,
    ) -> Result<Self, String> {
        let mat = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, trip)
            .map_err(|e| format!("Jacobian assembly failed: {e:?}"))?;
        let failed = |e: &dyn std::fmt::Debug| format!("Jacobian factorization failed: {e:?}");
        let pattern = patterns.of(mat.symbolic()).map_err(|e| failed(&e))?;
        let par = faer::get_global_parallelism();
        let mut numeric = NumericLu::new();
        let mut work = MemBuffer::try_new(
            pattern
                .lu
                .factorize_numeric_lu_scratch::<f64>(par, Default::default()),
        )
        .map_err(|e| failed(&e))?;
        pattern
            .lu
            .factorize_numeric_lu(
                &mut numeric,
                mat.as_ref(),
                par,
                MemStack::new(&mut work),
                Default::default(),
            )
            .map_err(|e| failed(&e))?;
        let scratch = MemBuffer::new(pattern.lu.solve_in_place_scratch::<f64>(1, par));
        Ok(Self {
            pattern,
            numeric,
            scratch,
        })
    }

    /// Solve in place.
    pub(super) fn solve(&mut self, rhs: &mut [f64]) {
        LuRef::<'_, usize, f64>::new_unchecked(&self.pattern.lu, &self.numeric)
            .solve_in_place_with_conj(
                Conj::No,
                ColMut::from_slice_mut(rhs).as_mat_mut(),
                faer::get_global_parallelism(),
                MemStack::new(&mut self.scratch),
            );
    }
}

/// How a step matrix is factorized.
enum Lu {
    /// The unit states eliminated onto their buses ([`super::schur`]).
    Blocks(Box<BlockLu>),
    /// One sparse LU over all unknowns: when a unit's own block is singular
    /// (an algebraic state that only the network determines).
    Full(Box<SparseLu>),
}

/// A Jacobian factorization kept across steps (the "very dishonest Newton" of
/// production transient-stability programs): reused while the step size, the
/// method and the system epoch are unchanged, refreshed when Newton stalls.
struct Factor {
    lu: Lu,
    /// The step the matrix was built with.
    h: f64,
    theta: f64,
    epoch: u64,
    /// The iterate the Jacobian was taken at: with `h` and `theta` it
    /// rebuilds this factorization exactly ([`Stepper::restore_factor`]).
    at: Vec<f64>,
}

impl Factor {
    /// `M − θ h ∂f/∂x` on the differential rows, `∂f/∂x` on the algebraic
    /// ones, at `x`, factorized. The factorization is a function of the
    /// matrix alone, so a run and its replay from a checkpoint factorize
    /// alike.
    fn new(
        sys: &DynSystem,
        mass: &[f64],
        x: &[f64],
        h: f64,
        theta: f64,
        patterns: &mut Patterns,
    ) -> Result<Self, String> {
        let jf = sys.jacobian(x);
        let by_blocks = if sys.factorize_by_blocks {
            BlockLu::new(sys, mass, &jf, h, theta, patterns)?
        } else {
            None
        };
        let lu = match by_blocks {
            Some(b) => Lu::Blocks(Box::new(b)),
            None => Lu::Full(Box::new(full_lu(sys.nx, mass, &jf, h, theta, patterns)?)),
        };
        Ok(Self {
            lu,
            h,
            theta,
            epoch: sys.epoch,
            at: x.to_vec(),
        })
    }

    /// Solve in place.
    fn solve(&mut self, rhs: &mut [f64]) {
        match &mut self.lu {
            Lu::Blocks(b) => b.solve(rhs),
            Lu::Full(f) => f.solve(rhs),
        }
    }
}

/// The step matrix entry of the Jacobian entry `v` in `row`.
#[inline]
pub(super) fn step_entry(mass: &[f64], row: usize, v: f64, h: f64, theta: f64) -> f64 {
    if mass[row] != 0.0 {
        -theta * h * v
    } else {
        v
    }
}

/// The sparse LU of the whole step matrix.
fn full_lu(
    nx: usize,
    mass: &[f64],
    jf: &[(usize, usize, f64)],
    h: f64,
    theta: f64,
    patterns: &mut Patterns,
) -> Result<SparseLu, String> {
    let mut trip: Vec<Triplet<usize, usize, f64>> = Vec::with_capacity(jf.len() + nx);
    for &(row, col, v) in jf {
        trip.push(Triplet::new(row, col, step_entry(mass, row, v, h, theta)));
    }
    for i in 0..nx {
        if mass[i] != 0.0 {
            trip.push(Triplet::new(i, i, mass[i]));
        }
    }
    SparseLu::new(nx, &trip, patterns)
}

/// Newton tolerances. `TOL_DX` bounds the error left in the iterate, judged
/// from the last correction and the contraction rate; `TOL_RES` is the
/// residual a step length must reach to be taken without a further decrease.
const TOL_DX: f64 = 1e-10;
const TOL_RES: f64 = 1e-10;
/// The residual below which the iteration stops without the test on the
/// correction.
const TOL_RES_EXIT: f64 = 0.1 * TOL_RES;
/// Safety factor on the error extrapolated with the contraction rate. With
/// the second-order predictor most steps end after two corrections, where the
/// rate is one observation: the factor (and the fresher of two rate
/// estimates, see [`Stepper::solve`]) keeps the run as close to the converged
/// solution as with more iterations.
const RATE_SAFETY: f64 = 0.3;
const MAX_ITERS: usize = 25;

/// Step sizes closer than this (relative) are the same step up to rounding
/// (`t_target − t` against `h`), so they share a factorization.
const SAME_STEP: f64 = 1e-9;

/// The predictor extrapolates only over earlier steps at least this long
/// relative to the present one (a much shorter one, just before an event,
/// would amplify rounding).
const MIN_PAST_STEP: f64 = 0.25;

/// The vectors of a step, kept across steps.
#[derive(Default)]
struct Work {
    x: Vec<f64>,
    f: Vec<f64>,
    r: Vec<f64>,
    trial: Vec<f64>,
    f_trial: Vec<f64>,
    r_trial: Vec<f64>,
    dx: Vec<f64>,
    /// The mass diagonal with every device state held.
    held: Vec<f64>,
}

impl Work {
    fn fit(&mut self, nx: usize) {
        for v in [
            &mut self.x,
            &mut self.f,
            &mut self.r,
            &mut self.trial,
            &mut self.f_trial,
            &mut self.r_trial,
            &mut self.dx,
        ] {
            v.resize(nx, 0.0);
        }
    }
}

/// The accepted points before the present one, newest first: what the
/// predictor extrapolates from. Emptied by an event (the algebraic states
/// jump there) and by a change of the system's epoch.
#[derive(Default)]
struct History {
    /// `x[0]` is the point one step back, `x[1]` two steps back.
    x: [Vec<f64>; 2],
    /// `h[k]` is the step from `x[k]` to the point after it.
    h: [f64; 2],
    len: usize,
    epoch: u64,
}

impl History {
    /// Take `x` (the point just left by a step `h`) as the newest point; `x`
    /// gets a spare buffer back.
    fn push(&mut self, x: &mut Vec<f64>, h: f64, epoch: u64) {
        if self.epoch != epoch {
            self.len = 0;
            self.epoch = epoch;
        }
        self.x.swap(0, 1);
        std::mem::swap(&mut self.x[0], x);
        self.h[1] = self.h[0];
        self.h[0] = h;
        self.len = (self.len + 1).min(2);
    }

    /// The points usable for a system of `n` states at `epoch`.
    fn usable(&self, n: usize, epoch: u64) -> usize {
        if self.epoch == epoch && self.x[..self.len].iter().all(|p| p.len() == n) {
            self.len
        } else {
            0
        }
    }
}

/// The integrator of one run: the factorization it reuses, the last points
/// and its buffers.
#[derive(Default)]
pub(crate) struct Stepper {
    factor: Option<Factor>,
    patterns: Patterns,
    history: History,
    work: Work,
}

impl Stepper {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The iterate, step and method of the factorization, when it is still
    /// valid for `sys` (its epoch), to rebuild it later.
    pub(crate) fn solver_state(&self, sys: &DynSystem) -> Option<(Vec<f64>, f64, f64)> {
        self.factor
            .as_ref()
            .filter(|c| c.epoch == sys.epoch)
            .map(|c| (c.at.clone(), c.h, c.theta))
    }

    /// Start from the factorization of `sys`'s Jacobian at `at` for step `h`
    /// and method `theta`, bit for bit the one built there during a run.
    ///
    /// # Errors
    /// When the assembly or the factorization fails.
    pub(crate) fn restore_factor(
        &mut self,
        sys: &DynSystem,
        at: &[f64],
        h: f64,
        theta: f64,
    ) -> Result<(), String> {
        self.factor = Some(Factor::new(
            sys,
            &sys.mass,
            at,
            h,
            theta,
            &mut self.patterns,
        )?);
        Ok(())
    }

    /// The predictor's memory at the present point of `sys`: the earlier
    /// points, newest first, each with the step that led away from it. With
    /// [`Self::solver_state`] it is what a run continues from exactly.
    pub(crate) fn past(&self, sys: &DynSystem) -> Vec<(f64, Vec<f64>)> {
        (0..self.history.usable(sys.nx, sys.epoch))
            .map(|k| (self.history.h[k], self.history.x[k].clone()))
            .collect()
    }

    /// Start with the predictor's memory `past` ([`Self::past`]).
    pub(crate) fn restore_past<'a>(
        &mut self,
        sys: &DynSystem,
        past: impl IntoIterator<Item = (f64, &'a [f64])>,
    ) {
        self.history.len = 0;
        self.history.epoch = sys.epoch;
        for (k, (h, x)) in past.into_iter().take(2).enumerate() {
            self.history.h[k] = h;
            self.history.x[k].clear();
            self.history.x[k].extend_from_slice(x);
            self.history.len = k + 1;
        }
    }

    /// Drop the predictor's memory: the present point is not the end of the
    /// last step taken (a step sequence was rolled back).
    pub(crate) fn forget(&mut self) {
        self.history.len = 0;
    }

    /// Advance `(x, f)` by one step `h` with weight `theta` (`0.5`
    /// trapezoidal, `1.0` backward Euler). Returns the Newton iterations; on
    /// an error `x` and `f` are unchanged.
    pub(crate) fn step(
        &mut self,
        sys: &DynSystem,
        x: &mut Vec<f64>,
        f: &mut Vec<f64>,
        h: f64,
        theta: f64,
    ) -> Result<usize, String> {
        let iterations = self.solve(sys, x, f, h, theta, false, true)?;
        std::mem::swap(x, &mut self.work.x);
        std::mem::swap(f, &mut self.work.f);
        // `work.x` is the point just left.
        self.history.push(&mut self.work.x, h, sys.epoch);
        Ok(iterations)
    }

    /// Re-solve the algebraic variables after an event with the differential
    /// states held. When a device's algebraic state only constrains
    /// differential states (a filterless IEEEST, for instance), that system
    /// is singular; the fallback then holds every device state and re-solves
    /// the network voltages. On an error `x` and `f` are unchanged.
    pub(crate) fn reinit(
        &mut self,
        sys: &DynSystem,
        x: &mut Vec<f64>,
        f: &mut Vec<f64>,
    ) -> Result<usize, String> {
        let iterations = match self.solve(sys, x, f, 0.0, 0.5, false, false) {
            Ok(n) => n,
            Err(_) => self.solve(sys, x, f, 0.0, 0.5, true, false)?,
        };
        std::mem::swap(x, &mut self.work.x);
        std::mem::swap(f, &mut self.work.f);
        self.history.len = 0;
        Ok(iterations)
    }

    /// The Newton start of a step `h` from `(xn, fn_)`.
    ///
    /// With two earlier points: the parabola through them and `xn`, for
    /// every state. Its error is of third order in the step, where the
    /// start below leaves one of second order in the differential states
    /// and of first order in the algebraic ones for the first Newton
    /// iteration, so a reused factorization needs about half the iterations
    /// to the same tolerance.
    ///
    /// Otherwise (at the start and for two steps after an event): the
    /// differential states move by `h ẋ_n` (with `ẋ_n = f_n / M`); algebraic
    /// states start at their last value.
    fn predict(
        history: &History,
        epoch: u64,
        x: &mut [f64],
        xn: &[f64],
        fn_: &[f64],
        mass: &[f64],
        h: f64,
    ) {
        let n = x.len();
        let (a, b) = (history.h[0], history.h[1]);
        if history.usable(n, epoch) == 2 && a >= MIN_PAST_STEP * h && b >= MIN_PAST_STEP * h {
            // Lagrange weights of the two earlier points at `t_n + h`, with
            // `xn` carrying the rest: exactly `xn` for a system at rest.
            let c1 = -h * (h + a + b) / (a * b);
            let c2 = h * (h + a) / ((a + b) * b);
            let (p1, p2) = (&history.x[0][..n], &history.x[1][..n]);
            let xn = &xn[..n];
            for i in 0..n {
                x[i] = xn[i] + c1 * (p1[i] - xn[i]) + c2 * (p2[i] - xn[i]);
            }
            return;
        }
        for i in 0..n {
            x[i] = if mass[i] != 0.0 {
                xn[i] + h * fn_[i] / mass[i]
            } else {
                xn[i]
            };
        }
    }

    /// Solve one step from `(xn, fn_)` into `work.x`, `work.f` (`h = 0`
    /// re-solves the algebraic variables only). `cached` uses and keeps the
    /// factorization.
    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        sys: &DynSystem,
        xn: &[f64],
        fn_: &[f64],
        h: f64,
        theta: f64,
        hold_devices: bool,
        cached: bool,
    ) -> Result<usize, String> {
        let nx = sys.nx;
        let Self {
            factor,
            patterns,
            history,
            work,
        } = self;
        work.fit(nx);
        let Work {
            x,
            f,
            r,
            trial,
            f_trial,
            r_trial,
            dx,
            held,
        } = work;
        let mass: &[f64] = if hold_devices {
            held.clear();
            held.extend((0..nx).map(|i| if i >= 2 * sys.n { 1.0 } else { sys.mass[i] }));
            held
        } else {
            &sys.mass
        };
        let step = Step {
            sys,
            mass,
            xn,
            fn_,
            h,
            theta,
        };
        // The predictor needs `ẋ_n`, which a held device row does not have;
        // the re-solve after an event (`h = 0`) starts from the event point
        // itself.
        if h > 0.0 && !hold_devices {
            Self::predict(history, sys.epoch, x, xn, fn_, mass, h);
        } else {
            x.copy_from_slice(xn);
        }
        // Start from the cached factorization when it was built for this
        // step. One that was not is dropped, and so is any on an error.
        let mut lu: Option<Factor> = None;
        let mut fresh = true;
        if cached {
            if let Some(c) = factor.take() {
                if (c.h - h).abs() <= SAME_STEP * h.abs()
                    && c.theta == theta
                    && c.epoch == sys.epoch
                {
                    lu = Some(c);
                    fresh = false;
                }
            }
        }
        let mut stalls = 0usize;
        let mut last_step = f64::INFINITY;
        let mut rnorm = step.residual(x, f, r);
        for iter in 0..MAX_ITERS {
            if !rnorm.is_finite() {
                return Err("the Newton iteration produced a non-finite residual".into());
            }
            if rnorm < TOL_RES_EXIT && iter > 0 {
                if cached {
                    *factor = lu;
                }
                return Ok(iter);
            }
            // A reused factorization stays until the iteration stalls; a
            // fresh one is refreshed every other iteration, and every
            // iteration from the fourth.
            let refresh = if fresh {
                lu.is_none() || iter % 2 == 0 || iter >= 4
            } else {
                stalls >= 2 || iter >= 6
            };
            if refresh {
                fresh = true;
                lu = Some(Factor::new(sys, mass, x, h, theta, patterns)?);
            }
            for i in 0..nx {
                dx[i] = -r[i];
            }
            lu.as_mut().expect("factorized above").solve(dx);
            let dnorm = dx.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
            if !dnorm.is_finite() {
                return Err("the Newton step is not finite (singular Jacobian?)".into());
            }
            // Backtracking: accept the first step length that lowers the
            // residual, or leaves it below the tolerance (a residual at
            // rounding level cannot be lowered, and searching for a step
            // length that does costs a residual each).
            let mut alpha = 1.0;
            let trial_norm = loop {
                for i in 0..nx {
                    trial[i] = x[i] + alpha * dx[i];
                }
                let tn = step.residual(trial, f_trial, r_trial);
                if (tn.is_finite() && tn < rnorm) || tn < TOL_RES || alpha < 1.0 / 64.0 {
                    break tn;
                }
                alpha *= 0.5;
            };
            std::mem::swap(x, trial);
            std::mem::swap(f, f_trial);
            std::mem::swap(r, r_trial);
            let previous = rnorm;
            rnorm = trial_norm;
            if !(rnorm < 0.5 * previous) {
                stalls += 1;
            }
            // Error left in `x`: for a contraction with rate `rho`, at most
            // `rho / (1 − rho)` times the correction just applied. The rate
            // is the larger of two observations: this correction against
            // the last one, and the residual after it against the residual
            // before. The second is one iteration fresher; after only two
            // corrections the first alone can miss a slowly converging part
            // of the error that the first correction's size hides.
            let step_norm = alpha * dnorm;
            // The first correction has no rate yet.
            let rho = (step_norm / last_step).max(rnorm / previous);
            let remaining = if last_step.is_finite() && rho < 0.9 {
                step_norm * rho / (1.0 - rho)
            } else {
                f64::INFINITY
            };
            last_step = step_norm;
            if (step_norm < TOL_DX || remaining < RATE_SAFETY * TOL_DX) && rnorm < 1e-6 {
                if cached {
                    *factor = lu;
                }
                return Ok(iter + 1);
            }
        }
        Err(format!("Newton did not converge in {MAX_ITERS} iterations"))
    }
}

/// The equations of one step from `(xn, fn_)`.
struct Step<'a> {
    sys: &'a DynSystem,
    mass: &'a [f64],
    xn: &'a [f64],
    fn_: &'a [f64],
    h: f64,
    theta: f64,
}

impl Step<'_> {
    /// `f(x)` into `f`, the step residual into `r`; returns its largest
    /// magnitude (NaN when any entry is).
    fn residual(&self, x: &[f64], f: &mut [f64], r: &mut [f64]) -> f64 {
        self.sys.residual(x, f, None);
        let n = r.len();
        let (x, f) = (&x[..n], &f[..n]);
        let (xn, fn_, mass) = (&self.xn[..n], &self.fn_[..n], &self.mass[..n]);
        let (h, theta) = (self.h, self.theta);
        let mut worst = 0.0_f64;
        let mut nan = false;
        for i in 0..n {
            let differential =
                mass[i] * (x[i] - xn[i]) - h * (theta * f[i] + (1.0 - theta) * fn_[i]);
            let v = if mass[i] != 0.0 { differential } else { f[i] };
            r[i] = v;
            let a = v.abs();
            nan |= a.is_nan();
            if a > worst {
                worst = a;
            }
        }
        if nan {
            f64::NAN
        } else {
            worst
        }
    }
}
