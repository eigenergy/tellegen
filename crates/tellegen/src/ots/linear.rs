//! Fixed-topology continuous subproblems of the two switching approximations.
use clarabel::solver::SupportedConeT::{self, NonnegativeConeT, SecondOrderConeT, ZeroConeT};
use num_complex::Complex64;
use powerio_matrix::AcOpfPreparation;
use powerio_prob::{AcOpfInstance, AcOtsInstance};
use powerio_tx::BranchSusceptanceFormula;

use super::{current_bounds, Dispatch};
use crate::model::{DcNetwork, PiecewiseCost};
use crate::problem::{dc_opf_cancellable, ProgramBuilder};
use crate::solve::run;
use crate::{EngineError, ErrorCode};

pub(super) fn dc(
    ots: &AcOtsInstance,
    fixed: &AcOpfInstance,
    ac: &AcOpfPreparation,
) -> Result<(f64, Dispatch), EngineError> {
    let (dc, _) = fixed.to_dc_opf();
    let dc = dc.with_branch_susceptance_formula(BranchSusceptanceFormula::TapAdjustedReactance);
    let mut model =
        DcNetwork::from_instance(&dc).map_err(|e| EngineError::new(ErrorCode::Unsupported, e))?;
    if model.branch_sources != ac.branches.analysis_sources
        || model.gen_source_rows != ac.generators.source_rows
        || !model
            .bus_ids
            .iter()
            .copied()
            .eq(ac.bus_ids.iter().map(|id| id.0))
    {
        return Err(EngineError::new(
            ErrorCode::Unsupported,
            "DC and AC preparations give different energized rows",
        ));
    }
    model.allow_shed = false;
    // Use the AC instance's selected limits, including unbounded ratings. A
    // DC current limit at |V|=1 is a bound on |P| at either terminal.
    model.angmin.clone_from(&ac.branches.angle_min);
    model.angmax.clone_from(&ac.branches.angle_max);
    model
        .angle_bound_active
        .clone_from(&ac.branches.angle_bound_active);
    for e in 0..model.m {
        let mut limit = if ac.branches.thermal_limit_active[e] {
            ac.branches.s_max[e]
        } else {
            f64::INFINITY
        };
        let current = current_bounds(ots, ac, e);
        for i in [current.from_pu, current.to_pu].into_iter().flatten() {
            limit = limit.min(i);
        }
        model.fmax[e] = limit;
        model.thermal_limit_active[e] = limit.is_finite();
    }
    let s = dc_opf_cancellable(&model, None)?;
    let mut point = Dispatch::new(ac, s.va, s.pg);
    point.p_to_pu = s.f.iter().map(|p| -p).collect();
    point.p_from_pu = s.f;
    Ok((s.objective, point))
}

// Each row below is in Clarabel's Ax + s = b convention.
struct Rows {
    builder: ProgramBuilder,
    cones: Vec<SupportedConeT<f64>>,
}

impl Rows {
    fn block(&mut self, cone: SupportedConeT<f64>, size: usize) -> usize {
        let row = self.builder.append_rows(size);
        self.cones.push(cone);
        row
    }

    fn upper(&mut self, coefficients: &[(usize, f64)], rhs: f64) {
        if rhs == f64::INFINITY {
            return;
        }
        let r = self.block(NonnegativeConeT(1), 1);
        for &(c, a) in coefficients {
            self.builder.a(r, c, a);
        }
        self.builder.rhs(r, rhs);
    }

    fn interval(&mut self, c: usize, lo: f64, hi: f64) {
        self.upper(&[(c, -1.0)], -lo);
        self.upper(&[(c, 1.0)], hi);
    }
}

/// S = S_flat + J [theta_f, vm_f - 1, theta_t, vm_t - 1].
struct Terminal {
    flat: Complex64,
    columns: [usize; 4],
    partials: [Complex64; 4],
}

impl Terminal {
    fn stamp(&self, b: &mut ProgramBuilder, p: usize, q: usize, sign: f64) {
        for (&c, d) in self.columns.iter().zip(self.partials) {
            b.a(p, c, sign * d.re);
            b.a(q, c, sign * d.im);
        }
    }

    fn evaluate(&self, x: &[f64]) -> Complex64 {
        self.columns
            .iter()
            .zip(self.partials)
            .fold(self.flat, |s, (&c, d)| s + d * x[c])
    }

    /// ||S|| <= limit, or ||S|| <= limit * (1 + dv) for a current rating.
    fn limit(&self, rows: &mut Rows, limit: f64, dv: Option<usize>) {
        if limit == 0.0 {
            let r = rows.block(ZeroConeT(2), 2);
            self.stamp(&mut rows.builder, r, r + 1, 1.0);
            rows.builder.rhs(r, -self.flat.re);
            rows.builder.rhs(r + 1, -self.flat.im);
        } else {
            let r = rows.block(SecondOrderConeT(3), 3);
            rows.builder.rhs(r, limit);
            if let Some(c) = dv {
                rows.builder.a(r, c, -limit);
            }
            self.stamp(&mut rows.builder, r + 1, r + 2, -1.0);
            rows.builder.rhs(r + 1, self.flat.re);
            rows.builder.rhs(r + 2, self.flat.im);
        }
    }
}

pub(super) fn lac(
    ots: &AcOtsInstance,
    p: &AcOpfPreparation,
) -> Result<(f64, Dispatch), EngineError> {
    let n = p.n_buses;
    let k = p.n_generators();
    let gens = &p.generators;
    let br = &p.branches;
    let pg = 2 * n;
    let qg = pg + k;
    let mut epigraph = qg + k;
    let nv = epigraph + gens.piecewise_linear.iter().flatten().count();
    let mut rows = Rows {
        builder: ProgramBuilder::new(nv, 0),
        cones: vec![],
    };
    rows.block(ZeroConeT(2 * n), 2 * n);
    let mut pd = p.buses.p_d.clone();
    let mut qd = p.buses.q_d.clone();
    for i in 0..n {
        // |V|² = 1 + 2 dv to first order. Shunt withdrawals are (G, -B)|V|².
        pd[i] += p.buses.g_s[i];
        qd[i] -= p.buses.b_s[i];
        rows.builder.a(i, n + i, -2.0 * p.buses.g_s[i]);
        rows.builder.a(n + i, n + i, 2.0 * p.buses.b_s[i]);
        if p.buses.voltage_bound_active[i] {
            rows.interval(n + i, p.buses.vm_min[i] - 1.0, p.buses.vm_max[i] - 1.0);
        } else {
            rows.upper(&[(n + i, -1.0)], 1.0);
        }
    }
    for &bus in p.reference_buses.iter() {
        let r = rows.block(ZeroConeT(1), 1);
        rows.builder.a(r, bus, 1.0);
    }
    for g in 0..k {
        let bus = gens.bus_of_gen[g];
        rows.builder.a(bus, pg + g, 1.0);
        rows.builder.a(n + bus, qg + g, 1.0);
        if gens.capability_active[g] {
            rows.interval(pg + g, gens.pmin[g], gens.pmax[g]);
            rows.interval(qg + g, gens.qmin[g], gens.qmax[g]);
        }
        if let Some(curve) = &gens.piecewise_linear[g] {
            let cost = PiecewiseCost::from_prepared(curve.clone());
            rows.builder.lin(epigraph, 1.0);
            for (&a, &b) in cost.slopes.iter().zip(&cost.intercepts) {
                rows.upper(&[(pg + g, a), (epigraph, -1.0)], -b);
            }
            epigraph += 1;
        } else {
            if gens.q[g] < 0.0 {
                return Err(EngineError::new(
                    ErrorCode::Unsupported,
                    "LACOTS requires convex generation costs",
                ));
            }
            rows.builder.quad(pg + g, gens.q[g]);
            rows.builder.lin(pg + g, gens.c[g]);
        }
    }
    let vm = vec![1.0; n];
    let va = vec![0.0; n];
    let flat = br.calc_branch_flows(&vm, &va);
    let mut terminals = Vec::with_capacity(br.len());
    for e in 0..br.len() {
        let (f, t) = (br.from_bus[e], br.to_bus[e]);
        let (df, dt) = br.calc_branch_flow_partials(e, &vm, &va);
        let columns = [f, n + f, t, n + t];
        let from = Terminal {
            columns,
            partials: df,
            flat: Complex64::new(flat.pf[e], flat.qf[e]),
        };
        let to = Terminal {
            columns,
            partials: dt,
            flat: Complex64::new(flat.pt[e], flat.qt[e]),
        };
        let current = current_bounds(ots, p, e);
        for (bus, end, imax) in [(f, &from, current.from_pu), (t, &to, current.to_pu)] {
            end.stamp(&mut rows.builder, bus, n + bus, -1.0);
            pd[bus] += end.flat.re;
            qd[bus] += end.flat.im;
            if br.thermal_limit_active[e] {
                end.limit(&mut rows, br.s_max[e], None);
            }
            if let Some(i) = imax {
                end.limit(&mut rows, i, Some(n + bus));
            }
        }
        if br.angle_bound_active[e] {
            rows.upper(&[(f, 1.0), (t, -1.0)], br.angle_max[e]);
            rows.upper(&[(f, -1.0), (t, 1.0)], -br.angle_min[e]);
        }
        terminals.push((from, to));
    }
    for i in 0..n {
        rows.builder.rhs(i, pd[i]);
        rows.builder.rhs(n + i, qd[i]);
    }
    let solution = run(&rows.builder.finish(rows.cones), None)?;
    let x = &solution.x;
    let mut point = Dispatch::new(p, x[..n].to_vec(), x[pg..qg].to_vec());
    point.vm_pu = Some(x[n..pg].iter().map(|dv| dv + 1.0).collect());
    point.qg_pu = Some(x[qg..qg + k].to_vec());
    let mut q_from = Vec::with_capacity(br.len());
    let mut q_to = Vec::with_capacity(br.len());
    for (from, to) in terminals {
        let (sf, st) = (from.evaluate(x), to.evaluate(x));
        point.p_from_pu.push(sf.re);
        point.p_to_pu.push(st.re);
        q_from.push(sf.im);
        q_to.push(st.im);
    }
    point.q_from_pu = Some(q_from);
    point.q_to_pu = Some(q_to);
    Ok((solution.objective + gens.c0.iter().sum::<f64>(), point))
}
