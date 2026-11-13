//! The step matrix factorized by blocks.
//!
//! With the bus voltages `v` first and the states `y_k` of each unit after
//! them, the step matrix of [`super::integrate`] is
//!
//! ```text
//! [ A    B_1  B_2  … ] [ v   ]   [ r_v ]
//! [ C_1  D_1         ] [ y_1 ] = [ r_1 ]
//! [ C_2       D_2    ] [ y_2 ]   [ r_2 ]
//! [ …              … ] [ …   ]   [ …   ]
//! ```
//!
//! plus one column: the speed of the reference frame, a state of one unit
//! that every other unit's equations see. A unit couples to the network
//! only through the two voltage components of its bus, so each `D_k` is
//! eliminated on its own (a dense LU of at most a few dozen rows), which
//! leaves the network with a 2 × 2 contribution per unit,
//!
//! ```text
//! (A − Σ B_k D_k⁻¹ C_k) v = r_v − Σ B_k D_k⁻¹ r_k,     y_k = D_k⁻¹ (r_k − C_k v).
//! ```
//!
//! The frame speed `w` is carried as one more unknown of the network system
//! (a bordered column, and the row `w = e_ωᵀ y_ref`).
//!
//! Against one sparse LU over all unknowns this solves the same system in
//! another order (the corrections agree to rounding), and:
//! - the network matrix has a pattern that only the topology changes, so
//!   its symbolic factorization is found once per event, where the pattern
//!   of the whole matrix moves with every limiter that reaches its stop;
//! - a refresh factorizes a third of the unknowns sparsely and the rest in
//!   small dense blocks.
//!
//! A unit whose own block is singular (an algebraic state that only the
//! network determines) cannot be eliminated first: [`BlockLu::new`] then
//! returns `None` and the caller takes the sparse LU of the whole matrix.

use faer::sparse::Triplet;

use super::integrate::{step_entry, Patterns, SparseLu};
use super::system::DynSystem;

/// A pivot below this fraction of its block's largest entry counts as zero.
const PIVOT_MIN: f64 = 1e-13;

/// One unit's place in the factorization.
struct Unit {
    /// First state in the device part (global index minus `2 n`).
    first: usize,
    /// Number of states.
    k: usize,
    /// Dense bus index.
    bus: usize,
    /// Where its block starts among the dense blocks being factorized.
    block: usize,
}

/// The factorization of one step matrix by blocks.
pub(super) struct BlockLu {
    /// Buses.
    n: usize,
    units: Vec<Unit>,
    /// The factors of the units' blocks, row by row and without their
    /// zeros (a unit's equations touch a few of its states each): per row
    /// of a unit (in elimination order) the entries of the unit lower
    /// factor (`start[i]..mid[i]`) and of the upper factor
    /// (`mid[i]..start[i + 1]`), each with its column inside the unit, in
    /// ascending columns; `inv` holds the reciprocal pivots.
    col: Vec<u16>,
    val: Vec<f64>,
    start: Vec<u32>,
    mid: Vec<u32>,
    inv: Vec<f64>,
    /// Per unit state: the row of the block that pivoting put in its place.
    perm: Vec<u32>,
    /// Per unit state two numbers: its entries in the unit's two bus rows
    /// (`B_k`, column by column).
    b: Vec<f64>,
    /// Per unit state three numbers: `D_k⁻¹ [C_k  E_k]` (the two voltage
    /// columns and the frame speed's).
    w: Vec<f64>,
    /// The frame speed's place in the device part, when the frame moves.
    reference: Option<usize>,
    /// The network with the units' contributions (and the frame speed).
    net: SparseLu,
    /// Right-hand side of the network system.
    rhs: Vec<f64>,
    /// A unit's states during its solve.
    tmp: Vec<f64>,
}

impl BlockLu {
    /// Factorize the step matrix of the Jacobian `jf` (triplets of
    /// `∂f/∂x`) for step `h` and method `theta`; `None` when a unit's
    /// block is singular or an entry couples what this form does not
    /// (two units directly, a unit and a bus that is not its own).
    ///
    /// # Errors
    /// When the network system cannot be factorized.
    pub(super) fn new(
        sys: &DynSystem,
        mass: &[f64],
        jf: &[(usize, usize, f64)],
        h: f64,
        theta: f64,
        patterns: &mut Patterns,
    ) -> Result<Option<Self>, String> {
        let n = sys.n;
        let n2 = 2 * n;
        // The units in state order; their states must fill the device part.
        let state_units = sys.state_units();
        let mut units = Vec::with_capacity(state_units.len());
        let mut owner = vec![u32::MAX; sys.nx - n2];
        let (mut next, mut block) = (n2, 0);
        for (offset, k, bus) in state_units {
            if offset != next {
                return Ok(None);
            }
            for o in &mut owner[next - n2..next - n2 + k] {
                *o = u32::try_from(units.len()).map_err(|e| e.to_string())?;
            }
            units.push(Unit {
                first: next - n2,
                k,
                bus,
                block,
            });
            next += k;
            block += k * k;
        }
        if next != sys.nx {
            return Ok(None);
        }
        let states = sys.nx - n2;
        let reference = sys.omega_ref.map(|r| r - n2);
        let size = n2 + usize::from(reference.is_some());
        let mut lu = vec![0.0; block];
        let mut b = vec![0.0; 2 * states];
        // `[C_k  E_k]` first, `D_k⁻¹` times it after the factorization.
        let mut w = vec![0.0; 3 * states];
        let mut trip: Vec<Triplet<usize, usize, f64>> =
            Vec::with_capacity(4 * sys.ybus.len() + 8 * units.len() + 8);
        for &(row, col, v) in jf {
            let e = step_entry(mass, row, v, h, theta);
            if row < n2 {
                if col < n2 {
                    trip.push(Triplet::new(row, col, e));
                } else if reference == Some(col - n2) {
                    // Every bus row's dependence on the frame speed is the
                    // border column, the reference unit's own included.
                    trip.push(Triplet::new(row, n2, e));
                } else {
                    let u = &units[owner[col - n2] as usize];
                    let which = if row == u.bus {
                        0
                    } else if row == n + u.bus {
                        1
                    } else {
                        return Ok(None);
                    };
                    b[2 * (col - n2) + which] += e;
                }
            } else {
                let s = row - n2;
                let u = &units[owner[s] as usize];
                if col >= n2 && col - n2 >= u.first && col - n2 < u.first + u.k {
                    lu[u.block + (s - u.first) * u.k + (col - n2 - u.first)] += e;
                } else if col == u.bus {
                    w[3 * s] += e;
                } else if col == n + u.bus {
                    w[3 * s + 1] += e;
                } else if col >= n2 && reference == Some(col - n2) {
                    w[3 * s + 2] += e;
                } else {
                    return Ok(None);
                }
            }
        }
        for u in &units {
            for i in 0..u.k {
                let m = mass[n2 + u.first + i];
                if m != 0.0 {
                    lu[u.block + i * u.k + i] += m;
                }
            }
        }
        let widest = units.iter().map(|u| u.k).max().unwrap_or(0);
        if widest > usize::from(u16::MAX) {
            return Ok(None);
        }
        let mut perm = vec![0u32; states];
        let mut tmp = vec![0.0; widest];
        let mut rhs_col = vec![0.0; widest];
        let mut factors = Factors {
            col: Vec::with_capacity(block / 2),
            val: Vec::with_capacity(block / 2),
            start: Vec::with_capacity(states + 1),
            mid: Vec::with_capacity(states),
            inv: Vec::with_capacity(states),
        };
        factors.start.push(0);
        for u in &units {
            let k = u.k;
            let block = &mut lu[u.block..u.block + k * k];
            let p = &mut perm[u.first..u.first + k];
            if !factor(block, k, p) {
                return Ok(None);
            }
            factors.push(block, k)?;
            // `D_k⁻¹ [C_k  E_k]`, a column at a time.
            for c in 0..3 {
                for i in 0..k {
                    rhs_col[i] = w[3 * (u.first + i) + c];
                }
                factors.solve(u.first, k, p, &mut rhs_col[..k], &mut tmp);
                for i in 0..k {
                    w[3 * (u.first + i) + c] = rhs_col[i];
                }
            }
            if w[3 * u.first..3 * (u.first + k)]
                .iter()
                .any(|v| !v.is_finite())
            {
                return Ok(None);
            }
            // The unit's contribution to its bus: `−B_k D_k⁻¹ [C_k  E_k]`.
            // Always entered, zero or not, so the network's pattern stays.
            let mut s = [[0.0; 3]; 2];
            for i in 0..k {
                let (b0, b1) = (b[2 * (u.first + i)], b[2 * (u.first + i) + 1]);
                for c in 0..3 {
                    let wv = w[3 * (u.first + i) + c];
                    s[0][c] -= b0 * wv;
                    s[1][c] -= b1 * wv;
                }
            }
            let rows = [u.bus, n + u.bus];
            for (r, &row) in rows.iter().enumerate() {
                trip.push(Triplet::new(row, u.bus, s[r][0]));
                trip.push(Triplet::new(row, n + u.bus, s[r][1]));
                if reference.is_some() {
                    trip.push(Triplet::new(row, n2, s[r][2]));
                }
            }
        }
        if let Some(r) = reference {
            // `w = e_ωᵀ y_ref` with `y_ref = D⁻¹ (r_ref − C_ref v)`.
            let u = &units[owner[r] as usize];
            trip.push(Triplet::new(n2, n2, 1.0));
            trip.push(Triplet::new(n2, u.bus, w[3 * r]));
            trip.push(Triplet::new(n2, n + u.bus, w[3 * r + 1]));
        }
        let net = SparseLu::new(size, &trip, patterns)?;
        let Factors {
            col,
            val,
            start,
            mid,
            inv,
        } = factors;
        Ok(Some(Self {
            n,
            units,
            col,
            val,
            start,
            mid,
            inv,
            perm,
            b,
            w,
            reference,
            net,
            rhs: vec![0.0; size],
            tmp,
        }))
    }

    /// Solve in place.
    pub(super) fn solve(&mut self, x: &mut [f64]) {
        self.eliminate(x);
        self.net.solve(&mut self.rhs);
        self.substitute(x);
    }

    /// `z_k = D_k⁻¹ r_k` in place of each unit's right-hand side, and the
    /// network's right-hand side `r_v − Σ B_k z_k` (with the frame speed's
    /// row) into `rhs`.
    fn eliminate(&mut self, x: &mut [f64]) {
        let n = self.n;
        let n2 = 2 * n;
        let (bus, dev) = x.split_at_mut(n2);
        let rhs = &mut self.rhs;
        rhs[..n2].copy_from_slice(bus);
        let factors = FactorsRef {
            col: &self.col,
            val: &self.val,
            start: &self.start,
            mid: &self.mid,
            inv: &self.inv,
        };
        for u in &self.units {
            let k = u.k;
            let z = &mut dev[u.first..u.first + k];
            factors.solve(
                u.first,
                k,
                &self.perm[u.first..u.first + k],
                z,
                &mut self.tmp,
            );
            let b = &self.b[2 * u.first..2 * (u.first + k)];
            let (mut s0, mut s1) = (0.0, 0.0);
            for (zi, bi) in z.iter().zip(b.chunks_exact(2)) {
                s0 += bi[0] * zi;
                s1 += bi[1] * zi;
            }
            rhs[u.bus] -= s0;
            rhs[n + u.bus] -= s1;
        }
        if let Some(r) = self.reference {
            rhs[n2] = dev[r];
        }
    }

    /// The network's solution from `rhs` into `x`, and
    /// `y_k = z_k − D_k⁻¹ [C_k  E_k] (v_bus, w)`.
    fn substitute(&self, x: &mut [f64]) {
        let n = self.n;
        let n2 = 2 * n;
        let (bus, dev) = x.split_at_mut(n2);
        bus.copy_from_slice(&self.rhs[..n2]);
        let dw = if self.reference.is_some() {
            self.rhs[n2]
        } else {
            0.0
        };
        for u in &self.units {
            let (vr, vi) = (bus[u.bus], bus[n + u.bus]);
            let w = &self.w[3 * u.first..3 * (u.first + u.k)];
            let y = &mut dev[u.first..u.first + u.k];
            for (yi, wi) in y.iter_mut().zip(w.chunks_exact(3)) {
                *yi -= wi[0] * vr + wi[1] * vi + wi[2] * dw;
            }
        }
    }
}

/// The units' factors without their zeros while they are built (see
/// [`BlockLu`]'s fields).
struct Factors {
    col: Vec<u16>,
    val: Vec<f64>,
    start: Vec<u32>,
    mid: Vec<u32>,
    inv: Vec<f64>,
}

impl Factors {
    /// Append the factors of [`factor`] of one unit.
    fn push(&mut self, a: &[f64], k: usize) -> Result<(), String> {
        for i in 0..k {
            let row = &a[i * k..(i + 1) * k];
            for (j, &v) in row.iter().enumerate() {
                if j == i {
                    self.mid
                        .push(u32::try_from(self.col.len()).map_err(|e| e.to_string())?);
                    self.inv.push(v);
                } else if v != 0.0 {
                    // `j < k ≤ u16::MAX`, checked by the caller.
                    self.col.push(j as u16);
                    self.val.push(v);
                }
            }
            self.start
                .push(u32::try_from(self.col.len()).map_err(|e| e.to_string())?);
        }
        Ok(())
    }

    fn solve(&self, first: usize, k: usize, perm: &[u32], x: &mut [f64], tmp: &mut [f64]) {
        FactorsRef {
            col: &self.col,
            val: &self.val,
            start: &self.start,
            mid: &self.mid,
            inv: &self.inv,
        }
        .solve(first, k, perm, x, tmp);
    }
}

/// The units' factors without their zeros.
#[derive(Clone, Copy)]
struct FactorsRef<'a> {
    col: &'a [u16],
    val: &'a [f64],
    start: &'a [u32],
    mid: &'a [u32],
    inv: &'a [f64],
}

impl FactorsRef<'_> {
    /// Solve with the factors of the unit whose first row is `first`, in
    /// place (`tmp` holds `k` numbers): the sums of the dense solve
    /// ([`solve`]) without the terms that are zero.
    fn solve(&self, first: usize, k: usize, perm: &[u32], x: &mut [f64], tmp: &mut [f64]) {
        let tmp = &mut tmp[..k];
        for i in 0..k {
            tmp[i] = x[perm[i] as usize];
        }
        for i in 0..k {
            let (a, b) = (self.start[first + i] as usize, self.mid[first + i] as usize);
            let mut s = tmp[i];
            for (c, v) in self.col[a..b].iter().zip(&self.val[a..b]) {
                s -= v * tmp[usize::from(*c)];
            }
            tmp[i] = s;
        }
        for i in (0..k).rev() {
            let (a, b) = (
                self.mid[first + i] as usize,
                self.start[first + i + 1] as usize,
            );
            let mut s = tmp[i];
            for (c, v) in self.col[a..b].iter().zip(&self.val[a..b]) {
                s -= v * tmp[usize::from(*c)];
            }
            tmp[i] = s * self.inv[first + i];
        }
        x.copy_from_slice(tmp);
    }
}

/// LU of the `k × k` matrix `a` (row by row) in place with partial
/// pivoting: the unit lower factor below the diagonal, the upper factor
/// with reciprocal pivots on it; `perm[i]` is the row that ends in place
/// `i`. `false` when a pivot vanishes against the matrix's largest entry.
fn factor(a: &mut [f64], k: usize, perm: &mut [u32]) -> bool {
    for (i, p) in perm.iter_mut().enumerate() {
        *p = i as u32;
    }
    let scale = a.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if !(scale > 0.0 && scale.is_finite()) {
        return k == 0;
    }
    for c in 0..k {
        let (mut at, mut best) = (c, a[c * k + c].abs());
        for r in c + 1..k {
            let v = a[r * k + c].abs();
            if v > best {
                (at, best) = (r, v);
            }
        }
        if !(best > PIVOT_MIN * scale) {
            return false;
        }
        if at != c {
            for j in 0..k {
                a.swap(c * k + j, at * k + j);
            }
            perm.swap(c, at);
        }
        let inv = 1.0 / a[c * k + c];
        a[c * k + c] = inv;
        for r in c + 1..k {
            let l = a[r * k + c] * inv;
            a[r * k + c] = l;
            if l != 0.0 {
                for j in c + 1..k {
                    a[r * k + j] -= l * a[c * k + j];
                }
            }
        }
    }
    true
}

/// Solve with the factors of [`factor`] in place (`tmp` holds `k` numbers).
#[cfg(test)]
fn solve(a: &[f64], k: usize, perm: &[u32], x: &mut [f64], tmp: &mut [f64]) {
    let tmp = &mut tmp[..k];
    for i in 0..k {
        tmp[i] = x[perm[i] as usize];
    }
    for i in 1..k {
        let row = &a[i * k..i * k + i];
        let mut s = tmp[i];
        for j in 0..i {
            s -= row[j] * tmp[j];
        }
        tmp[i] = s;
    }
    for i in (0..k).rev() {
        let row = &a[i * k..(i + 1) * k];
        let mut s = tmp[i];
        for j in i + 1..k {
            s -= row[j] * tmp[j];
        }
        tmp[i] = s * row[i];
    }
    x.copy_from_slice(tmp);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::{LoadModel, RmsSimulation};
    use super::*;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/case_data.rs"
    ));

    fn simulation(dir: &Path) -> RmsSimulation {
        let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        let dyr =
            powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
        RmsSimulation::with_models(net, &dyr, &[], LoadModel::ConstantImpedance).unwrap()
    }

    /// A right-hand side with entries of every size on every unknown.
    fn rhs(nx: usize, seed: u64) -> Vec<f64> {
        let mut s = seed;
        (0..nx)
            .map(|_| {
                s = s
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let u = (s >> 11) as f64 / (1u64 << 53) as f64;
                (u - 0.5) * 10f64.powi(((s >> 7) % 7) as i32 - 6)
            })
            .collect()
    }

    /// Both factorizations of the step matrix of `sys` at `x` solve the
    /// same system: the largest difference of the two solutions against the
    /// solution's largest entry.
    fn compare(sys: &DynSystem, x: &[f64], h: f64) -> f64 {
        let jf = sys.jacobian(x);
        let mut patterns = Patterns::default();
        let mut blocks = BlockLu::new(sys, &sys.mass, &jf, h, 0.5, &mut patterns)
            .unwrap()
            .expect("the units' blocks are regular");
        let mut trip = Vec::new();
        for &(row, col, v) in &jf {
            trip.push(Triplet::new(
                row,
                col,
                step_entry(&sys.mass, row, v, h, 0.5),
            ));
        }
        for i in 0..sys.nx {
            if sys.mass[i] != 0.0 {
                trip.push(Triplet::new(i, i, sys.mass[i]));
            }
        }
        let mut full = SparseLu::new(sys.nx, &trip, &mut patterns).unwrap();
        let mut worst = 0.0_f64;
        for seed in 1..4 {
            let r = rhs(sys.nx, seed);
            let (mut a, mut b) = (r.clone(), r);
            blocks.solve(&mut a);
            full.solve(&mut b);
            let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            let diff = a
                .iter()
                .zip(&b)
                .fold(0.0_f64, |m, (p, q)| m.max((p - q).abs()));
            worst = worst.max(diff / scale);
        }
        worst
    }

    #[test]
    fn dense_factors_solve() {
        // A matrix that needs pivoting.
        let a0 = [0.0, 2.0, 1.0, 1.0, 1.0, 0.0, 4.0, -1.0, 3.0];
        let mut a = a0;
        let mut perm = [0u32; 3];
        assert!(factor(&mut a, 3, &mut perm));
        let want = [1.5, -2.0, 0.25];
        let mut x = [0.0; 3];
        for i in 0..3 {
            x[i] = (0..3).map(|j| a0[i * 3 + j] * want[j]).sum();
        }
        let b = x;
        solve(&a, 3, &perm, &mut x, &mut [0.0; 3]);
        for i in 0..3 {
            assert!((x[i] - want[i]).abs() < 1e-14, "{x:?}");
        }
        // Without the zeros: the same sums, the same bits.
        let mut f = Factors {
            col: Vec::new(),
            val: Vec::new(),
            start: vec![0],
            mid: Vec::new(),
            inv: Vec::new(),
        };
        f.push(&a, 3).unwrap();
        let mut y = b;
        f.solve(0, 3, &perm, &mut y, &mut [0.0; 3]);
        assert_eq!(x.map(f64::to_bits), y.map(f64::to_bits));
        // A zero column is singular.
        let mut s = [1.0, 0.0, 2.0, 0.0];
        assert!(!factor(&mut s, 2, &mut [0u32; 2]));
    }

    #[test]
    fn blocks_solve_what_the_whole_matrix_does() {
        // WECC 240 with library machine models on every generator (the block
        // form needs every unit to be a machine, no voltage sources); the
        // cases kept outside the repository where they are.
        let raw =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240/case.raw");
        let module = powerio::parse(powerio::Source::open(raw).unwrap()).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        let request = crate::rms::DynamizeRequest {
            machine: "GENROU".into(),
            exciter: Some("SEXS".into()),
            governor: Some("TGOV1".into()),
            ..crate::rms::DynamizeRequest::default()
        };
        let dyr =
            powerio::parse_dyr(&crate::rms::dyr_dynamize(net, "", &request).unwrap()).unwrap();
        let sim = RmsSimulation::new(net, &dyr, LoadModel::ConstantImpedance).unwrap();
        for h in [0.0, 0.005, 0.02] {
            let d = compare(&sim.sys, &sim.x0, h);
            assert!(d < 1e-9, "dynamized WECC 240 h {h}: {d:e}");
        }
        for dir in [rms_example("kundur"), rms_example("nordic44")]
            .into_iter()
            .flatten()
        {
            let sim = simulation(&dir);
            for h in [0.0, 0.005, 0.02] {
                let d = compare(&sim.sys, &sim.x0, h);
                assert!(d < 1e-9, "{} h {h}: {d:e}", dir.display());
            }
        }
    }
}
