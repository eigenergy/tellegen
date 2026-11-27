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
//!
//! @block Block factorization

use faer::sparse::Triplet;

use super::integrate::{step_entry, NetLu, Patterns};
use super::system::DynSystem;

/// A pivot below this fraction of its block's largest entry counts as zero.
const PIVOT_MIN: f64 = 1e-13;

/// Experimental: units from which their blocks are factorized on the spin pool,
/// and the number of chunks they are split into.
pub(crate) static DBG_BLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const PAR_UNITS_MIN: usize = 256;
const PAR_UNIT_CHUNKS: usize = 64;

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

/// The factorization of one step matrix by blocks. With enough units the
/// units' blocks are assembled, factorized and solved in chunks on the spin
/// pool; the chunks join in unit order, so every array and the network's
/// triplets are those of the serial loop, value for value.
// @block Factorization by blocks
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
    net: NetLu,
    /// Right-hand side of the network system.
    rhs: Vec<f64>,
    /// A unit's states during its solve.
    tmp: Vec<f64>,
    /// Experimental: unit chunks of the parallel solve (bounds into `units`; one
    /// chunk = serial) and each unit's `B_k z_k`, added to the network side in unit order.
    solve_chunks: Vec<usize>,
    contrib: Vec<[f64; 2]>,
}

/// Experimental: threads of the spin pool besides the caller (none on wasm without
/// threads, where the unit chunks run serially).
fn pool_threads() -> usize {
    #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
    return super::spin_pool::SpinPool::global().threads();
    #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
    0
}

/// Experimental: `TELLEGEN_NET_ONE=1` factorizes the network as one sparse LU even with
/// regions (which then still split the residual and the Jacobian).
fn net_one() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("TELLEGEN_NET_ONE").is_ok_and(|v| v == "1"))
}

/// Experimental: the region of each dense bus from `TELLEGEN_REGIONS` (a JSON map
/// of bus number to region), or `None` without one.
pub(super) fn region_map(sys: &DynSystem) -> Option<&'static [u16]> {
    static MAP: std::sync::OnceLock<Option<(usize, Vec<u16>)>> = std::sync::OnceLock::new();
    let m = MAP.get_or_init(|| {
        let path = std::env::var("TELLEGEN_REGIONS").ok()?;
        let text = std::fs::read_to_string(path).ok()?;
        let by_number: std::collections::HashMap<String, u16> = serde_json::from_str(&text).ok()?;
        let v = sys
            .bus_numbers
            .iter()
            .map(|b| by_number.get(&b.to_string()).copied().unwrap_or(0))
            .collect();
        Some((sys.n, v))
    });
    m.as_ref().filter(|(n, _)| *n == sys.n).map(|(_, v)| v.as_slice())
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
        ranges: Option<&[(usize, usize, usize)]>,
        h: f64,
        theta: f64,
        patterns: &mut Patterns,
    ) -> Result<Option<Self>, String> {
        let t_pre = crate::rms::clock::Instant::now();
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
        super::integrate::time_add(15, t_pre);
        DBG_BLOCK.store(block as u64, std::sync::atomic::Ordering::Relaxed);
        let t_asm = crate::rms::clock::Instant::now();
        let nu = units.len();
        let pooled = sys.threads && nu >= PAR_UNITS_MIN && pool_threads() > 0;
        let nchunks = if pooled { PAR_UNIT_CHUNKS.min(nu) } else { 1 };
        let bounds: Vec<usize> = (0..=nchunks).map(|c| c * nu / nchunks).collect();
        // Experimental: when the Jacobian says where each unit's entries lie, the
        // units' entries are assembled by the chunks below (each entry lands where the serial
        // loop puts it, and the network entries are joined in the Jacobian's order); here only
        // the network's and loads' entries. The Jacobian builds devices, then plants: not the
        // state order, so each range is mapped to its unit by its first state.
        let unit_range: Option<Vec<(usize, usize)>> = ranges.filter(|_| pooled).and_then(|r| {
            if r.len() != nu || !r.windows(2).all(|p| p[0].2 == p[1].1) || r.last().is_some_and(|l| l.2 != jf.len()) {
                return None;
            }
            let mut ur = vec![(usize::MAX, 0); nu];
            for &(off, a, b) in r {
                let i = *owner.get(off.checked_sub(n2)?)? as usize;
                if i >= nu || units[i].first != off - n2 || ur[i].0 != usize::MAX {
                    return None;
                }
                ur[i] = (a, b);
            }
            Some(ur)
        });
        let split_asm = unit_range.is_some();
        let prefix = match (split_asm, ranges) {
            (true, Some(r)) => r.first().map_or(jf.len(), |f| f.1),
            _ => jf.len(),
        };
        let mut trip: Vec<Triplet<usize, usize, f64>> =
            Vec::with_capacity(4 * sys.ybus.len() + 8 * units.len() + 8);
        for &(row, col, v) in &jf[..prefix] {
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
        if !split_asm {
            for u in &units {
                for i in 0..u.k {
                    let m = mass[n2 + u.first + i];
                    if m != 0.0 {
                        lu[u.block + i * u.k + i] += m;
                    }
                }
            }
        }
        let widest = units.iter().map(|u| u.k).max().unwrap_or(0);
        if widest > usize::from(u16::MAX) {
            return Ok(None);
        }
        let mut perm = vec![0u32; states];
        let tmp = vec![0.0; widest];
        let mut factors = Factors {
            col: Vec::with_capacity(block / 2),
            val: Vec::with_capacity(block / 2),
            start: Vec::with_capacity(states + 1),
            mid: Vec::with_capacity(states),
            inv: Vec::with_capacity(states),
        };
        factors.start.push(0);
        super::integrate::time_add(8, t_asm);
        let t_units = crate::rms::clock::Instant::now();
        // Experimental: the units' blocks in chunks on the spin pool. Each chunk owns
        // its units' contiguous parts of `lu`, `perm` and `w` and builds its own compressed
        // factors; they are joined in unit order afterwards, with the network entries, so the
        // arrays and the triplets are those of the serial loop, value for value.
        type ChunkOut = (Factors, Vec<[[f64; 3]; 2]>, bool, Vec<Triplet<usize, usize, f64>>, Vec<usize>);
        #[allow(clippy::type_complexity)]
        let mut parts: Vec<std::sync::Mutex<(&mut [f64], &mut [u32], &mut [f64], &mut [f64], ChunkOut)>> =
            Vec::with_capacity(nchunks);
        {
            let (mut lu_rest, mut perm_rest, mut w_rest, mut b_rest): (&mut [f64], &mut [u32], &mut [f64], &mut [f64]) =
                (&mut lu[..], &mut perm[..], &mut w[..], &mut b[..]);
            let (mut lu_at, mut st_at) = (0usize, 0usize);
            for c in 0..nchunks {
                let next = bounds[c + 1];
                let lu_end = if next < nu { units[next].block } else { block };
                let st_end = if next < nu { units[next].first } else { states };
                let (l, lr) = std::mem::take(&mut lu_rest).split_at_mut(lu_end - lu_at);
                let (pm, pr) = std::mem::take(&mut perm_rest).split_at_mut(st_end - st_at);
                let (wv, wr) = std::mem::take(&mut w_rest).split_at_mut(3 * (st_end - st_at));
                let (bv, br) = std::mem::take(&mut b_rest).split_at_mut(2 * (st_end - st_at));
                (lu_rest, perm_rest, w_rest, b_rest) = (lr, pr, wr, br);
                (lu_at, st_at) = (lu_end, st_end);
                let f = Factors {
                    col: Vec::new(),
                    val: Vec::new(),
                    start: vec![0],
                    mid: Vec::new(),
                    inv: Vec::new(),
                };
                parts.push(std::sync::Mutex::new((l, pm, wv, bv, (f, Vec::new(), true, Vec::new(), vec![0]))));
            }
        }
        let run_chunk = |c: usize| {
            let mut guard = parts[c].lock().expect("unit chunk");
            let (lu_c, perm_c, w_c, b_c, (fac, contrib, ok, ntrip, spans)) = &mut *guard;
            let (a, z) = (bounds[c], bounds[c + 1]);
            if a == z {
                return;
            }
            let (lu0, st0) = (units[a].block, units[a].first);
            let mut tmp = vec![0.0; widest];
            let mut rhs_col = vec![0.0; widest];
            contrib.reserve(z - a);
            for (ui, u) in units[a..z].iter().enumerate() {
                let ui = a + ui;
                let k = u.k;
                let (fl, fs) = (u.first - st0, u.block - lu0);
                if split_asm {
                    // The unit's own entries, as the serial loop places them.
                    let (e0, e1) = unit_range.as_ref().expect("checked")[ui];
                    for &(row, col, v) in &jf[e0..e1] {
                        let e = step_entry(mass, row, v, h, theta);
                        if row < n2 {
                            if col < n2 {
                                ntrip.push(Triplet::new(row, col, e));
                            } else if reference == Some(col - n2) {
                                ntrip.push(Triplet::new(row, n2, e));
                            } else {
                                let which = if owner[col - n2] as usize != ui {
                                    *ok = false;
                                    return;
                                } else if row == u.bus {
                                    0
                                } else if row == n + u.bus {
                                    1
                                } else {
                                    *ok = false;
                                    return;
                                };
                                b_c[2 * (col - n2 - st0) + which] += e;
                            }
                        } else {
                            let s = row - n2;
                            if owner[s] as usize != ui {
                                *ok = false;
                                return;
                            }
                            if col >= n2 && col - n2 >= u.first && col - n2 < u.first + u.k {
                                lu_c[fs + (s - u.first) * k + (col - n2 - u.first)] += e;
                            } else if col == u.bus {
                                w_c[3 * (s - st0)] += e;
                            } else if col == n + u.bus {
                                w_c[3 * (s - st0) + 1] += e;
                            } else if col >= n2 && reference == Some(col - n2) {
                                w_c[3 * (s - st0) + 2] += e;
                            } else {
                                *ok = false;
                                return;
                            }
                        }
                    }
                    for i in 0..k {
                        let m = mass[n2 + u.first + i];
                        if m != 0.0 {
                            lu_c[fs + i * k + i] += m;
                        }
                    }
                    spans.push(ntrip.len());
                }
                let blk = &mut lu_c[fs..fs + k * k];
                let p = &mut perm_c[fl..fl + k];
                if !factor(blk, k, p) {
                    *ok = false;
                    return;
                }
                if fac.push(blk, k).is_err() {
                    *ok = false;
                    return;
                }
                // `D_k⁻¹ [C_k  E_k]`, a column at a time.
                for col in 0..3 {
                    for i in 0..k {
                        rhs_col[i] = w_c[3 * (fl + i) + col];
                    }
                    fac.solve(fl, k, p, &mut rhs_col[..k], &mut tmp);
                    for i in 0..k {
                        w_c[3 * (fl + i) + col] = rhs_col[i];
                    }
                }
                if w_c[3 * fl..3 * (fl + k)].iter().any(|v| !v.is_finite()) {
                    *ok = false;
                    return;
                }
                // The unit's contribution to its bus: `−B_k D_k⁻¹ [C_k  E_k]`.
                let mut sc = [[0.0; 3]; 2];
                for i in 0..k {
                    let (b0, b1) = (b_c[2 * (fl + i)], b_c[2 * (fl + i) + 1]);
                    for col in 0..3 {
                        let wv = w_c[3 * (fl + i) + col];
                        sc[0][col] -= b0 * wv;
                        sc[1][col] -= b1 * wv;
                    }
                }
                contrib.push(sc);
            }
        };
        if nchunks > 1 {
            super::integrate::par_for(nchunks, &run_chunk);
        } else {
            run_chunk(0);
        }
        // Join in unit order: the compressed factors, then the network entries.
        let t_join = crate::rms::clock::Instant::now();
        let outs: Vec<ChunkOut> = parts.into_iter().map(|p| p.into_inner().expect("unit chunk").4).collect();
        if outs.iter().any(|o| !o.2) {
            return Ok(None);
        }
        // The units' own network entries first (where the serial loop put them), then below the
        // units' contributions.
        if let (Some(r), true) = (ranges, split_asm) {
            // In the Jacobian's order, as the serial loop adds them.
            for &(off, _, _) in r {
                let ui = owner[off - n2] as usize;
                let c = bounds.partition_point(|&x| x <= ui) - 1;
                let (o, j) = (&outs[c], ui - bounds[c]);
                trip.extend_from_slice(&o.3[o.4[j]..o.4[j + 1]]);
            }
        }
        for (c, (fac, contrib, _, _, _)) in outs.into_iter().enumerate() {
            let off = u32::try_from(factors.col.len()).map_err(|e| e.to_string())?;
            u32::try_from(factors.col.len() + fac.col.len()).map_err(|e| e.to_string())?;
            factors.col.extend_from_slice(&fac.col);
            factors.val.extend_from_slice(&fac.val);
            factors.inv.extend_from_slice(&fac.inv);
            factors.mid.extend(fac.mid.iter().map(|m| m + off));
            factors.start.extend(fac.start[1..].iter().map(|m| m + off));
            // The unit's contribution: always entered, zero or not, so the network's pattern stays.
            for (u, sc) in units[bounds[c]..bounds[c + 1]].iter().zip(&contrib) {
                let rows = [u.bus, n + u.bus];
                for (r, &row) in rows.iter().enumerate() {
                    trip.push(Triplet::new(row, u.bus, sc[r][0]));
                    trip.push(Triplet::new(row, n + u.bus, sc[r][1]));
                    if reference.is_some() {
                        trip.push(Triplet::new(row, n2, sc[r][2]));
                    }
                }
            }
        }
        super::integrate::time_add(18, t_join);
        if let Some(r) = reference {
            // `w = e_ωᵀ y_ref` with `y_ref = D⁻¹ (r_ref − C_ref v)`.
            let u = &units[owner[r] as usize];
            trip.push(Triplet::new(n2, n2, 1.0));
            trip.push(Triplet::new(n2, u.bus, w[3 * r]));
            trip.push(Triplet::new(n2, n + u.bus, w[3 * r + 1]));
        }
        super::integrate::time_add(9, t_units);
        let t_net = crate::rms::clock::Instant::now();
        let net = NetLu::new(size, n, &trip, patterns, if sys.threads && !net_one() { region_map(sys) } else { None }, sys.epoch)?;
        super::integrate::time_add(10, t_net);
        let Factors {
            col,
            val,
            start,
            mid,
            inv,
        } = factors;
        Ok(Some(Self {
            n,
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
            solve_chunks: if pooled {
                (0..=nchunks).map(|c| c * nu / nchunks).collect()
            } else {
                vec![0, nu]
            },
            contrib: vec![[0.0; 2]; nu],
            units,
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
    /// row) into `rhs`. The units' solves run in chunks on the spin pool; their
    /// contributions are added in unit order afterwards, as the serial loop
    /// adds them.
    // @block Eliminate the units
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
        let (units, perm, b, bounds) = (&self.units, &self.perm, &self.b, &self.solve_chunks);
        let nchunks = bounds.len() - 1;
        let one = |dev_c: &mut [f64], at: usize, contrib_c: &mut [[f64; 2]], c: usize, tmp: &mut [f64]| {
            for (j, u) in units[bounds[c]..bounds[c + 1]].iter().enumerate() {
                let k = u.k;
                let z = &mut dev_c[u.first - at..u.first - at + k];
                factors.solve(u.first, k, &perm[u.first..u.first + k], z, tmp);
                let bb = &b[2 * u.first..2 * (u.first + k)];
                let (mut s0, mut s1) = (0.0, 0.0);
                for (zi, bi) in z.iter().zip(bb.chunks_exact(2)) {
                    s0 += bi[0] * zi;
                    s1 += bi[1] * zi;
                }
                contrib_c[j] = [s0, s1];
            }
        };
        if nchunks > 1 {
            // Experimental: the units' solves in chunks on the spin pool; their
            // contributions are added below in unit order, as the serial loop adds them.
            let parts = split_units(units, bounds, dev, &mut self.contrib);
            let widest = self.tmp.len();
            super::integrate::par_for(nchunks, &|c| {
                let mut g = parts[c].lock().expect("unit chunk");
                let (dev_c, at, contrib_c) = &mut *g;
                let mut tmp = vec![0.0; widest];
                one(dev_c, *at, contrib_c, c, &mut tmp);
            });
        } else {
            one(dev, 0, &mut self.contrib, 0, &mut self.tmp);
        }
        for (u, s) in units.iter().zip(&self.contrib) {
            rhs[u.bus] -= s[0];
            rhs[n + u.bus] -= s[1];
        }
        if let Some(r) = self.reference {
            rhs[n2] = dev[r];
        }
    }

    /// The network's solution from `rhs` into `x`, and
    /// `y_k = z_k − D_k⁻¹ [C_k  E_k] (v_bus, w)`, unit chunks on the spin pool.
    // @block Back-substitute the units
    fn substitute(&mut self, x: &mut [f64]) {
        let n = self.n;
        let n2 = 2 * n;
        let (bus, dev) = x.split_at_mut(n2);
        bus.copy_from_slice(&self.rhs[..n2]);
        let dw = if self.reference.is_some() {
            self.rhs[n2]
        } else {
            0.0
        };
        let (units, wv, bounds) = (&self.units, &self.w, &self.solve_chunks);
        let bus = &*bus;
        let one = |dev_c: &mut [f64], at: usize, c: usize| {
            for u in &units[bounds[c]..bounds[c + 1]] {
                let (vr, vi) = (bus[u.bus], bus[n + u.bus]);
                let w = &wv[3 * u.first..3 * (u.first + u.k)];
                let y = &mut dev_c[u.first - at..u.first - at + u.k];
                for (yi, wi) in y.iter_mut().zip(w.chunks_exact(3)) {
                    *yi -= wi[0] * vr + wi[1] * vi + wi[2] * dw;
                }
            }
        };
        let nchunks = bounds.len() - 1;
        if nchunks > 1 {
            let parts = split_units(units, bounds, dev, &mut self.contrib);
            super::integrate::par_for(nchunks, &|c| {
                let mut g = parts[c].lock().expect("unit chunk");
                let (dev_c, at, _) = &mut *g;
                one(dev_c, *at, c);
            });
        } else {
            one(dev, 0, 0);
        }
    }
}

/// Experimental: the device part and the per-unit contributions split at the unit
/// chunks' bounds, each chunk's part with the index of its first state.
#[allow(clippy::type_complexity)]
fn split_units<'a>(
    units: &[Unit],
    bounds: &[usize],
    mut dev: &'a mut [f64],
    mut contrib: &'a mut [[f64; 2]],
) -> Vec<std::sync::Mutex<(&'a mut [f64], usize, &'a mut [[f64; 2]])>> {
    let nu = units.len();
    let mut out = Vec::with_capacity(bounds.len() - 1);
    let mut at = 0usize;
    for c in 0..bounds.len() - 1 {
        let next = bounds[c + 1];
        let end = if next < nu { units[next].first } else { at + dev.len() };
        let (d, rest) = std::mem::take(&mut dev).split_at_mut(end - at);
        let (k, krest) = std::mem::take(&mut contrib).split_at_mut(next - bounds[c]);
        out.push(std::sync::Mutex::new((d, at, k)));
        (dev, contrib, at) = (rest, krest, end);
    }
    out
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
// @block Dense LU of one unit
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
        if super::not_gt(best, PIVOT_MIN * scale) {
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
    use super::*;

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

    /// Whole-case comparisons on dynamized cases (`rms-experimental`).
    #[cfg(feature = "rms-experimental")]
    mod dynamized {
        use std::path::Path;

        use super::super::super::{LoadModel, RmsSimulation};
        use super::super::*;

        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/case_data.rs"
        ));
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/wecc240.rs"
        ));

        fn simulation(dir: &Path) -> RmsSimulation {
            let module =
                powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
            let powerio::PioValue::BalancedNetwork(net) = module.value() else {
                panic!("not a balanced network")
            };
            let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap())
                .unwrap();
            RmsSimulation::new(net, &dyr, LoadModel::ConstantImpedance).unwrap()
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
            let mut blocks = BlockLu::new(sys, &sys.mass, &jf, None, h, 0.5, &mut patterns)
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
        fn blocks_solve_what_the_whole_matrix_does() {
            // WECC 240 with library machine models on every generator (the block
            // form needs every unit to be a machine, no voltage sources); the
            // cases kept outside the repository where they are.
            let Some(net) = shipped_wecc240_network() else {
                return;
            };
            let net = &net;
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
}
