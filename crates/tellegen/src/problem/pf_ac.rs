//! AC power flow: the operational Newton-Raphson solve in polar coordinates with
//! standard slack / PV / PQ bus typing.
//!
//! The reference bus is the slack (`vm` and `va` fixed). A bus carrying an in-service
//! generator regulates its voltage magnitude — a PV bus (`vm` fixed at the setpoint,
//! the reactive injection free, the angle unknown). Every other bus is PQ (`vm` and
//! `va` both unknown, both injections fixed). The reduced Newton system stacks the
//! active power mismatch at every non-slack bus and the reactive power mismatch at every
//! PQ bus, over the angle unknowns (non-slack) and the magnitude unknowns (PQ):
//!   `x = [va(non-slack); vm(PQ)]`,  `r = [ΔP(non-slack); ΔQ(PQ)]`.
//! Each Newton step factorizes the reduced polar Jacobian with the faer sparse LU from
//! [`crate::solve`] and backtracks on the mismatch ∞-norm so a poor iterate cannot
//! overshoot; if a flat start does not converge it is retried from a few deterministic
//! perturbations and the best result is kept. Gated behind `sensitivity` with the faer
//! paths.
//!
//! The polar power injection at bus i is `S_i = V_i conj((Y V)_i)`, `V_i = vm_i
//! e^{j va_i}`. The Jacobian blocks come from the standard complex derivatives
//!   dS/dva = j diag(V) conj(diag(I) - Y diag(V))
//!   dS/dvm =   diag(V) conj(Y diag(V/|V|)) + conj(diag(I)) diag(V/|V|)
//! read as dP = Re, dQ = Im over the reduced rows and columns.

use num_complex::Complex;

use crate::formulation::{AcPolar, Formulation};
use crate::model::AcNetwork;
use crate::solve::solve_sparse;

/// Convergence tolerance on the bus-power mismatch ∞-norm (per unit). Loosened from
/// machine precision so a well-converged operating point is not rejected over the last
/// ULPs; the analytic sensitivities only need the converged Jacobian, not 1e-12.
const TOL: f64 = 1e-8;
/// Newton iterations per start before giving up on it.
const MAX_ITERS: usize = 50;
/// Perturbed restarts attempted after the flat start fails to converge.
const RESTARTS: usize = 4;

/// Bus role in the power flow: the reference slack, a voltage-regulating generator bus
/// (PV), or a load bus (PQ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BusKind {
    Slack,
    Pv,
    Pq,
}

/// The reduced unknown/equation layout of a polar AC power flow under slack/PV/PQ bus
/// typing. Columns are `[va(non-slack); vm(PQ)]`, rows `[ΔP(non-slack); ΔQ(PQ)]`.
/// Construct with [`AcPfLayout::new`].
#[non_exhaustive]
pub struct AcPfLayout {
    n: usize,
    /// Dense bus -> angle column (`None` at the slack).
    va_of: Vec<Option<usize>>,
    /// Dense bus -> magnitude column (`None` at slack and PV buses).
    vm_of: Vec<Option<usize>>,
    /// Dense bus -> active-mismatch row (`None` at the slack).
    p_of: Vec<Option<usize>>,
    /// Dense bus -> reactive-mismatch row (`None` at slack and PV buses).
    q_of: Vec<Option<usize>>,
    dim: usize,
}

/// The default bus typing for `net`: the reference is the slack, every bus with an
/// in-service generator is PV, the rest PQ. The starting point for the Q-limit outer
/// loop, which converts PV buses to PQ as their generators hit their reactive limits.
pub(crate) fn default_kinds(net: &AcNetwork) -> Vec<BusKind> {
    let mut kind = vec![BusKind::Pq; net.n];
    for &b in &net.gen_bus {
        kind[b] = BusKind::Pv;
    }
    kind[net.slack] = BusKind::Slack;
    kind
}

impl AcPfLayout {
    /// Build the layout for `net` under the default typing (slack / PV at generator buses /
    /// PQ elsewhere).
    pub fn new(net: &AcNetwork) -> Self {
        Self::with_kinds(net, &default_kinds(net))
    }

    /// Build the layout for `net` under an explicit bus typing. The Q-limit outer loop in
    /// [`ac_pf`] rebuilds the layout each round as it converts PV buses to PQ; the AC
    /// sensitivity ([`crate::AcNewton`]) rebuilds it from the converged solution's typing so
    /// the differentiated system matches the active constraint set.
    pub(crate) fn with_kinds(net: &AcNetwork, kind: &[BusKind]) -> Self {
        let n = net.n;
        let mut va_of = vec![None; n];
        let mut vm_of = vec![None; n];
        let mut p_of = vec![None; n];
        let mut q_of = vec![None; n];
        // Columns: angles for every non-slack bus, then magnitudes for every PQ bus.
        let mut col = 0usize;
        for (b, &kb) in kind.iter().enumerate() {
            if kb != BusKind::Slack {
                va_of[b] = Some(col);
                col += 1;
            }
        }
        for (b, &kb) in kind.iter().enumerate() {
            if kb == BusKind::Pq {
                vm_of[b] = Some(col);
                col += 1;
            }
        }
        // Rows match the columns: ΔP for every non-slack bus, then ΔQ for every PQ bus.
        let mut row = 0usize;
        for (b, &kb) in kind.iter().enumerate() {
            if kb != BusKind::Slack {
                p_of[b] = Some(row);
                row += 1;
            }
        }
        for (b, &kb) in kind.iter().enumerate() {
            if kb == BusKind::Pq {
                q_of[b] = Some(row);
                row += 1;
            }
        }
        debug_assert_eq!(col, row, "AC PF system must be square");
        AcPfLayout {
            n,
            va_of,
            vm_of,
            p_of,
            q_of,
            dim: col,
        }
    }

    /// Dimension of the reduced Newton system.
    pub fn dim(&self) -> usize {
        self.dim
    }
    /// The role of `bus`, recovered from its column map: no angle column → slack; an
    /// angle but no magnitude column → PV; both → PQ.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn kind(&self, bus: usize) -> BusKind {
        match (self.va_of[bus].is_some(), self.vm_of[bus].is_some()) {
            (false, _) => BusKind::Slack,
            (true, false) => BusKind::Pv,
            (true, true) => BusKind::Pq,
        }
    }
    /// Angle column of `bus` (`None` at the slack).
    pub(crate) fn va_col(&self, bus: usize) -> Option<usize> {
        self.va_of[bus]
    }
    /// Magnitude column of `bus` (`None` at slack and PV buses).
    pub(crate) fn vm_col(&self, bus: usize) -> Option<usize> {
        self.vm_of[bus]
    }
    /// Active-mismatch row of `bus` (`None` at the slack).
    pub(crate) fn p_row(&self, bus: usize) -> Option<usize> {
        self.p_of[bus]
    }
    /// Reactive-mismatch row of `bus` (`None` at slack and PV buses).
    pub(crate) fn q_row(&self, bus: usize) -> Option<usize> {
        self.q_of[bus]
    }

    /// Power mismatch `r = spec − calc`: `spec` is the net scheduled injection
    /// `(pg − pd) + j(qg − qd)`, `calc` the polar injection. `pg`/`qg` are the scheduled
    /// per-bus generation (the published dispatch for an operational solve; the relaxation
    /// dispatch for the OPF warm-start polish; a limit-clamped value at a bus the Q-limit
    /// loop has switched PV→PQ). Only the active mismatch enters at PV buses (their reactive
    /// injection is free); the slack has neither.
    fn residual(
        &self,
        net: &AcNetwork,
        pg: &[f64],
        qg: &[f64],
        p_calc: &[f64],
        q_calc: &[f64],
    ) -> Vec<f64> {
        let mut r = vec![0.0; self.dim];
        for b in 0..self.n {
            if let Some(pr) = self.p_of[b] {
                r[pr] = (pg[b] - net.pd[b]) - p_calc[b];
            }
            if let Some(qr) = self.q_of[b] {
                r[qr] = (qg[b] - net.qd[b]) - q_calc[b];
            }
        }
        r
    }
}

/// Polar bus power injections and the bus current `I = Y V` at state `(vm, va)`.
/// Returns `(p, q, i_bus)`: real and reactive injection per bus and the complex
/// bus current the Jacobian reuses.
pub(crate) fn ac_injections(
    ybus: &[(usize, usize, Complex<f64>)],
    vm: &[f64],
    va: &[f64],
) -> (Vec<f64>, Vec<f64>, Vec<Complex<f64>>) {
    let n = vm.len();
    let v: Vec<Complex<f64>> = (0..n).map(|i| Complex::from_polar(vm[i], va[i])).collect();
    let mut i_bus = vec![Complex::new(0.0, 0.0); n];
    for &(i, k, y) in ybus {
        i_bus[i] += y * v[k];
    }
    let mut p = vec![0.0; n];
    let mut q = vec![0.0; n];
    for i in 0..n {
        let s = v[i] * i_bus[i].conj();
        p[i] = s.re;
        q[i] = s.im;
    }
    (p, q, i_bus)
}

/// The reduced polar power flow Jacobian `∂(P, Q)/∂(va, vm)` as `(row, col, value)`
/// triplets, over the slack/PV/PQ reduced rows and columns of `layout`. Each
/// admittance entry `(i, k)` contributes the real parts to the active rows and the
/// imaginary parts to the reactive rows, dropped where the row or column is absent
/// (the slack everywhere, the magnitude column and reactive row at PV buses).
pub(crate) fn ac_jacobian(
    ybus: &[(usize, usize, Complex<f64>)],
    vm: &[f64],
    va: &[f64],
    i_bus: &[Complex<f64>],
    layout: &AcPfLayout,
) -> Vec<(usize, usize, f64)> {
    let n = vm.len();
    let v: Vec<Complex<f64>> = (0..n).map(|i| Complex::from_polar(vm[i], va[i])).collect();
    let j = Complex::<f64>::i();
    let mut t: Vec<(usize, usize, f64)> = Vec::with_capacity(4 * ybus.len());
    for &(i, k, y) in ybus {
        let (ds_dva, ds_dvm) = if i == k {
            let vnorm = Complex::from_polar(1.0, va[i]);
            let dva = j * v[i] * (i_bus[i] - y * v[i]).conj();
            let dvm = v[i] * y.conj() * vnorm.conj() + i_bus[i].conj() * vnorm;
            (dva, dvm)
        } else {
            let vnorm_k = Complex::from_polar(1.0, va[k]);
            let dva = -j * v[i] * (y * v[k]).conj();
            let dvm = v[i] * y.conj() * vnorm_k.conj();
            (dva, dvm)
        };
        // Active rows (Re) at every non-slack bus i; reactive rows (Im) at PQ buses.
        if let Some(pr) = layout.p_of[i] {
            if let Some(c) = layout.va_of[k] {
                t.push((pr, c, ds_dva.re));
            }
            if let Some(c) = layout.vm_of[k] {
                t.push((pr, c, ds_dvm.re));
            }
        }
        if let Some(qr) = layout.q_of[i] {
            if let Some(c) = layout.va_of[k] {
                t.push((qr, c, ds_dva.im));
            }
            if let Some(c) = layout.vm_of[k] {
                t.push((qr, c, ds_dvm.im));
            }
        }
    }
    t
}

/// AC power flow solution in polar coordinates: the converged bus voltage
/// magnitudes and angles, the net real and reactive injection they carry (the
/// slack and PV reactive entries are the recovered values), and the Newton iteration
/// count and final mismatch.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AcPfSolution {
    /// Bus voltage magnitudes (per unit). PV and slack buses hold their setpoints.
    pub vm: Vec<f64>,
    /// Bus voltage angles (radians); `va[slack] = 0`.
    pub va: Vec<f64>,
    /// Net real power injection per bus (per unit), `S_i = V_i conj((Y V)_i)`.
    /// At the slack bus this is the recovered slack power.
    pub p: Vec<f64>,
    /// Net reactive power injection per bus (per unit). At PV and slack buses this is
    /// the recovered reactive output.
    pub q: Vec<f64>,
    /// Newton iterations taken to converge.
    pub iterations: usize,
    /// Final infinity-norm power mismatch.
    pub residual: f64,
    /// Final bus typing after the Q-limit outer loop. A generator bus that hit its reactive
    /// limit is recorded as PQ (its voltage was released, its reactive output fixed at the
    /// limit). The AC sensitivity reads this so the differentiated system matches the active
    /// constraint set; without it a switched bus would be linearized as still regulating.
    pub(crate) kinds: Vec<BusKind>,
}

impl AcPfSolution {
    /// Bundle the converged voltages, injections, the final bus typing, and convergence
    /// diagnostics. Crate-internal: the typing carries [`BusKind`], and a solution is only
    /// produced by [`ac_pf`].
    pub(crate) fn new(
        vm: Vec<f64>,
        va: Vec<f64>,
        p: Vec<f64>,
        q: Vec<f64>,
        iterations: usize,
        residual: f64,
        kinds: Vec<BusKind>,
    ) -> Self {
        AcPfSolution {
            vm,
            va,
            p,
            q,
            iterations,
            residual,
            kinds,
        }
    }
}

/// A formulation that can drive a Newton AC power flow — the dispatch point the
/// generic [`ac_pf`] calls, the AC analogue of
/// [`DcPfFormulation`](super::DcPfFormulation). The two methods are the polar physics:
/// the bus injections and the Newton Jacobian. Not sealed.
pub trait AcPfFormulation: Formulation {
    /// Bus power injections and the bus current `Y V` at state `(vm, va)`.
    fn injections(
        &self,
        ybus: &[(usize, usize, Complex<f64>)],
        vm: &[f64],
        va: &[f64],
    ) -> (Vec<f64>, Vec<f64>, Vec<Complex<f64>>);

    /// The reduced Newton Jacobian `∂(P, Q)/∂(va, vm)` as triplets over `layout`.
    fn jacobian(
        &self,
        ybus: &[(usize, usize, Complex<f64>)],
        vm: &[f64],
        va: &[f64],
        i_bus: &[Complex<f64>],
        layout: &AcPfLayout,
    ) -> Vec<(usize, usize, f64)>;
}

impl AcPfFormulation for AcPolar {
    fn injections(
        &self,
        ybus: &[(usize, usize, Complex<f64>)],
        vm: &[f64],
        va: &[f64],
    ) -> (Vec<f64>, Vec<f64>, Vec<Complex<f64>>) {
        ac_injections(ybus, vm, va)
    }
    fn jacobian(
        &self,
        ybus: &[(usize, usize, Complex<f64>)],
        vm: &[f64],
        va: &[f64],
        i_bus: &[Complex<f64>],
        layout: &AcPfLayout,
    ) -> Vec<(usize, usize, f64)> {
        ac_jacobian(ybus, vm, va, i_bus, layout)
    }
}

fn inf_norm(v: &[f64]) -> f64 {
    // `f64::max` returns the non-NaN operand, so a plain fold would let a NaN entry
    // norm to 0.0 and read as a converged point. Make NaN propagate so the caller's
    // `residual < TOL` check fails and the iterate is rejected.
    let mut norm = 0.0_f64;
    for &x in v {
        if x.is_nan() {
            return f64::NAN;
        }
        norm = norm.max(x.abs());
    }
    norm
}

/// Apply `x ← x + α·dx` over the reduced columns, returning the trial `(vm, va)`.
fn take_step(
    vm: &[f64],
    va: &[f64],
    dx: &[f64],
    layout: &AcPfLayout,
    alpha: f64,
) -> (Vec<f64>, Vec<f64>) {
    let mut tvm = vm.to_vec();
    let mut tva = va.to_vec();
    for b in 0..layout.n {
        if let Some(c) = layout.va_of[b] {
            tva[b] += alpha * dx[c];
        }
        if let Some(c) = layout.vm_of[b] {
            tvm[b] += alpha * dx[c];
        }
    }
    (tvm, tva)
}

/// Run Newton-Raphson from a start at an explicit scheduled dispatch, backtracking on the
/// mismatch ∞-norm. Returns the best iterate reached as `(vm, va, iterations, residual)`
/// (converged when residual < [`TOL`]). `pg`/`qg` are the per-bus scheduled generation the
/// residual is measured against.
#[allow(clippy::too_many_arguments)]
fn newton<F: AcPfFormulation>(
    f: &F,
    net: &AcNetwork,
    ybus: &[(usize, usize, Complex<f64>)],
    layout: &AcPfLayout,
    pg: &[f64],
    qg: &[f64],
    mut vm: Vec<f64>,
    mut va: Vec<f64>,
) -> (Vec<f64>, Vec<f64>, usize, f64) {
    // `r` / `residual` / `i_bus` track the *current* iterate: measured once up front and
    // refreshed from the accepted trial each step, so the returned residual always reflects
    // the final stepped point (not the pre-step one) and `layout.residual` is built once per
    // point rather than twice.
    let mut iters = 0;
    let (p0, q0, mut i_bus) = f.injections(ybus, &vm, &va);
    let mut r = layout.residual(net, pg, qg, &p0, &q0);
    let mut residual = inf_norm(&r);
    for it in 0..MAX_ITERS {
        iters = it;
        if residual < TOL {
            break;
        }
        let jac = f.jacobian(ybus, &vm, &va, &i_bus, layout);
        let Ok(dx) = solve_sparse(layout.dim(), &jac, &r) else {
            break; // singular Jacobian: stop at the best iterate so far
        };
        // Backtracking line search: accept the largest step that reduces the mismatch
        // and keeps magnitudes positive.
        let mut alpha = 1.0;
        let mut accepted = false;
        for _ in 0..12 {
            let (tvm, tva) = take_step(&vm, &va, &dx, layout, alpha);
            // Reject a non-finite trial before it poisons the iterate: a near-singular
            // reduced Jacobian can hand back a NaN angle step while the magnitudes stay
            // finite and positive, so the angles need the same guard the magnitudes get.
            if tvm.iter().any(|&x| x.is_nan() || x <= 0.0) || tva.iter().any(|&x| x.is_nan()) {
                alpha *= 0.5;
                continue;
            }
            let (tp, tq, ti) = f.injections(ybus, &tvm, &tva);
            let tr = layout.residual(net, pg, qg, &tp, &tq);
            let tres = inf_norm(&tr);
            if tres < residual {
                vm = tvm;
                va = tva;
                i_bus = ti;
                r = tr;
                residual = tres;
                accepted = true;
                break;
            }
            alpha *= 0.5;
        }
        if !accepted {
            break; // no descent direction reduced the mismatch: stop
        }
    }
    (vm, va, iters, residual)
}

/// Aggregate generator reactive limits to buses and flag which buses carry a generator.
/// Returns `(qmin_bus, qmax_bus, has_gen)`; a bus with no generator has a zero range and is
/// never a voltage-regulating bus.
fn aggregate_q(net: &AcNetwork) -> (Vec<f64>, Vec<f64>, Vec<bool>) {
    let mut qmin_bus = vec![0.0; net.n];
    let mut qmax_bus = vec![0.0; net.n];
    let mut has_gen = vec![false; net.n];
    for g in 0..net.k {
        let b = net.gen_bus[g];
        qmin_bus[b] += net.qmin[g];
        qmax_bus[b] += net.qmax[g];
        has_gen[b] = true;
    }
    (qmin_bus, qmax_bus, has_gen)
}

/// The result of a Q-limited power flow solve: converged `(vm, va)`, the final bus typing,
/// the Newton iteration count of the last round, and the final mismatch ∞-norm.
type QlimSolve = (Vec<f64>, Vec<f64>, Vec<BusKind>, usize, f64);

/// Solve the AC power flow at an explicit per-bus dispatch from a start, enforcing generator
/// reactive limits by MATPOWER-style PV→PQ switching. Each outer round runs Newton under the
/// current typing, then converts any PV bus whose recovered reactive generation exceeds its
/// aggregate `[qmin, qmax]` to PQ — fixing its reactive injection at the violated limit and
/// releasing its voltage — and converts a bus back to PV when its released voltage has moved
/// past the setpoint in the direction the limit would correct. A bus may back-switch at most
/// once, so a pair of generators cannot trade limit violations every round and chatter the
/// active set forever. The reference is left regulating (its reactive output absorbs the
/// slack). Returns `(vm, va, kinds, iterations, residual)` for the best round; the caller
/// checks `residual < TOL` for convergence.
#[allow(clippy::too_many_arguments)]
fn solve_qlim<F: AcPfFormulation>(
    f: &F,
    net: &AcNetwork,
    ybus: &[(usize, usize, Complex<f64>)],
    pg_bus: &[f64],
    qg_bus: &[f64],
    qmin_bus: &[f64],
    qmax_bus: &[f64],
    has_gen: &[bool],
    vm_hold: &[f64],
    mut vm: Vec<f64>,
    mut va: Vec<f64>,
) -> QlimSolve {
    // Tolerance on a reactive-limit violation (per unit) and on the back-switch voltage test.
    const Q_TOL: f64 = 1e-6;
    const V_BACK: f64 = 1e-6;
    const MAX_OUTER: usize = 12;

    let mut kinds = default_kinds(net);
    let mut qsched = qg_bus.to_vec();
    // Per bus: 0 not converted, +1 fixed at qmax, −1 fixed at qmin.
    let mut converted = vec![0i8; net.n];
    // Whether a bus has already spent its one allowed PQ→PV back-switch. Capping reverts at
    // one breaks the PV↔PQ chatter that would otherwise let two generators trade limit
    // violations every round and never settle: a re-violating bus then stays pinned at its
    // limit instead of toggling, so the active set converges within a bounded round count.
    let mut reverted = vec![false; net.n];
    // Generator buses regulate to the hold voltage.
    for b in 0..net.n {
        if kinds[b] != BusKind::Pq {
            vm[b] = vm_hold[b];
        }
    }

    let mut result = (vm.clone(), va.clone(), kinds.clone(), 0usize, f64::INFINITY);
    // Set once the outer loop reaches a settled verdict: Newton failing under a typing, or
    // convergence with a stable (unchanged) typing. If the loop instead runs out of rounds
    // mid-switch, the active set never settled and the snapshot is not a real fixed point.
    let mut settled = false;
    for _ in 0..MAX_OUTER {
        let layout = AcPfLayout::with_kinds(net, &kinds);
        let (nvm, nva, iters, residual) = newton(
            f,
            net,
            ybus,
            &layout,
            pg_bus,
            &qsched,
            vm.clone(),
            va.clone(),
        );
        vm = nvm;
        va = nva;
        // Snapshot the typing this round's Newton actually used, alongside its
        // (vm, va), so the returned kinds always matches the returned point — even
        // when the outer loop exits on MAX_OUTER with a switch still pending.
        result = (vm.clone(), va.clone(), kinds.clone(), iters, residual);
        if residual >= TOL {
            // Newton's own non-convergence under this typing is a settled verdict the
            // residual already carries; report the best iterate.
            settled = true;
            break;
        }

        // Recover each regulating bus's reactive generation and switch on a limit violation.
        let (_, q_calc, _) = f.injections(ybus, &vm, &va);
        let mut changed = false;
        for b in 0..net.n {
            if b == net.slack || !has_gen[b] {
                continue;
            }
            if kinds[b] == BusKind::Pv {
                let qg = q_calc[b] + net.qd[b];
                if qg > qmax_bus[b] + Q_TOL {
                    kinds[b] = BusKind::Pq;
                    qsched[b] = qmax_bus[b];
                    converted[b] = 1;
                    changed = true;
                } else if qg < qmin_bus[b] - Q_TOL {
                    kinds[b] = BusKind::Pq;
                    qsched[b] = qmin_bus[b];
                    converted[b] = -1;
                    changed = true;
                }
            } else if converted[b] != 0 && !reverted[b] {
                // Back-switch (once per bus): a bus fixed at qmax whose released voltage rose
                // above the setpoint (or fixed at qmin and fell below) could hold the setpoint
                // within its limit, so restore it to PV. The one-shot cap prevents the bus from
                // toggling back to PQ and reverting again round after round.
                let revert = (converted[b] == 1 && vm[b] > vm_hold[b] + V_BACK)
                    || (converted[b] == -1 && vm[b] < vm_hold[b] - V_BACK);
                if revert {
                    kinds[b] = BusKind::Pv;
                    vm[b] = vm_hold[b];
                    qsched[b] = qg_bus[b];
                    converted[b] = 0;
                    reverted[b] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            settled = true;
            break; // converged with a stable typing
        }
        // Re-pin the magnitude at any bus regulating this round before the next Newton.
        for b in 0..net.n {
            if kinds[b] != BusKind::Pq {
                vm[b] = vm_hold[b];
            }
        }
    }

    // Exhausting MAX_OUTER with a switch still pending means the PV/PQ active set never
    // settled: the snapshot's sub-TOL residual converged under a typing the loop had
    // already decided to abandon (a generator still past its reactive limit). Surface
    // non-convergence so the caller neither accepts it nor linearizes the AC sensitivity
    // around an unstable active set; a cleaner restart can still win in `ac_pf`.
    if !settled {
        result.4 = f64::INFINITY;
    }
    result
}

/// A deterministic value in `[-1, 1]` from `(a, b)` — a SplitMix64-style mix. Used to
/// perturb restarts reproducibly, with no RNG dependency (the solves stay deterministic).
fn mix(a: u64, b: u64) -> f64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    (x as f64 / u64::MAX as f64) * 2.0 - 1.0
}

/// Solve the AC power flow for `net` under formulation `f`: try a flat start, then a few
/// deterministic perturbations of it, and keep the lowest-residual converged result. Each
/// start runs the Q-limit outer loop ([`solve_qlim`]), so a generator that would exceed its
/// reactive limit is backed off to the limit and its bus released to PQ. Slack and
/// still-regulating PV magnitudes hold their setpoints; the reference angle stays at zero.
/// Generic over the formulation, like [`dc_pf`](super::dc_pf).
pub fn ac_pf<F: AcPfFormulation>(f: &F, net: &AcNetwork) -> Result<AcPfSolution, String> {
    let ybus = net.ybus();
    let (qmin_bus, qmax_bus, has_gen) = aggregate_q(net);
    let vm_hold = net.vm_set.clone();
    let flat_vm = net.vm_set.clone();
    let flat_va = vec![0.0; net.n];

    let mut best: Option<QlimSolve> = None;
    for restart in 0..=RESTARTS {
        let (vm0, va0) = if restart == 0 {
            (flat_vm.clone(), flat_va.clone())
        } else {
            // Perturb the free unknowns: ±0.1 rad on non-slack angles, ±3% on non-slack
            // magnitudes (released PV buses get a magnitude start too).
            let mut vm = flat_vm.clone();
            let mut va = flat_va.clone();
            for b in 0..net.n {
                if b != net.slack {
                    va[b] = 0.10 * mix(b as u64, restart as u64);
                    vm[b] = flat_vm[b] * (1.0 + 0.03 * mix(b as u64, restart as u64 + 1_000));
                }
            }
            (vm, va)
        };
        let res = solve_qlim(
            f, net, &ybus, &net.pg, &net.qg, &qmin_bus, &qmax_bus, &has_gen, &vm_hold, vm0, va0,
        );
        // Keep the smallest-residual restart. A non-finite (NaN) residual must never
        // displace a finite one nor mask a later converging start: replace only when
        // the new residual is finite and strictly better (or the current best is NaN).
        let replace = best
            .as_ref()
            .is_none_or(|b| !res.4.is_nan() && (b.4.is_nan() || res.4 < b.4));
        if replace {
            best = Some(res);
        }
        if best.as_ref().is_some_and(|b| b.4 < TOL) {
            break;
        }
    }

    let (vm, va, kinds, iters, residual) = best.expect("at least the flat start ran");
    // Reject NaN as well as an over-tolerance mismatch: `NaN >= TOL` is false, so a
    // best built from an all-NaN restart set would otherwise fall through as converged.
    if residual.is_nan() || residual >= TOL {
        return Err(format!(
            "AC power flow did not converge: best mismatch {residual:.3e} over {} starts",
            RESTARTS + 1
        ));
    }
    let (p, q, _) = f.injections(&ybus, &vm, &va);
    Ok(AcPfSolution::new(vm, va, p, q, iters, residual, kinds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case9_ac;
    use num_complex::Complex;

    fn approx(a: f64, b: f64, tol: f64, what: &str) {
        assert!((a - b).abs() < tol, "{what}: expected {b}, got {a}");
    }

    /// A minimal 2-bus AC case with a closed-form check: bus 1 is the slack at
    /// `V = 1∠0`, bus 2 a PQ load (30 MW, 10 MVAr) behind a single `0.01 + j0.1`
    /// line. One generator at the slack supplies whatever closes the balance.
    const CASE2: &str = "\
function mpc = case2ac
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 230 1 1.1 0.9;
 2 1 30 10 0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0 0 300 -300 1 100 1 300 0 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0 10 0;
];
";

    fn parse_case2() -> AcNetwork {
        let net = powerio::parse_str(CASE2, "matpower")
            .expect("parse case2")
            .network;
        AcNetwork::from_network(&net).expect("build AcNetwork")
    }

    #[test]
    fn two_bus_matches_independent_gauss_seidel() {
        let net = parse_case2();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        assert!(sol.residual < 1e-8, "Newton residual {}", sol.residual);
        // Slack pinned at 1∠0.
        approx(sol.vm[net.slack], 1.0, 1e-9, "slack vm");
        approx(sol.va[net.slack], 0.0, 1e-12, "slack va");

        // Independent Gauss-Seidel for the single PQ bus (no shunt, no charging,
        // so Y22 = y and Y21 = -y). A different algorithm than Newton, so agreement
        // pins the operating point, not just self-consistency.
        let y = Complex::new(net.g[0], net.b[0]);
        let s2 = Complex::new(net.pg[1] - net.pd[1], net.qg[1] - net.qd[1]);
        let v1 = Complex::new(1.0, 0.0);
        let mut v2 = Complex::new(1.0, 0.0);
        for _ in 0..500 {
            let v2n = (s2.conj() / v2.conj() + y * v1) / y;
            if (v2n - v2).norm() < 1e-15 {
                v2 = v2n;
                break;
            }
            v2 = v2n;
        }
        let v2_newton = Complex::from_polar(sol.vm[1], sol.va[1]);
        approx(v2_newton.re, v2.re, 1e-8, "V2 real");
        approx(v2_newton.im, v2.im, 1e-8, "V2 imag");

        // The recovered injection at the load bus equals the specified load.
        approx(sol.p[1], net.pg[1] - net.pd[1], 1e-8, "P2");
        approx(sol.q[1], net.qg[1] - net.qd[1], 1e-8, "Q2");
    }

    #[cfg(feature = "conic")]
    #[test]
    fn conic_objective_is_a_lower_bound_on_ac() {
        use crate::model::{parse_case3_ac, parse_case9_ac};
        use crate::problem::socwr_opf;
        // Reference AC OPF objectives from the PowerModels (Julia) parity harness;
        // the SOCWR relaxation is a convex lower bound, tight on these cases.
        for (net, ac_ref, soc_ref) in [
            (parse_case3_ac(), 631.5334, 631.5334),
            (parse_case9_ac(), 5296.6862, 5296.6659),
        ] {
            let sol = socwr_opf(&net).expect("socwr solve");
            assert!(
                sol.objective <= ac_ref + 1e-3,
                "relaxation bound violated: SOCWR {} > AC {ac_ref}",
                sol.objective
            );
            assert!(
                (sol.objective - soc_ref).abs() < 0.5,
                "SOCWR objective {} != reference {soc_ref}",
                sol.objective
            );
        }
    }

    #[test]
    fn case9_converges_with_pv_pq_typing() {
        let net = parse_case9_ac();
        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow case9");
        assert!(sol.residual < TOL, "residual {}", sol.residual);
        // Newton converges quadratically here; bound the final round's iteration count so a
        // regression that quietly degrades convergence speed (a Jacobian-assembly or
        // line-search slowdown) trips the suite instead of shipping silently.
        assert!(
            sol.iterations <= 10,
            "case9 took {} Newton iters",
            sol.iterations
        );
        approx(sol.va[net.slack], 0.0, 1e-12, "slack angle pinned");

        let layout = AcPfLayout::new(&net);
        // case9's voltage-regulating generators hold the slack at vg = 1.04 and the two PV
        // buses at vg = 1.025. Asserting the literal setpoints — not just self-consistency
        // with vm_set — catches a regression to regulating at the flat bus.vm = 1.0.
        approx(
            sol.vm[net.slack],
            1.04,
            1e-9,
            "slack regulated to gen vg 1.04",
        );
        for i in 0..net.n {
            match layout.kind(i) {
                // PV buses hold the generator voltage setpoint.
                BusKind::Pv => approx(sol.vm[i], 1.025, 1e-9, "PV regulated to gen vg 1.025"),
                BusKind::Slack => {}
                // PQ buses meet both the active and reactive injection schedule.
                BusKind::Pq => {
                    approx(sol.p[i], net.pg[i] - net.pd[i], 1e-7, "P balance");
                    approx(sol.q[i], net.qg[i] - net.qd[i], 1e-7, "Q balance");
                }
            }
            // Active balance holds at every non-slack bus (PV included).
            if i != net.slack {
                approx(sol.p[i], net.pg[i] - net.pd[i], 1e-7, "P balance");
            }
            // Voltages stay physical.
            assert!(
                sol.vm[i] > 0.5 && sol.vm[i] < 1.5 && sol.vm[i].is_finite(),
                "vm[{i}] = {} out of range",
                sol.vm[i]
            );
        }
        // At least one bus is PV (case9 has voltage-regulating generators).
        assert!(
            (0..net.n).any(|i| layout.kind(i) == BusKind::Pv),
            "case9 should have PV buses"
        );
    }

    /// A 3-bus case to exercise the Q-limit outer loop. Bus 2 carries a voltage-regulating
    /// generator (vg = 1.05) with a deliberately tight reactive ceiling (qmax = 1 MVAr); the
    /// 60-MVAr load behind it at bus 3 demands far more reactive support than that to hold
    /// 1.05, so the generator must hit its limit.
    const CASE3QLIM: &str = "\
function mpc = case3qlim
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1.00 0 230 1 1.1 0.9;
 2 2 20 10 0 0 1 1.05 0 230 1 1.1 0.9;
 3 1 80 60 0 0 1 1.00 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1.00 100 1 300 0 0 0 0 0 0 0 0 0 0 0 0;
 2 50 0 1   -100 1.05 100 1 300 0 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.05 0 250 250 250 0 0 1 -360 360;
 2 3 0.02 0.08 0 250 250 250 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0 10 0;
 2 0 0 3 0 10 0;
];
";

    fn parse_case3qlim() -> AcNetwork {
        let net = powerio::parse_str(CASE3QLIM, "matpower")
            .expect("parse case3qlim")
            .network;
        AcNetwork::from_network(&net).expect("build AcNetwork")
    }

    /// A PV generator that cannot supply the reactive power needed to hold its voltage
    /// setpoint hits its limit and its bus is converted PV→PQ (MATPOWER `enforce_q_lims`):
    /// the reactive output pins at `qmax` and the voltage is released below the setpoint.
    /// Widening the limit removes the conversion and the bus holds its setpoint again.
    #[test]
    fn pv_bus_switches_to_pq_at_reactive_limit() {
        let net = parse_case3qlim();
        let b2 = net.bus_ids.iter().position(|&id| id == 2).expect("bus 2");
        let qmax2: f64 = (0..net.k)
            .filter(|&g| net.gen_bus[g] == b2)
            .map(|g| net.qmax[g])
            .sum();

        let sol = ac_pf(&AcPolar::new(), &net).expect("ac power flow");
        assert!(sol.residual < TOL, "residual {}", sol.residual);
        // The generator's reactive limit binds, so the bus is now PQ.
        assert_eq!(
            sol.kinds[b2],
            BusKind::Pq,
            "tight-qmax PV bus should switch to PQ"
        );
        // Its reactive generation is pinned at the ceiling and its voltage fell below the
        // 1.05 setpoint it could no longer hold.
        let qg2 = sol.q[b2] + net.qd[b2];
        approx(qg2, qmax2, 1e-5, "bus 2 reactive output pinned at qmax");
        assert!(
            sol.vm[b2] < 1.05 - 1e-4,
            "released PV voltage should fall below the setpoint, got {}",
            sol.vm[b2]
        );
        // Active balance still holds everywhere off the slack.
        for i in 0..net.n {
            if i != net.slack {
                approx(sol.p[i], net.pg[i] - net.pd[i], 1e-7, "active balance");
            }
        }

        // Widen the reactive ceiling: the generator can now hold the setpoint, no switch.
        let mut wide = net.clone();
        for g in 0..wide.k {
            if wide.gen_bus[g] == b2 {
                wide.qmax[g] = 5.0;
            }
        }
        let sol_wide = ac_pf(&AcPolar::new(), &wide).expect("ac power flow (wide)");
        assert_eq!(sol_wide.kinds[b2], BusKind::Pv, "wide-limit bus stays PV");
        approx(
            sol_wide.vm[b2],
            1.05,
            1e-6,
            "wide-limit PV holds its setpoint",
        );
    }
}
