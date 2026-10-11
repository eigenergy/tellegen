//! IVR polynomial construction. Only this layer knows about POUNCE expressions.
use pounce_nl::nl_reader::{BinOp, CmpOp, Expr, NlProblem, NlProblemParts, UnaryOp};
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
fn descriptor(row: &powerio_matrix::McOpfComplexRow, voltage: &[Pair], currents: &[Pair]) -> Pair {
    let mut out = [Affine::default(), Affine::default()];
    for (terms, values) in [(&row.voltage, voltage), (&row.current, currents)] {
        for &(k, [r, i]) in terms {
            out[0] = out[0]
                .add(&values[k][0].scale(r))
                .sub(&values[k][1].scale(i));
            out[1] = out[1]
                .add(&values[k][1].scale(r))
                .add(&values[k][0].scale(i));
        }
    }
    out
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
    pub transformer_tap: Vec<Option<usize>>,
    pub transformer_current: Vec<Vec<Pair>>,
    pub transformer_power: Vec<Vec<Option<[usize; 2]>>>,
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
    fn droop(&mut self, name: &str, voltage: &[Pair], curve: &powerio_matrix::McOpfDroop) -> Expr {
        let mut magnitudes = Vec::new();
        for (j, &(p, q)) in curve.monitors.iter().enumerate() {
            let u = difference(voltage, p, q);
            let start = u[0].value(&self.start).hypot(u[1].value(&self.start));
            let m = self.var(format!("{name}:magnitude:{j}"), start);
            self.row(
                format!("{name}:magnitude:{j}:nonnegative"),
                Expr::Var(m),
                0.0,
                f64::INFINITY,
            );
            self.row(
                format!("{name}:magnitude:{j}:definition"),
                sum(vec![
                    product(&u[0], &u[0]),
                    product(&u[1], &u[1]),
                    scale(-1.0, product(&Affine::var(m), &Affine::var(m))),
                ]),
                0.0,
                0.0,
            );
            magnitudes.push(Expr::Var(m));
        }
        let mean = scale(1.0 / (magnitudes.len() as f64), sum(magnitudes));
        let mut out = vec![Expr::Const(curve.values[0])];
        for j in 0..curve.knots.len() - 1 {
            let slope =
                (curve.values[j + 1] - curve.values[j]) / (curve.knots[j + 1] - curve.knots[j]);
            if slope == 0.0 {
                continue;
            }
            for (k, sign) in [(j, 1.0), (j + 1, -1.0)] {
                let z = scale(
                    1.0 / curve.epsilon,
                    bin(BinOp::Sub, mean.clone(), Expr::Const(curve.knots[k])),
                );
                out.push(scale(sign * slope * curve.epsilon, softplus(z)));
            }
        }
        sum(out)
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
    let mut transformer_tap = Vec::new();
    let mut transformer_current = Vec::new();
    let mut transformer_power = Vec::new();
    for tx in &prep.transformers {
        let tap = tx.tap_bounds.map(|[lo, hi]| {
            let id = b.var(
                format!("transformer:{}:tap", tx.identity),
                1.0_f64.clamp(lo, hi),
            );
            b.row(
                format!("transformer:{}:tap_bounds", tx.identity),
                Expr::Var(id),
                lo,
                hi,
            );
            id
        });
        transformer_tap.push(tap);
        let currents: Vec<_> = tx
            .coils
            .iter()
            .enumerate()
            .map(|(k, _)| b.pair(&format!("transformer:{}:{k}", tx.identity), [0.0, 0.0]))
            .collect();
        for (k, row) in tx.equations.iter().enumerate() {
            let value = descriptor(row, &voltage, &currents);
            for (j, a) in value.iter().enumerate() {
                let name = format!("transformer:{}:equation:{k}:{j}", tx.identity);
                if let Some(tap) = tap {
                    let dynamic = descriptor(&tx.tap_equations[k], &voltage, &currents);
                    b.row(
                        name,
                        sum(vec![a.expr(), product(&Affine::var(tap), &dynamic[j])]),
                        0.0,
                        0.0,
                    );
                } else {
                    b.affine_row(name, a)?;
                }
            }
        }
        for (coil, i) in tx.coils.iter().zip(&currents) {
            kcl[coil.positive] = pair_add(&kcl[coil.positive], &pair_scale(i, -1.0));
            if let Some(n) = coil.negative {
                kcl[n] = pair_add(&kcl[n], i);
            }
        }
        let mut powers = Vec::new();
        for port in &tx.ports {
            let name = format!("transformer:{}:{}", tx.identity, port.name);
            let i = descriptor(&port.current, &voltage, &currents);
            let u = difference(&voltage, port.positive, port.negative);
            if let Some(cap) = port.current_max {
                b.norm(&format!("{name}:current"), &i, cap)?;
            }
            let ids = if let Some(cap) = port.apparent_max {
                let ids = b.power_lift(&name, &u, &i, [0.0, 0.0]);
                b.norm(
                    &format!("{name}:apparent"),
                    &[Affine::var(ids[0]), Affine::var(ids[1])],
                    cap,
                )?;
                Some(ids)
            } else {
                None
            };
            powers.push(ids);
        }
        transformer_current.push(currents);
        transformer_power.push(powers);
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
            let zero_power = coil.current_max == Some(0.0) || coil.apparent_max == Some(0.0);
            if let Some(law) = &coil.load_law {
                let impedance = law.terms.iter().flatten().all(|t| t[1] == 2.0);
                if impedance {
                    // Preserve the affine constitutive law even at U = 0.
                    let g: f64 =
                        law.terms[0].iter().map(|t| t[0]).sum::<f64>() / law.nominal.powi(2);
                    let q: f64 =
                        law.terms[1].iter().map(|t| t[0]).sum::<f64>() / law.nominal.powi(2);
                    b.affine_row(
                        format!("{name}:impedance:r"),
                        &i[0].sub(&u[0].scale(g)).sub(&u[1].scale(q)),
                    )?;
                    b.affine_row(
                        format!("{name}:impedance:i"),
                        &i[1].sub(&u[1].scale(g)).add(&u[0].scale(q)),
                    )?;
                } else if law.terms.iter().flatten().all(|t| t[1] == 0.0) {
                    for (j, &power_column) in pq.iter().enumerate() {
                        let target: f64 = law.terms[j].iter().map(|t| t[0]).sum();
                        b.row(
                            format!("{name}:constant:{j}"),
                            Expr::Var(power_column),
                            target,
                            target,
                        );
                    }
                } else {
                    // Log-voltage lift defines the exact positive-voltage domain;
                    // there is no artificial epsilon floor or fractional power at zero.
                    if u.iter().all(|a| a.terms.is_empty()) && u2 == 0.0 {
                        return Err(format!(
                            "{name}: voltage-dependent law at fixed zero voltage"
                        ));
                    }
                    let ell = b.var(
                        format!("{name}:log_voltage"),
                        (u2.sqrt().max(law.nominal * 0.5) / law.nominal).ln(),
                    );
                    let exp = |exponent| {
                        Expr::Unary(UnaryOp::Exp, Box::new(scale(exponent, Expr::Var(ell))))
                    };
                    let un = pair_scale(&u, 1.0 / law.nominal);
                    b.row(
                        format!("{name}:positive_voltage"),
                        bin(
                            BinOp::Sub,
                            sum(vec![product(&un[0], &un[0]), product(&un[1], &un[1])]),
                            exp(2.0),
                        ),
                        0.0,
                        0.0,
                    );
                    for (j, &power_column) in pq.iter().enumerate() {
                        let rhs = sum(law.terms[j]
                            .iter()
                            .map(|t| {
                                if t[1] == 0.0 {
                                    Expr::Const(t[0])
                                } else {
                                    scale(t[0], exp(t[1]))
                                }
                            })
                            .collect());
                        b.row(
                            format!("{name}:voltage_law:{j}"),
                            bin(BinOp::Sub, Expr::Var(power_column), rhs),
                            0.0,
                            0.0,
                        );
                    }
                }
            } else if let Some(pq0) = coil.prescribed {
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
                    if zero_power {
                        if lo.is_some_and(|v| v > 0.0) || hi.is_some_and(|v| v < 0.0) {
                            return Err(format!("{name}: zero rating conflicts with capability"));
                        }
                    } else if lo.is_some() || hi.is_some() {
                        b.row(
                            format!("{name}:capability:{j}"),
                            Expr::Var(pq[j]),
                            lo.unwrap_or(f64::NEG_INFINITY),
                            hi.unwrap_or(f64::INFINITY),
                        );
                    }
                }
            }
            if let Some(slope) = coil.reactive_slope.filter(|_| !zero_power) {
                b.affine_row(
                    format!("{name}:power_factor"),
                    &Affine::var(pq[1]).sub(&Affine::var(pq[0]).scale(slope)),
                )?;
            }
            for (family, curve, column, upper) in [
                ("volt_var", coil.volt_var.as_ref(), pq[1], false),
                ("volt_watt", coil.volt_watt.as_ref(), pq[0], true),
            ] {
                if let Some(curve) =
                    curve.filter(|c| !zero_power || c.values.iter().any(|v| *v != 0.0))
                {
                    let target = b.droop(&format!("{name}:{family}"), &voltage, curve);
                    b.row(
                        format!("{name}:{family}"),
                        bin(BinOp::Sub, Expr::Var(column), target),
                        if upper { f64::NEG_INFINITY } else { 0.0 },
                        0.0,
                    );
                }
            }
            if let Some(cap) = coil.current_max {
                b.norm(&format!("{name}:current_cap"), &i, cap)?;
            }
            if let Some(cap) = coil.apparent_max.filter(|_| coil.current_max != Some(0.0)) {
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
        if let Some(cap) = dev
            .neutral_current_max
            .filter(|_| !dev.coils.iter().all(|c| c.current_max == Some(0.0)))
        {
            b.norm(
                &format!("{:?}:{}:neutral_cap", dev.kind, dev.identity),
                &total,
                cap,
            )?;
        }
        if let Some([lo, hi]) = dev.net_active_bounds {
            if dev
                .coils
                .iter()
                .all(|c| c.current_max == Some(0.0) || c.apparent_max == Some(0.0))
            {
                if lo > 0.0 || hi < 0.0 {
                    return Err(format!(
                        "{}: zero rating conflicts with DC-link power",
                        dev.identity
                    ));
                }
            } else {
                b.row(
                    format!("Ibr:{}:dc_link", dev.identity),
                    sum(cols.power.iter().map(|pq| Expr::Var(pq[0])).collect()),
                    lo,
                    hi,
                );
            }
        }
        devices.push(cols);
    }
    for limit in &prep.voltage_limits {
        let u = if limit.combination.is_empty() {
            difference(&voltage, limit.positive, limit.negative)
        } else {
            let sequence = descriptor(
                &powerio_matrix::McOpfComplexRow {
                    voltage: limit.combination.clone(),
                    current: Vec::new(),
                },
                &voltage,
                &[],
            );
            // Lift the sequence voltage before squaring. This avoids cancellation
            // between expanded Fortescue cross terms in the backend's conservative
            // quadratic recognizer and keeps the defining rows affine.
            let lifted = b.pair(
                &format!("sequence:{}", limit.identity),
                [sequence[0].value(&b.start), sequence[1].value(&b.start)],
            );
            for k in 0..2 {
                b.affine_row(
                    format!("sequence:{}:{k}", limit.identity),
                    &lifted[k].sub(&sequence[k]),
                )?;
            }
            lifted
        };
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
    for angle in &prep.angle_limits {
        let first = &voltage[angle.first];
        let second = &voltage[angle.second];
        if [first, second]
            .iter()
            .any(|u| u.iter().all(|a| a.terms.is_empty() && a.constant == 0.0))
        {
            continue;
        }
        let rotate = |theta: f64| {
            [
                first[0]
                    .scale(theta.cos())
                    .add(&first[1].scale(theta.sin())),
                first[1]
                    .scale(theta.cos())
                    .sub(&first[0].scale(theta.sin())),
            ]
        };
        let lower = power(&rotate(angle.offset + angle.lower), second)[1].clone();
        if angle.lower == angle.upper {
            b.row(
                format!("angle:{}:equality", angle.identity),
                lower,
                0.0,
                0.0,
            );
            b.row(
                format!("angle:{}:domain", angle.identity),
                power(&rotate(angle.offset), second)[0].clone(),
                0.0,
                f64::INFINITY,
            );
        } else {
            b.row(
                format!("angle:{}:lower", angle.identity),
                lower,
                0.0,
                f64::INFINITY,
            );
            b.row(
                format!("angle:{}:upper", angle.identity),
                power(&rotate(angle.offset + angle.upper), second)[1].clone(),
                f64::NEG_INFINITY,
                0.0,
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
        transformer_tap,
        transformer_current,
        transformer_power,
        devices,
        problem,
    })
}

pub(super) fn softplus(z: Expr) -> Expr {
    // Stable log(1+exp(z)) = max(z,0)+log(1+exp(-abs(z))).
    // The branch arguments remain finite even if a tape evaluates both.
    let abs = Expr::Cond {
        cond: Box::new(Expr::Compare(
            CmpOp::Ge,
            Box::new(z.clone()),
            Box::new(Expr::Const(0.0)),
        )),
        then_: Box::new(z.clone()),
        else_: Box::new(scale(-1.0, z.clone())),
    };
    let positive = Expr::Cond {
        cond: Box::new(Expr::Compare(
            CmpOp::Ge,
            Box::new(z.clone()),
            Box::new(Expr::Const(0.0)),
        )),
        then_: Box::new(z),
        else_: Box::new(Expr::Const(0.0)),
    };
    let smooth = Expr::Unary(
        UnaryOp::Log,
        Box::new(sum(vec![
            Expr::Const(1.0),
            Expr::Unary(UnaryOp::Exp, Box::new(scale(-1.0, abs))),
        ])),
    );
    sum(vec![positive, smooth])
}
