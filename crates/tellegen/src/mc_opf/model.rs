//! IVR polynomial construction. Only this layer knows about POUNCE expressions.
use pounce_nl::nl_reader::{BinOp, Expr, NlProblem, NlProblemParts};
use powerio_matrix::{McAcOpfPreparation, McOpfDeviceKind};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub(super) struct Affine {
    pub constant: f64,
    pub terms: BTreeMap<usize, f64>,
}
impl Affine {
    pub fn constant(x: f64) -> Self {
        Self {
            constant: x,
            terms: BTreeMap::new(),
        }
    }
    pub fn var(i: usize) -> Self {
        Self {
            constant: 0.0,
            terms: BTreeMap::from([(i, 1.0)]),
        }
    }
    pub fn add(&self, other: &Self) -> Self {
        let mut a = self.clone();
        a.constant += other.constant;
        for (&i, &v) in &other.terms {
            *a.terms.entry(i).or_default() += v;
        }
        a.terms.retain(|_, v| *v != 0.0);
        a
    }
    pub fn scale(&self, k: f64) -> Self {
        Self {
            constant: self.constant * k,
            terms: self
                .terms
                .iter()
                .filter_map(|(&i, &v)| (k * v != 0.0).then_some((i, k * v)))
                .collect(),
        }
    }
    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.scale(-1.0))
    }
    pub fn value(&self, x: &[f64]) -> f64 {
        self.constant + self.terms.iter().map(|(&i, &v)| v * x[i]).sum::<f64>()
    }
    pub fn expr(&self) -> Expr {
        sum(std::iter::once(Expr::Const(self.constant))
            .chain(self.terms.iter().map(|(&i, &v)| scale(v, Expr::Var(i))))
            .collect())
    }
}
pub(super) type Pair = [Affine; 2];
fn sum(xs: Vec<Expr>) -> Expr {
    Expr::Sum(xs)
}
fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::Binary(op, Box::new(a), Box::new(b))
}
fn scale(k: f64, a: Expr) -> Expr {
    bin(BinOp::Mul, Expr::Const(k), a)
}
fn product(a: &Affine, b: &Affine) -> Expr {
    // Emit explicit small local monomials: scaling a sum makes POUNCE decline
    // its expanded-quadratic path. This is construction, not differentiation.
    let mut terms = vec![Expr::Const(a.constant * b.constant)];
    terms.extend(
        a.terms
            .iter()
            .map(|(&i, &v)| scale(v * b.constant, Expr::Var(i))),
    );
    terms.extend(
        b.terms
            .iter()
            .map(|(&i, &v)| scale(v * a.constant, Expr::Var(i))),
    );
    for (&i, &v) in &a.terms {
        for (&j, &w) in &b.terms {
            terms.push(scale(v * w, bin(BinOp::Mul, Expr::Var(i), Expr::Var(j))));
        }
    }
    sum(terms)
}
fn power(v: &Pair, i: &Pair) -> [Expr; 2] {
    [
        sum(vec![product(&v[0], &i[0]), product(&v[1], &i[1])]),
        bin(BinOp::Sub, product(&v[1], &i[0]), product(&v[0], &i[1])),
    ]
}
pub(super) fn pair_add(a: &Pair, b: &Pair) -> Pair {
    [a[0].add(&b[0]), a[1].add(&b[1])]
}
pub(super) fn pair_scale(a: &Pair, k: f64) -> Pair {
    [a[0].scale(k), a[1].scale(k)]
}
pub(super) fn difference(v: &[Pair], a: usize, b: Option<usize>) -> Pair {
    b.map_or_else(
        || v[a].clone(),
        |b| pair_add(&v[a], &pair_scale(&v[b], -1.0)),
    )
}
fn admittance(v: &[Pair], nodes: &[usize], g: &[Vec<f64>], b: &[Vec<f64>], k: usize) -> Pair {
    let mut out = [Affine::default(), Affine::default()];
    for (j, &node) in nodes.iter().enumerate() {
        out[0] = out[0]
            .add(&v[node][0].scale(g[k][j]))
            .sub(&v[node][1].scale(b[k][j]));
        out[1] = out[1]
            .add(&v[node][1].scale(g[k][j]))
            .add(&v[node][0].scale(b[k][j]));
    }
    out
}
#[derive(Clone, Debug)]
pub(super) struct DeviceColumns {
    pub current: Vec<Pair>,
    pub power: Vec<[usize; 2]>,
}
#[derive(Clone, Debug)]
pub(super) struct Model {
    pub prep: McAcOpfPreparation,
    pub voltage: Vec<Pair>,
    pub branch_current: Vec<Vec<Pair>>,
    /// Per branch, conductor, and end: optional active/reactive lift columns.
    pub branch_power: Vec<Vec<[Option<[usize; 2]>; 2]>>,
    pub devices: Vec<DeviceColumns>,
    pub problem: NlProblem,
}
#[derive(Default)]
struct Builder {
    start: Vec<f64>,
    names: Vec<String>,
    rows: Vec<Expr>,
    row_names: Vec<String>,
    lo: Vec<f64>,
    hi: Vec<f64>,
}
impl Builder {
    fn var(&mut self, name: String, start: f64) -> usize {
        let i = self.start.len();
        self.start.push(start);
        self.names.push(name);
        i
    }
    fn pair(&mut self, name: &str, start: [f64; 2]) -> Pair {
        [
            Affine::var(self.var(format!("{name}:r"), start[0])),
            Affine::var(self.var(format!("{name}:i"), start[1])),
        ]
    }
    fn row(&mut self, name: String, e: Expr, lo: f64, hi: f64) {
        self.rows.push(e);
        self.row_names.push(name);
        self.lo.push(lo);
        self.hi.push(hi);
    }
    fn affine_row(&mut self, name: String, a: &Affine) -> Result<(), String> {
        if a.terms.is_empty() {
            if a.constant.abs() > 1e-12 {
                return Err(format!("{name}: inconsistent fixed equation"));
            }
            return Ok(());
        }
        self.row(name, a.expr(), 0.0, 0.0);
        Ok(())
    }
    fn norm(&mut self, name: &str, v: &Pair, cap: f64) -> Result<(), String> {
        if cap == 0.0 {
            for (k, a) in v.iter().enumerate() {
                self.affine_row(format!("{name}:{k}:zero"), a)?;
            }
        } else {
            let a = v[0].scale(1.0 / cap);
            let b = v[1].scale(1.0 / cap);
            self.row(
                name.into(),
                sum(vec![product(&a, &a), product(&b, &b)]),
                f64::NEG_INFINITY,
                1.0,
            );
        }
        Ok(())
    }
    fn power_lift(&mut self, name: &str, v: &Pair, i: &Pair, start: [f64; 2]) -> [usize; 2] {
        let ids = [
            self.var(format!("{name}:p"), start[0]),
            self.var(format!("{name}:q"), start[1]),
        ];
        for (k, expr) in power(v, i).into_iter().enumerate() {
            self.row(
                format!("{name}:power:{k}"),
                bin(BinOp::Sub, Expr::Var(ids[k]), expr),
                0.0,
                0.0,
            );
        }
        ids
    }
}
pub(super) fn compile(prep: McAcOpfPreparation) -> Result<Model, String> {
    let mut b = Builder::default();
    let voltage: Vec<_> = prep
        .terminals
        .iter()
        .map(|t| {
            t.fixed.map_or_else(
                || b.pair(&format!("voltage:{}:{}", t.bus, t.terminal), t.start),
                |v| [Affine::constant(v[0]), Affine::constant(v[1])],
            )
        })
        .collect();
    let mut kcl = vec![[Affine::default(), Affine::default()]; voltage.len()];
    let mut branch_current = Vec::new();
    let mut branch_power = Vec::new();
    for br in &prep.branches {
        let mut power_columns = vec![[None; 2]; br.from.len()];
        let currents: Vec<_> = (0..br.from.len())
            .map(|k| {
                if br.open {
                    [Affine::default(), Affine::default()]
                } else {
                    b.pair(&format!("{}:{k}:series", br.identity), [0.0, 0.0])
                }
            })
            .collect();
        if !br.open {
            for k in 0..br.from.len() {
                let mut drop = difference(&voltage, br.from[k], Some(br.to[k]));
                for (j, i) in currents.iter().enumerate() {
                    drop[0] = drop[0]
                        .sub(&i[0].scale(br.r[k][j]))
                        .add(&i[1].scale(br.x[k][j]));
                    drop[1] = drop[1]
                        .sub(&i[1].scale(br.r[k][j]))
                        .sub(&i[0].scale(br.x[k][j]));
                }
                for (p, a) in drop.iter().enumerate() {
                    b.affine_row(format!("{}:{k}:kvl:{p}", br.identity), a)?;
                }
                for (end_index, (end, nodes, g, sh, sign)) in [
                    ("from", &br.from, &br.g_from, &br.b_from, 1.0),
                    ("to", &br.to, &br.g_to, &br.b_to, -1.0),
                ]
                .into_iter()
                .enumerate()
                {
                    let i = pair_add(
                        &pair_scale(&currents[k], sign),
                        &admittance(&voltage, nodes, g, sh, k),
                    );
                    let node = nodes[k];
                    kcl[node] = pair_add(&kcl[node], &pair_scale(&i, -1.0));
                    if let Some(cap) = br.current_max[k] {
                        b.norm(&format!("{}:{end}:{k}:current", br.identity), &i, cap)?;
                    }
                    if let Some(cap) = br.apparent_max[k] {
                        let ids = b.power_lift(
                            &format!("{}:{end}:{k}", br.identity),
                            &voltage[node],
                            &i,
                            [0.0, 0.0],
                        );
                        power_columns[k][end_index] = Some(ids);
                        b.norm(
                            &format!("{}:{end}:{k}:apparent", br.identity),
                            &[Affine::var(ids[0]), Affine::var(ids[1])],
                            cap,
                        )?;
                    }
                }
            }
        }
        branch_current.push(currents);
        branch_power.push(power_columns);
    }
    for sh in &prep.shunts {
        for (k, &node) in sh.terminals.iter().enumerate() {
            let i = admittance(&voltage, &sh.terminals, &sh.g, &sh.b, k);
            kcl[node] = pair_add(&kcl[node], &pair_scale(&i, -1.0));
        }
    }
    let mut devices = Vec::new();
    let mut objective = Vec::new();
    for dev in &prep.devices {
        let mut cols = DeviceColumns {
            current: Vec::new(),
            power: Vec::new(),
        };
        let mut total = [Affine::default(), Affine::default()];
        let sign = if dev.kind == McOpfDeviceKind::Load {
            -1.0
        } else {
            1.0
        };
        for (k, coil) in dev.coils.iter().enumerate() {
            let name = format!("{:?}:{}:{k}", dev.kind, dev.identity);
            let u = difference(&voltage, coil.positive, coil.negative);
            let p0 = coil.prescribed.unwrap_or([0.0, 0.0]);
            let ur = u[0].value(&b.start);
            let ui = u[1].value(&b.start);
            let u2 = ur * ur + ui * ui;
            let i0 = if u2 > 1e-12 {
                [
                    (p0[0] * ur + p0[1] * ui) / u2,
                    (p0[0] * ui - p0[1] * ur) / u2,
                ]
            } else {
                [0.0, 0.0]
            };
            let i = b.pair(&format!("{name}:current"), i0);
            let pq = b.power_lift(&name, &u, &i, p0);
            if let Some(pq0) = coil.prescribed {
                for j in 0..2 {
                    b.row(
                        format!("{name}:prescribed:{j}"),
                        Expr::Var(pq[j]),
                        pq0[j],
                        pq0[j],
                    );
                }
            } else {
                for (j, (lo, hi)) in [(coil.p_min, coil.p_max), (coil.q_min, coil.q_max)]
                    .into_iter()
                    .enumerate()
                {
                    if lo.is_some() || hi.is_some() {
                        b.row(
                            format!("{name}:capability:{j}"),
                            Expr::Var(pq[j]),
                            lo.unwrap_or(f64::NEG_INFINITY),
                            hi.unwrap_or(f64::INFINITY),
                        );
                    }
                }
            }
            if let Some(cap) = coil.current_max {
                b.norm(&format!("{name}:current_cap"), &i, cap)?;
            }
            if let Some(cap) = coil.apparent_max {
                b.norm(
                    &format!("{name}:apparent_cap"),
                    &[Affine::var(pq[0]), Affine::var(pq[1])],
                    cap,
                )?;
            }
            objective.push(scale(coil.cost, Expr::Var(pq[0])));
            kcl[coil.positive] = pair_add(&kcl[coil.positive], &pair_scale(&i, sign));
            if let Some(n) = coil.negative {
                kcl[n] = pair_add(&kcl[n], &pair_scale(&i, -sign));
            }
            total = pair_add(&total, &i);
            cols.current.push(i);
            cols.power.push(pq);
        }
        if let Some(cap) = dev.neutral_current_max {
            b.norm(
                &format!("{:?}:{}:neutral_cap", dev.kind, dev.identity),
                &total,
                cap,
            )?;
        }
        devices.push(cols);
    }
    for limit in &prep.voltage_limits {
        let u = difference(&voltage, limit.positive, limit.negative);
        if let Some(cap) = limit.upper {
            b.norm(&format!("voltage:{}:upper", limit.identity), &u, cap)?;
        }
        if let Some(floor) = limit.lower.filter(|x| *x > 0.0) {
            let a = u[0].scale(1.0 / floor);
            let c = u[1].scale(1.0 / floor);
            b.row(
                format!("voltage:{}:lower", limit.identity),
                sum(vec![product(&a, &a), product(&c, &c)]),
                1.0,
                f64::INFINITY,
            );
        }
    }
    for (k, t) in prep.terminals.iter().enumerate() {
        if !t.grounded {
            for (p, a) in kcl[k].iter().enumerate() {
                b.affine_row(format!("kcl:{}:{}:{p}", t.bus, t.terminal), a)?;
            }
        }
    }
    let n = b.start.len();
    if n == 0 {
        return Err("IVR model has no decision variables".into());
    }
    let problem = NlProblem::from_expressions(NlProblemParts {
        minimize: true,
        objective: sum(objective),
        obj_constant: 0.0,
        constraints: b.rows,
        x_l: vec![f64::NEG_INFINITY; n],
        x_u: vec![f64::INFINITY; n],
        x0: b.start,
        g_l: b.lo,
        g_u: b.hi,
        var_names: b.names,
        con_names: b.row_names,
    })?;
    Ok(Model {
        prep,
        voltage,
        branch_current,
        branch_power,
        devices,
        problem,
    })
}
