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
//!
//! @block Time integration

use std::sync::Arc;

use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::lu::{factorize_symbolic_lu, SymbolicLu};
use faer::sparse::{SparseColMat, Triplet};

use super::schur::BlockLu;
use super::system::DynSystem;

/// The sparsity pattern of a step matrix with its symbolic LU (the column
/// ordering and the structure of the factors), shared by every numeric
/// factorization of a matrix with exactly this pattern. `pivots` holds the
/// row order and the factor structure of the first numeric factorization:
/// later matrices of this pattern are refactorized in that order, values
/// only (KLU's refactorization), while the pivots stay acceptable.
struct Pattern {
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    lu: SymbolicLu<usize>,
    pivots: std::sync::Mutex<Option<Arc<Pivots>>>,
}

/// The row order and the factor structure of a pivoted factorization, in
/// the permuted index space: `L` unit lower triangular (its unit diagonal
/// not stored), `U` upper triangular with its off-diagonal rows ascending
/// and the diagonal apart.
struct Pivots {
    /// Original row of each pivot position.
    row_perm: Vec<usize>,
    /// Pivot position of each original row.
    row_perm_inv: Vec<usize>,
    /// Original column of each factor column.
    col_perm: Vec<usize>,
    l_ptr: Vec<usize>,
    l_idx: Vec<usize>,
    u_ptr: Vec<usize>,
    u_idx: Vec<usize>,
}

/// A refactorization accepts a pivot down to this fraction of the largest
/// entry below it (the threshold of KLU's refactorization check); a smaller
/// one pivots afresh.
const PIVOT_TOL: f64 = 1e-3;

/// `TELLEGEN_PIVOT_TOL` overrides [`PIVOT_TOL`] (experimental).
fn pivot_tol() -> f64 {
    static P: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *P.get_or_init(|| std::env::var("TELLEGEN_PIVOT_TOL").ok().and_then(|v| v.parse().ok()).unwrap_or(PIVOT_TOL))
}

/// The triplet layout of the last matrix assembled: the same sequence of
/// `(row, col)` adds its values into the same slots, without sorting.
#[derive(Default)]
pub(super) struct Assembly {
    keys: Vec<(usize, usize)>,
    slot: Vec<usize>,
    n: usize,
    pattern: Option<Arc<Pattern>>,
}

/// The symbolic factorizations of the last few patterns, newest first.
#[derive(Default)]
pub(super) struct Patterns {
    kept: Vec<Arc<Pattern>>,
    assembly: Assembly,
    /// Per region, the patterns of its block (experimental: [`RegionLu`]).
    sub: Vec<std::sync::Mutex<Patterns>>,
    /// Pivot every factorization afresh ([`super::RmsOptions::refactor_values`] off).
    pub(super) pivot_always: bool,
}

impl Patterns {
    const KEPT: usize = 4;

    fn of(
        &mut self,
        col_ptr: &[usize],
        row_idx: &[usize],
        sym: faer::sparse::SymbolicSparseColMatRef<'_, usize>,
    ) -> Result<Arc<Pattern>, faer::sparse::FaerError> {
        if let Some(k) = self
            .kept
            .iter()
            .position(|p| p.col_ptr == col_ptr && p.row_idx == row_idx)
        {
            let p = self.kept.remove(k);
            self.kept.insert(0, Arc::clone(&p));
            return Ok(p);
        }
        STATS[12].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = Arc::new(Pattern {
            col_ptr: col_ptr.to_vec(),
            row_idx: row_idx.to_vec(),
            lu: factorize_symbolic_lu(sym, Default::default())?,
            pivots: std::sync::Mutex::new(None),
        });
        self.kept.truncate(Self::KEPT - 1);
        self.kept.insert(0, Arc::clone(&p));
        Ok(p)
    }

    /// The matrix of `trip` in compressed columns (repeated entries add up)
    /// and its pattern. A triplet sequence seen last time reuses its slots.
    fn assemble(
        &mut self,
        n: usize,
        trip: &[Triplet<usize, usize, f64>],
    ) -> Result<(Arc<Pattern>, Vec<f64>), String> {
        let a = &mut self.assembly;
        let same = a.n == n
            && a.pattern.is_some()
            && a.keys.len() == trip.len()
            && a.keys.iter().zip(trip).all(|(&(r, c), t)| r == t.row && c == t.col);
        if same {
            let p = Arc::clone(a.pattern.as_ref().expect("checked"));
            let mut val = vec![0.0; p.row_idx.len()];
            for (t, &s) in trip.iter().zip(&a.slot) {
                val[s] += t.val;
            }
            return Ok((p, val));
        }
        let mat = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, trip)
            .map_err(|e| format!("Jacobian assembly failed: {e:?}"))?;
        let sym = mat.symbolic();
        let (col_ptr, row_idx) = (sym.col_ptr(), sym.row_idx());
        // A kept pattern that holds this one keeps its ordering and pivots: a
        // branch that opened, a unit that tripped, leave their entries as zeros.
        let holder = self.kept.iter().position(|p| contains(&p.col_ptr, &p.row_idx, col_ptr, row_idx));
        let pattern = match holder {
            Some(k) => {
                let p = self.kept.remove(k);
                self.kept.insert(0, Arc::clone(&p));
                p
            }
            None => {
                // Experimental: a pattern of a size seen before is merged with
                // that one, so the variants (finite differences that happen to be
                // zero) end up in one pattern with one ordering and one row order.
                let same = self.kept.iter().find(|p| p.col_ptr.len() == col_ptr.len()).cloned();
                match same {
                    Some(old) => {
                        let (ucp, uri) = union(&old.col_ptr, &old.row_idx, col_ptr, row_idx);
                        let usym = unsafe {
                            faer::sparse::SymbolicSparseColMatRef::new_unchecked(n, n, &ucp, None, &uri)
                        };
                        self.of(&ucp, &uri, usym)
                            .map_err(|e| format!("Jacobian factorization failed: {e:?}"))?
                    }
                    None => self
                        .of(col_ptr, row_idx, sym)
                        .map_err(|e| format!("Jacobian factorization failed: {e:?}"))?,
                }
            }
        };
        // The slot of each triplet: its row's position within its column.
        let mut slot = Vec::with_capacity(trip.len());
        for t in trip {
            let rows = &pattern.row_idx[pattern.col_ptr[t.col]..pattern.col_ptr[t.col + 1]];
            let k = rows
                .binary_search(&t.row)
                .map_err(|_| "Jacobian assembly: an entry outside its pattern".to_string())?;
            slot.push(pattern.col_ptr[t.col] + k);
        }
        let mut val = vec![0.0; pattern.row_idx.len()];
        for (t, &k) in trip.iter().zip(&slot) {
            val[k] += t.val;
        }
        let a = &mut self.assembly;
        a.keys.clear();
        a.keys.extend(trip.iter().map(|t| (t.row, t.col)));
        a.slot = slot;
        a.n = n;
        a.pattern = Some(Arc::clone(&pattern));
        Ok((pattern, val))
    }
}

/// The union of two patterns of the same size (rows ascending in each column).
fn union(cp: &[usize], ri: &[usize], cp2: &[usize], ri2: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let mut ucp = Vec::with_capacity(cp.len());
    let mut uri = Vec::with_capacity(ri.len() + ri2.len() / 16);
    ucp.push(0);
    for j in 0..cp.len() - 1 {
        let (a, b) = (&ri[cp[j]..cp[j + 1]], &ri2[cp2[j]..cp2[j + 1]]);
        let (mut i, mut k) = (0, 0);
        while i < a.len() || k < b.len() {
            let next = match (a.get(i), b.get(k)) {
                (Some(&x), Some(&y)) if x == y => {
                    i += 1;
                    k += 1;
                    x
                }
                (Some(&x), Some(&y)) if x < y => {
                    i += 1;
                    x
                }
                (Some(_), Some(&y)) => {
                    k += 1;
                    y
                }
                (Some(&x), None) => {
                    i += 1;
                    x
                }
                (None, Some(&y)) => {
                    k += 1;
                    y
                }
                (None, None) => unreachable!(),
            };
            uri.push(next);
        }
        ucp.push(uri.len());
    }
    (ucp, uri)
}

/// Whether the pattern `(cp, ri)` holds every entry of `(cp2, ri2)` (same
/// size, rows ascending in each column).
fn contains(cp: &[usize], ri: &[usize], cp2: &[usize], ri2: &[usize]) -> bool {
    if cp.len() != cp2.len() || ri.len() < ri2.len() {
        return false;
    }
    for j in 0..cp.len() - 1 {
        let (a, b) = (&ri[cp[j]..cp[j + 1]], &ri2[cp2[j]..cp2[j + 1]]);
        if b.len() > a.len() {
            return false;
        }
        let mut k = 0;
        for &r in b {
            while k < a.len() && a[k] < r {
                k += 1;
            }
            if k == a.len() || a[k] != r {
                return false;
            }
        }
    }
    true
}

/// A sparse LU of a square matrix.
// @block Sparse LU
pub(super) struct SparseLu {
    pivots: Arc<Pivots>,
    l_val: Vec<f64>,
    u_val: Vec<f64>,
    u_diag: Vec<f64>,
    work: Vec<f64>,
}

impl SparseLu {
    /// Factorize the `n × n` matrix of `trip` (repeated entries add up):
    /// in the row order of the pattern's first factorization when its pivots
    /// hold, pivoting afresh otherwise.
    pub(super) fn new(
        n: usize,
        trip: &[Triplet<usize, usize, f64>],
        patterns: &mut Patterns,
    ) -> Result<Self, String> {
        let (pattern, val) = patterns.assemble(n, trip)?;
        let known = if patterns.pivot_always {
            None
        } else {
            pattern.pivots.lock().expect("pivots lock").clone()
        };
        if let Some(pv) = known {
            if let Some(lu) = Self::refactor(&pattern, &pv, &val) {
                return Ok(lu);
            }
            STATS[13].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        STATS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let lu = Self::pivoted(n, &pattern, &val)?;
        *pattern.pivots.lock().expect("pivots lock") = Some(Arc::clone(&lu.pivots));
        Ok(lu)
    }

    /// A pivoted factorization (faer's simplicial LU) and its structure.
    fn pivoted(n: usize, pattern: &Pattern, val: &[f64]) -> Result<Self, String> {
        use faer::sparse::linalg::lu::simplicial::{
            factorize_simplicial_numeric_lu, factorize_simplicial_numeric_lu_scratch, SimplicialLu,
        };
        use faer::sparse::{SparseColMatRef, SymbolicSparseColMatRef};
        let failed = |e: &dyn std::fmt::Debug| format!("Jacobian factorization failed: {e:?}");
        let a = SparseColMatRef::new(
            unsafe { SymbolicSparseColMatRef::new_unchecked(n, n, &pattern.col_ptr, None, &pattern.row_idx) },
            val,
        );
        let col_perm = pattern.lu.col_perm();
        let mut row_perm = vec![0usize; n];
        let mut row_perm_inv = vec![0usize; n];
        let mut lu = SimplicialLu::<usize, f64>::new();
        let mut mem = MemBuffer::try_new(factorize_simplicial_numeric_lu_scratch::<usize, f64>(n, n))
            .map_err(|e| failed(&e))?;
        factorize_simplicial_numeric_lu(
            &mut row_perm,
            &mut row_perm_inv,
            &mut lu,
            a,
            col_perm,
            MemStack::new(&mut mem),
        )
        .map_err(|e| failed(&e))?;
        let l = lu.l_factor_unsorted();
        let u = lu.u_factor_unsorted();
        // L: each column's first entry is its unit pivot; the rest are below.
        let mut l_ptr = Vec::with_capacity(n + 1);
        let mut l_idx = Vec::new();
        let mut l_val = Vec::new();
        l_ptr.push(0);
        for j in 0..n {
            for (&i, &v) in l.row_idx_of_col_raw(j).iter().zip(l.val_of_col(j)) {
                if i != j {
                    l_idx.push(i);
                    l_val.push(v);
                }
            }
            l_ptr.push(l_idx.len());
        }
        // U: the diagonal apart, the rest ascending (a topological order of
        // the column's updates, since L is lower triangular).
        let mut u_ptr = Vec::with_capacity(n + 1);
        let mut u_idx = Vec::new();
        let mut u_val = Vec::new();
        let mut u_diag = vec![0.0; n];
        let mut col: Vec<(usize, f64)> = Vec::new();
        u_ptr.push(0);
        for j in 0..n {
            col.clear();
            for (&i, &v) in u.row_idx_of_col_raw(j).iter().zip(u.val_of_col(j)) {
                if i == j {
                    u_diag[j] = v;
                } else {
                    col.push((i, v));
                }
            }
            col.sort_unstable_by_key(|e| e.0);
            for &(i, v) in &col {
                u_idx.push(i);
                u_val.push(v);
            }
            u_ptr.push(u_idx.len());
        }
        // A zero pivot: the matrix is singular (the simplicial LU does not say
        // so itself); the caller falls back as on any failed factorization.
        if let Some(j) = u_diag.iter().position(|d| *d == 0.0 || !d.is_finite()) {
            return Err(format!("Jacobian factorization failed: zero pivot in column {j}"));
        }
        let (fwd, _) = col_perm.arrays();
        if std::env::var("TELLEGEN_FILL").is_ok() {
            eprintln!("pivoted LU: n {n}, nnz(A) {}, nnz(L) {}, nnz(U) {} (+{} diagonal)", pattern.row_idx.len(), l_idx.len(), u_idx.len(), n);
        }
        let pivots = Arc::new(Pivots {
            row_perm,
            row_perm_inv,
            col_perm: fwd.to_vec(),
            l_ptr,
            l_idx,
            u_ptr,
            u_idx,
        });
        Ok(Self {
            pivots,
            l_val,
            u_val,
            u_diag,
            work: vec![0.0; n],
        })
    }

    /// The factors of a matrix of `pattern` in the row order `pv`, or
    /// `None` when a pivot falls below [`PIVOT_TOL`] of its column.
    fn refactor(pattern: &Pattern, pv: &Arc<Pivots>, val: &[f64]) -> Option<Self> {
        let n = pv.row_perm.len();
        let tol = pivot_tol();
        let mut x = vec![0.0; n];
        let mut l_val = vec![0.0; pv.l_idx.len()];
        let mut u_val = vec![0.0; pv.u_idx.len()];
        let mut u_diag = vec![0.0; n];
        for j in 0..n {
            let c = pv.col_perm[j];
            for p in pattern.col_ptr[c]..pattern.col_ptr[c + 1] {
                x[pv.row_perm_inv[pattern.row_idx[p]]] += val[p];
            }
            for p in pv.u_ptr[j]..pv.u_ptr[j + 1] {
                let k = pv.u_idx[p];
                let xk = x[k];
                u_val[p] = xk;
                x[k] = 0.0;
                if xk != 0.0 {
                    for q in pv.l_ptr[k]..pv.l_ptr[k + 1] {
                        x[pv.l_idx[q]] -= l_val[q] * xk;
                    }
                }
            }
            let d = x[j];
            x[j] = 0.0;
            let mut below = 0.0_f64;
            for q in pv.l_ptr[j]..pv.l_ptr[j + 1] {
                below = below.max(x[pv.l_idx[q]].abs());
            }
            if !(d.abs() > tol * below) || !d.is_finite() {
                return None;
            }
            u_diag[j] = d;
            let inv = 1.0 / d;
            for q in pv.l_ptr[j]..pv.l_ptr[j + 1] {
                let i = pv.l_idx[q];
                l_val[q] = x[i] * inv;
                x[i] = 0.0;
            }
        }
        Some(Self {
            pivots: Arc::clone(pv),
            l_val,
            u_val,
            u_diag,
            work: x,
        })
    }

    /// Solve in place.
    pub(super) fn solve(&mut self, rhs: &mut [f64]) {
        let pv = &*self.pivots;
        let n = pv.row_perm.len();
        let z = &mut self.work;
        for j in 0..n {
            z[j] = rhs[pv.row_perm[j]];
        }
        for j in 0..n {
            let zj = z[j];
            if zj != 0.0 {
                for q in pv.l_ptr[j]..pv.l_ptr[j + 1] {
                    z[pv.l_idx[q]] -= self.l_val[q] * zj;
                }
            }
        }
        for j in (0..n).rev() {
            let zj = z[j] / self.u_diag[j];
            z[j] = zj;
            if zj != 0.0 {
                for p in pv.u_ptr[j]..pv.u_ptr[j + 1] {
                    z[pv.u_idx[p]] -= self.u_val[p] * zj;
                }
            }
        }
        for j in 0..n {
            rhs[pv.col_perm[j]] = z[j];
        }
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
// @block Step matrix factorization
struct Factor {
    lu: Lu,
    /// The step the matrix was built with.
    h: f64,
    theta: f64,
    epoch: u64,
    /// Experimental: built with every device state held (the re-solve fallback).
    held: bool,
    /// The iterate the Jacobian was taken at: with `h` and `theta` it
    /// rebuilds this factorization exactly (`Stepper::restore_factor`).
    #[cfg_attr(not(feature = "rms-experimental"), allow(dead_code))]
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
        patterns.pivot_always = !sys.refactor_values;
        let jf = {
            let _t = Timer(7, crate::rms::clock::Instant::now());
            sys.jacobian(x)
        };
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
            held: false,
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
    for (i, &m) in mass.iter().enumerate().take(nx) {
        if m != 0.0 {
            trip.push(Triplet::new(i, i, m));
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

/// Factorization counters of the process (experimental): re-solves after events,
/// new step factorizations (epoch or step size changed), refreshes within Newton, pivoted LUs;
/// then steps and Newton iterations. Printed by [`lu_stats`].
pub(crate) static STATS: [std::sync::atomic::AtomicUsize; 16] = [const { std::sync::atomic::AtomicUsize::new(0) }; 16];
/// Wall time in microseconds (experimental): relay evaluations run first, waits for
/// relays beside the step, bank settling, integration steps.
pub(crate) static TIMES: [std::sync::atomic::AtomicU64; 8] = [const { std::sync::atomic::AtomicU64::new(0) }; 8];

pub(crate) fn time_add(k: usize, t0: crate::rms::clock::Instant) {
    TIMES[k].fetch_add(t0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
}

/// Adds its elapsed time to `TIMES[.0]` when dropped.
pub(crate) struct Timer(pub(crate) usize, pub(crate) crate::rms::clock::Instant);
impl Drop for Timer {
    fn drop(&mut self) {
        time_add(self.0, self.1);
    }
}

/// The counters of [`STATS`] as one line.
#[must_use]
pub fn lu_stats() -> String {
    let regions = super::regions::region_stats();
    let g = |k: usize| STATS[k].load(std::sync::atomic::Ordering::Relaxed);
    let tm = |k: usize| TIMES[k].load(std::sync::atomic::Ordering::Relaxed) as f64 * 1e-6;
    format!(
        "factorizations: reinit {}, new {}, refresh {} (line search {}, stall {}, cap {}); pivoted {} (new patterns {}, pivot fails {}); steps {}, newton iterations {}
         relays: {} evaluations first ({:.1} s), {} beside (wait {:.1} s); banks {:.1} s; steps {:.1} s
         in steps: residual {:.1} s, factorizations {:.1} s, solves {:.1} s, jacobian {:.1} s
         {}",
        g(0), g(1), g(2), g(6), g(7), g(8), g(3), g(12), g(13), g(4), g(5),
        g(10), tm(0), g(11), tm(1), tm(2), tm(3), tm(4), tm(5), tm(6), tm(7), regions
    )
}

/// Experimental: `TELLEGEN_STALL` (the residual ratio that counts as a stalled Newton
/// iteration, 0.5) and `TELLEGEN_RELAXED=1` (keep any factorization until Newton stalls).
/// Without `TELLEGEN_RELAXED`, [`super::RmsOptions::keep_factorization`] decides.
fn stall_settings() -> (f64, Option<bool>) {
    static T: std::sync::OnceLock<(f64, Option<bool>)> = std::sync::OnceLock::new();
    *T.get_or_init(|| {
        let stall = std::env::var("TELLEGEN_STALL").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5);
        let relaxed = std::env::var("TELLEGEN_RELAXED").ok().map(|v| v == "1");
        (stall, relaxed)
    })
}

/// Experimental: `TELLEGEN_KEEP_EPOCH=1` keeps factorizations across events.
/// Without it, [`super::RmsOptions::keep_factorization`] decides.
fn keep_epoch() -> Option<bool> {
    static K: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *K.get_or_init(|| std::env::var("TELLEGEN_KEEP_EPOCH").ok().map(|v| v == "1"))
}

/// The integrator of one run: the factorization it reuses, the last points
/// and its buffers.
// @block The stepper's state
#[derive(Default)]
pub(crate) struct Stepper {
    factor: Option<Factor>,
    /// Experimental: the factorization of the last re-solve after an event.
    reinit_factor: Option<Factor>,
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
    #[cfg(feature = "rms-experimental")]
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
    #[cfg(feature = "rms-experimental")]
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
    #[cfg(feature = "rms-experimental")]
    pub(crate) fn past(&self, sys: &DynSystem) -> Vec<(f64, Vec<f64>)> {
        (0..self.history.usable(sys.nx, sys.epoch))
            .map(|k| (self.history.h[k], self.history.x[k].clone()))
            .collect()
    }

    /// Start with the predictor's memory `past` ([`Self::past`]).
    #[cfg(feature = "rms-experimental")]
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
    // @block One step
    pub(crate) fn step(
        &mut self,
        sys: &DynSystem,
        x: &mut Vec<f64>,
        f: &mut Vec<f64>,
        h: f64,
        theta: f64,
    ) -> Result<usize, String> {
        let iterations = self.solve(sys, x, f, h, theta, false, true)?;
        STATS[4].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        STATS[5].fetch_add(iterations, std::sync::atomic::Ordering::Relaxed);
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
    // @block Re-solve after an event
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
    // @block Predictor
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
    // @block Newton solve of one step
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
        // @block Set up the step
        // Work buffers sized to the system, and the mass matrix (with device rows
        // held for the re-solve fallback).
        let nx = sys.nx;
        let Self {
            factor,
            reinit_factor,
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
        // @block Predict
        // Newton starts from the parabola through the last points (or an Euler
        // step), so the first correction is small.

        // The predictor needs `ẋ_n`, which a held device row does not have;
        // the re-solve after an event (`h = 0`) starts from the event point
        // itself.
        if h > 0.0 && !hold_devices {
            Self::predict(history, sys.epoch, x, xn, fn_, mass, h);
        } else {
            x.copy_from_slice(xn);
        }
        // @block Reuse a factorization
        // The factorization of an earlier step is kept while the step size, the
        // method and the system (its epoch) are the same.

        // Start from the cached factorization when it was built for this
        // step. One that was not is dropped, and so is any on an error.
        let mut lu: Option<Factor> = None;
        let mut fresh = true;
        // Experimental: with `TELLEGEN_KEEP_EPOCH=1`, a factorization stays
        // across events too (the step's, and the re-solve's for the next
        // re-solve) until Newton stalls; the residual decides convergence, so
        // the solution is the same.
        let keep = keep_epoch().unwrap_or(sys.keep_factorization);
        let relaxed = stall_settings().1.unwrap_or(sys.keep_factorization);
        if cached {
            if let Some(c) = factor.take() {
                if (c.h - h).abs() <= SAME_STEP * h.abs()
                    && c.theta == theta
                    && (c.epoch == sys.epoch || keep)
                {
                    lu = Some(c);
                    fresh = false;
                }
            }
        } else if keep && h == 0.0 {
            if let Some(c) = reinit_factor.take() {
                if c.h == 0.0 && c.theta == theta && c.held == hold_devices {
                    lu = Some(c);
                    fresh = false;
                }
            }
        }
        // @block Newton iterations
        // Up to 25 corrections of the step equations, each from the current
        // factorization, with a line search and a stopping test.
        let mut stalls = 0usize;
        let mut since_refresh = 0usize;
        let mut last_step = f64::INFINITY;
        let mut rnorm = step.residual(x, f, r);
        for iter in 0..MAX_ITERS {
            if !rnorm.is_finite() {
                return Err("the Newton iteration produced a non-finite residual".into());
            }
            if rnorm < TOL_RES_EXIT && iter > 0 {
                if cached {
                    *factor = lu;
                } else if keep && h == 0.0 {
                    *reinit_factor = lu;
                }
                return Ok(iter);
            }
            // @block Refresh the factorization
            // A reused factorization stays while it converges; a fresh one is
            // rebuilt every other iteration and every iteration from the fourth.

            // A reused factorization stays until the iteration stalls; a
            // fresh one is refreshed every other iteration, and every
            // iteration from the fourth.
            let refresh = if lu.is_none() {
                true
            } else if relaxed {
                // Experimental: a factorization, fresh or reused, stays
                // until Newton stalls; Newton's stopping test decides convergence.
                stalls >= 2 || iter >= since_refresh + 8
            } else if fresh {
                iter % 2 == 0 || iter >= 4
            } else {
                stalls >= 2 || iter >= 6
            };
            if refresh {
                let cause = if !cached {
                    0
                } else if lu.is_none() && iter == 0 {
                    1
                } else {
                    STATS[if lu.is_none() { 6 } else if stalls >= 2 { 7 } else { 8 }]
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    2
                };
                STATS[cause].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if relaxed {
                    stalls = 0;
                    since_refresh = iter;
                }
                fresh = true;
                let _t = Timer(5, crate::rms::clock::Instant::now());
                let mut fresh_lu = Factor::new(sys, mass, x, h, theta, patterns)?;
                fresh_lu.held = hold_devices;
                lu = Some(fresh_lu);
            }
            for i in 0..nx {
                dx[i] = -r[i];
            }
            {
                let _t = Timer(6, crate::rms::clock::Instant::now());
                lu.as_mut().expect("factorized above").solve(dx);
            }
            let dnorm = dx.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
            if !dnorm.is_finite() {
                return Err("the Newton step is not finite (singular Jacobian?)".into());
            }
            // @block Line search
            // Backtrack along the Newton direction until the residual drops; a
            // direction that lowers nothing refactorizes once, then fails the step.

            // A step length that lowers the residual (or reaches the
            // tolerance). None: the direction is not a descent one. From a
            // reused factorization, refactorize at this iterate and try
            // again; from a fresh one, the step fails and the caller falls
            // back (backward Euler, then halved steps).
            let found = backtrack(rnorm, |alpha| {
                for i in 0..nx {
                    trial[i] = x[i] + alpha * dx[i];
                }
                step.residual(trial, f_trial, r_trial)
            });
            let Some((alpha, trial_norm)) = found else {
                if refresh {
                    return Err(format!(
                        "the Newton step does not lower the residual {rnorm:.3e} \
                         (largest in {})",
                        largest_residual(sys, r)
                    ));
                }
                lu = None;
                fresh = true;
                stalls = 0;
                continue;
            };
            std::mem::swap(x, trial);
            std::mem::swap(f, f_trial);
            std::mem::swap(r, r_trial);
            let previous = rnorm;
            rnorm = trial_norm;
            if super::not_lt(rnorm, stall_settings().0 * previous) {
                stalls += 1;
            }
            // @block Stopping test
            // Stop when the correction, extrapolated with the observed contraction
            // rate, is below the tolerance (the test of DASSL and IDA), and keep the
            // factorization for the next step.

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
                } else if keep && h == 0.0 {
                    *reinit_factor = lu;
                }
                return Ok(iter + 1);
            }
        }
        Err(format!("Newton did not converge in {MAX_ITERS} iterations"))
    }
}

/// Backtracking along a Newton direction from a point with residual
/// `rnorm`: the first step length `α ∈ {1, 1/2, …, 1/64}` whose trial
/// residual `trial(α)` is finite and below `rnorm`, or below `TOL_RES` (a
/// residual at rounding level cannot be lowered), with that residual.
/// `None` when no step length does: a step that raises the residual is never
/// taken.
// @block Backtracking
fn backtrack(rnorm: f64, mut trial: impl FnMut(f64) -> f64) -> Option<(f64, f64)> {
    let mut alpha = 1.0;
    while alpha >= 1.0 / 64.0 {
        let tn = trial(alpha);
        if (tn.is_finite() && tn < rnorm) || tn < TOL_RES {
            return Some((alpha, tn));
        }
        alpha *= 0.5;
    }
    None
}

/// The equations of one step from `(xn, fn_)`.
// @block Step equations
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
        let _t = Timer(4, crate::rms::clock::Instant::now());
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

/// The variable with the largest residual, named (the state that stops a
/// step, for the error).
fn largest_residual(sys: &DynSystem, r: &[f64]) -> String {
    let i = (0..r.len())
        .max_by(|&a, &b| r[a].abs().total_cmp(&r[b].abs()))
        .unwrap_or(0);
    match sys.variable_names().into_iter().nth(i) {
        Some(name) if !name.is_empty() => name,
        _ => format!("variable {i}"),
    }
}

/// Take a trapezoidal step in place; on Newton failure fall back to an
/// L-stable backward-Euler step (which damps the chatter of limiters and
/// rectifiers that switch inside the step), then halve the step. On an error
/// `x` and `f` are unchanged.
// @block Step with fallbacks
pub(super) fn step_adaptive(
    sys: &DynSystem,
    x: &mut Vec<f64>,
    f: &mut Vec<f64>,
    h: f64,
    depth: usize,
    euler_steps: &mut usize,
    stepper: &mut Stepper,
) -> Result<usize, String> {
    match stepper.step(sys, x, f, h, 0.5) {
        Ok(ok) => Ok(ok),
        Err(trap) => {
            if let Ok(ok) = stepper.step(sys, x, f, h, 1.0) {
                *euler_steps += 1;
                return Ok(ok);
            }
            if depth >= 10 {
                return Err(trap);
            }
            let start = (x.clone(), f.clone());
            let halves = step_adaptive(sys, x, f, 0.5 * h, depth + 1, euler_steps, stepper)
                .and_then(|i1| {
                    step_adaptive(sys, x, f, 0.5 * h, depth + 1, euler_steps, stepper)
                        .map(|i2| i1 + i2)
                });
            if halves.is_err() {
                (*x, *f) = start;
                stepper.forget();
            }
            halves
        }
    }
}

// @block Network LU by regions
// Experimental: the network matrix in bordered block form. One end
// bus of every line between two regions (and the frame speed) forms the
// separator; each region's block is factorized and solved on its own task,
// and the separator's Schur complement, small and dense, couples them. The
// linear system is the same as with one sparse LU: Newton's iterates agree
// to rounding.

/// `f(0..n)` on the spin pool (serially on wasm).
fn par_for(n: usize, f: &(dyn Fn(usize) + Sync)) {
    #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
    super::spin_pool::SpinPool::global().run(n, f);
    #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
    (0..n).for_each(f);
}

/// The LU of the network matrix: whole, or by regions.
pub(super) enum NetLu {
    One(SparseLu),
    Regions(Box<RegionLu>),
}

impl NetLu {
    pub(super) fn new(
        size: usize,
        n: usize,
        trip: &[Triplet<usize, usize, f64>],
        patterns: &mut Patterns,
        regions: Option<&[u16]>,
    ) -> Result<Self, String> {
        match regions {
            Some(reg) if reg.len() == n && n > 0 && size >= 2 * n => {
                Ok(Self::Regions(Box::new(RegionLu::new(size, n, trip, patterns, reg)?)))
            }
            _ => Ok(Self::One(SparseLu::new(size, trip, patterns)?)),
        }
    }

    pub(super) fn solve(&mut self, rhs: &mut [f64]) {
        match self {
            Self::One(l) => l.solve(rhs),
            Self::Regions(r) => r.solve(rhs),
        }
    }
}

/// The separator and the blocks of the network matrix by regions.
pub(super) struct RegionLu {
    /// Per unknown: its block (`nb` for the separator) and its index there.
    blk: Vec<u32>,
    idx: Vec<u32>,
    nb: usize,
    blocks: Vec<std::sync::Mutex<Option<SparseLu>>>,
    sizes: Vec<usize>,
    /// The separator's unknowns.
    sep: Vec<usize>,
    /// Per block: the separator columns it couples to, and `A_rr⁻¹ B_r` for
    /// them (column after column).
    xcols: Vec<Vec<usize>>,
    x: Vec<Vec<f64>>,
    /// Per block: `C_r` as (separator row, block column, value).
    c: Vec<Vec<(u32, u32, f64)>>,
    /// The Schur complement's LU (row-major) and its pivots.
    s_lu: Vec<f64>,
    s_piv: Vec<usize>,
    /// Per block: its part of a right-hand side.
    work: Vec<std::sync::Mutex<Vec<f64>>>,
}

impl RegionLu {
    fn new(
        size: usize,
        n: usize,
        trip: &[Triplet<usize, usize, f64>],
        patterns: &mut Patterns,
        reg: &[u16],
    ) -> Result<Self, String> {
        let n2 = 2 * n;
        let nb = reg.iter().copied().max().map_or(1, |m| usize::from(m) + 1);
        // The separator: of each line between two regions, the end bus in the
        // higher region; and every unknown past the bus voltages.
        let mut sep_bus = vec![false; n];
        for t in trip {
            if t.row < n2 && t.col < n2 {
                let (a, b) = (t.row % n, t.col % n);
                if reg[a] != reg[b] {
                    sep_bus[if reg[a] > reg[b] { a } else { b }] = true;
                }
            }
        }
        let mut blk = vec![0u32; size];
        let mut idx = vec![0u32; size];
        let mut sizes = vec![0usize; nb];
        let mut sep = Vec::new();
        for u in 0..size {
            if u >= n2 || sep_bus[u % n] {
                blk[u] = nb as u32;
                idx[u] = sep.len() as u32;
                sep.push(u);
            } else {
                let r = usize::from(reg[u % n]);
                blk[u] = r as u32;
                idx[u] = sizes[r] as u32;
                sizes[r] += 1;
            }
        }
        let ns = sep.len();
        let mut trips: Vec<Vec<Triplet<usize, usize, f64>>> = vec![Vec::new(); nb];
        let mut bents: Vec<Vec<(u32, u32, f64)>> = vec![Vec::new(); nb];
        let mut c: Vec<Vec<(u32, u32, f64)>> = vec![Vec::new(); nb];
        let mut s = vec![0.0; ns * ns];
        for t in trip {
            let (br, bc) = (blk[t.row] as usize, blk[t.col] as usize);
            let (ir, ic) = (idx[t.row], idx[t.col]);
            match (br == nb, bc == nb) {
                (false, false) => {
                    if br != bc {
                        return Err("region LU: a coupling between two regions outside the separator".into());
                    }
                    trips[br].push(Triplet::new(ir as usize, ic as usize, t.val));
                }
                (false, true) => bents[br].push((ir, ic, t.val)),
                (true, false) => c[bc].push((ir, ic, t.val)),
                (true, true) => s[ir as usize * ns + ic as usize] += t.val,
            }
        }
        if patterns.sub.len() != nb {
            patterns.sub = (0..nb).map(|_| std::sync::Mutex::new(Patterns::default())).collect();
        }
        for p in &mut patterns.sub {
            p.get_mut().expect("region patterns").pivot_always = patterns.pivot_always;
        }
        // Each block: its LU and `A_rr⁻¹ B_r`.
        let blocks: Vec<std::sync::Mutex<Option<SparseLu>>> =
            (0..nb).map(|_| std::sync::Mutex::new(None)).collect();
        let xs: Vec<std::sync::Mutex<(Vec<usize>, Vec<f64>)>> =
            (0..nb).map(|_| std::sync::Mutex::new((Vec::new(), Vec::new()))).collect();
        let errors: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
        let sub = &patterns.sub;
        par_for(nb, &|r| {
            if sizes[r] == 0 {
                return;
            }
            let lu = {
                let mut pat = sub[r].lock().expect("region patterns");
                SparseLu::new(sizes[r], &trips[r], &mut pat)
            };
            let mut lu = match lu {
                Ok(lu) => lu,
                Err(e) => {
                    *errors.lock().expect("errors") = Some(e);
                    return;
                }
            };
            let mut cols: Vec<usize> = bents[r].iter().map(|e| e.1 as usize).collect();
            cols.sort_unstable();
            cols.dedup();
            let m = sizes[r];
            let mut x = vec![0.0; m * cols.len()];
            for &(row, col, v) in &bents[r] {
                let j = cols.binary_search(&(col as usize)).expect("column listed");
                x[j * m + row as usize] += v;
            }
            for j in 0..cols.len() {
                lu.solve(&mut x[j * m..(j + 1) * m]);
            }
            *blocks[r].lock().expect("block") = Some(lu);
            *xs[r].lock().expect("x") = (cols, x);
        });
        if let Some(e) = errors.into_inner().expect("errors") {
            return Err(e);
        }
        let mut xcols = Vec::with_capacity(nb);
        let mut x = Vec::with_capacity(nb);
        for m in xs {
            let (cols, v) = m.into_inner().expect("x");
            xcols.push(cols);
            x.push(v);
        }
        // S = A_SS − Σ C_r A_rr⁻¹ B_r.
        for r in 0..nb {
            let m = sizes[r];
            for &(k, l, v) in &c[r] {
                for (jj, &j) in xcols[r].iter().enumerate() {
                    s[k as usize * ns + j] -= v * x[r][jj * m + l as usize];
                }
            }
        }
        let mut s_piv = vec![0usize; ns];
        if !dense_lu(&mut s, ns, &mut s_piv) {
            return Err("Jacobian factorization failed: the separator's Schur complement is singular".into());
        }
        let work = sizes.iter().map(|&m| std::sync::Mutex::new(vec![0.0; m])).collect();
        Ok(Self {
            blk,
            idx,
            nb,
            blocks,
            sizes,
            sep,
            xcols,
            x,
            c,
            s_lu: s,
            s_piv,
            work,
        })
    }

    fn solve(&mut self, rhs: &mut [f64]) {
        let nb = self.nb;
        let ns = self.sep.len();
        for (u, v) in rhs.iter().enumerate() {
            let b = self.blk[u] as usize;
            if b < nb {
                self.work[b].get_mut().expect("work")[self.idx[u] as usize] = *v;
            }
        }
        {
            let (blocks, work) = (&self.blocks, &self.work);
            par_for(nb, &|r| {
                if let Some(lu) = blocks[r].lock().expect("block").as_mut() {
                    lu.solve(&mut work[r].lock().expect("work"));
                }
            });
        }
        let mut bs: Vec<f64> = self.sep.iter().map(|&u| rhs[u]).collect();
        for r in 0..nb {
            let y = self.work[r].get_mut().expect("work");
            for &(k, l, v) in &self.c[r] {
                bs[k as usize] -= v * y[l as usize];
            }
        }
        dense_solve(&self.s_lu, ns, &self.s_piv, &mut bs);
        {
            let (xcols, x, sizes, work) = (&self.xcols, &self.x, &self.sizes, &self.work);
            let bs_ref = &bs;
            par_for(nb, &|r| {
                let m = sizes[r];
                let mut y = work[r].lock().expect("work");
                for (jj, &j) in xcols[r].iter().enumerate() {
                    let xj = bs_ref[j];
                    if xj != 0.0 {
                        let col = &x[r][jj * m..(jj + 1) * m];
                        for l in 0..m {
                            y[l] -= col[l] * xj;
                        }
                    }
                }
            });
        }
        for (k, &u) in self.sep.iter().enumerate() {
            rhs[u] = bs[k];
        }
        for (u, v) in rhs.iter_mut().enumerate() {
            let b = self.blk[u] as usize;
            if b < nb {
                *v = self.work[b].get_mut().expect("work")[self.idx[u] as usize];
            }
        }
    }
}

/// LU with partial pivoting of the row-major `n × n` matrix `a` in place.
fn dense_lu(a: &mut [f64], n: usize, piv: &mut [usize]) -> bool {
    for k in 0..n {
        let mut p = k;
        let mut best = a[k * n + k].abs();
        for i in k + 1..n {
            let v = a[i * n + k].abs();
            if v > best {
                best = v;
                p = i;
            }
        }
        if !(best > 0.0) {
            return false;
        }
        piv[k] = p;
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
        }
        let d = a[k * n + k];
        for i in k + 1..n {
            let f = a[i * n + k] / d;
            a[i * n + k] = f;
            if f != 0.0 {
                for j in k + 1..n {
                    a[i * n + j] -= f * a[k * n + j];
                }
            }
        }
    }
    true
}

fn dense_solve(a: &[f64], n: usize, piv: &[usize], b: &mut [f64]) {
    for k in 0..n {
        b.swap(k, piv[k]);
    }
    for i in 0..n {
        let mut s = b[i];
        for j in 0..i {
            s -= a[i * n + j] * b[j];
        }
        b[i] = s;
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= a[i * n + j] * b[j];
        }
        b[i] = s / a[i * n + i];
    }
}

#[cfg(test)]
mod tests {
    use super::{backtrack, TOL_RES};

    /// A step length that lowers the residual is taken, the longest first.
    #[test]
    fn backtracking_takes_the_first_decrease() {
        let mut tried = Vec::new();
        let r = backtrack(1.0, |a| {
            tried.push(a);
            if a <= 0.25 {
                0.5
            } else {
                2.0
            }
        });
        assert_eq!(r, Some((0.25, 0.5)));
        assert_eq!(tried, [1.0, 0.5, 0.25]);
    }

    /// No step length down to 1/64 lowers the residual: no step is taken
    /// (the old search took the 1/128 step whatever its residual), and a
    /// non-finite trial counts as no decrease.
    #[test]
    fn backtracking_refuses_an_increase() {
        let mut tried = Vec::new();
        let r = backtrack(1.0, |a| {
            tried.push(a);
            if a == 0.5 {
                f64::NAN
            } else {
                1.0 + a
            }
        });
        assert_eq!(r, None);
        assert_eq!(tried.len(), 7, "1 down to 1/64: {tried:?}");
        assert_eq!(*tried.last().unwrap(), 1.0 / 64.0);
    }

    /// A residual at rounding level is accepted even when it does not
    /// decrease.
    #[test]
    fn backtracking_accepts_the_tolerance() {
        let r = backtrack(0.5 * TOL_RES, |_| 0.9 * TOL_RES);
        assert_eq!(r, Some((1.0, 0.9 * TOL_RES)));
    }
}
