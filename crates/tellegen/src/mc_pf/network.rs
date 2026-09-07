//! Terminal indexing and passive multiconductor network preparation.

use std::collections::{BTreeMap, BTreeSet};

use num_complex::Complex64;
use powerio_dist::{DistLineCode, MulticonductorNetwork};
use powerio_prob::McAcPfInstance;

use super::linear::RetainedComplexLu;
use super::loads::{prepare_load, BranchLoad};
use super::transformer::prepare_transformer;

#[derive(Clone, Debug)]
pub(crate) struct NetworkIndex {
    pub terminal_ids: Vec<(String, String)>,
    positions: BTreeMap<(String, String), usize>,
}

impl NetworkIndex {
    pub(crate) fn new(network: &MulticonductorNetwork) -> Result<Self, String> {
        let mut terminal_ids = Vec::new();
        let mut positions = BTreeMap::new();
        for bus in network.buses() {
            if bus.terminals.is_empty() {
                return Err(format!("bus `{}` has no terminals", bus.id));
            }
            for terminal in &bus.terminals {
                let key = (bus.id.clone(), terminal.clone());
                if positions.insert(key.clone(), terminal_ids.len()).is_some() {
                    return Err(format!("duplicate terminal identity {}:{terminal}", bus.id));
                }
                terminal_ids.push(key);
            }
            for grounded in &bus.grounded {
                if !positions.contains_key(&(bus.id.clone(), grounded.clone())) {
                    return Err(format!(
                        "bus `{}` grounds unknown terminal `{grounded}`",
                        bus.id
                    ));
                }
            }
        }
        Ok(Self {
            terminal_ids,
            positions,
        })
    }

    pub(crate) fn resolve(&self, identity: (&String, &str)) -> Result<usize, String> {
        self.positions
            .get(&(identity.0.clone(), identity.1.to_owned()))
            .copied()
            .ok_or_else(|| format!("unknown terminal identity {}:{}", identity.0, identity.1))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StampedMatrix {
    pub(crate) values: BTreeMap<(usize, usize), Complex64>,
}

impl StampedMatrix {
    pub(crate) fn zeros(_n: usize) -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
    pub(crate) fn add(&mut self, row: usize, col: usize, value: Complex64) {
        if value.norm() == 0.0 {
            return;
        }
        let entry = self.values.entry((row, col)).or_default();
        *entry += value;
        if entry.norm() == 0.0 {
            self.values.remove(&(row, col));
        }
    }
    pub(crate) fn row_mul(&self, row: usize, vector: &[Complex64]) -> Complex64 {
        self.values
            .range((row, 0)..=(row, usize::MAX))
            .map(|((_, c), value)| *value * vector[*c])
            .sum()
    }
    pub(crate) fn row_fixed_mul(&self, row: usize, fixed: &[Option<Complex64>]) -> Complex64 {
        self.values
            .range((row, 0)..=(row, usize::MAX))
            .filter_map(|((_, c), value)| fixed[*c].map(|v| *value * v))
            .sum()
    }
}

fn stamp_local_values(
    y: &mut StampedMatrix,
    terminals: &[usize],
    local: &[Vec<Complex64>],
) -> Result<(), String> {
    if local.len() != terminals.len() || local.iter().any(|row| row.len() != terminals.len()) {
        return Err("component primitive and terminal map dimensions differ".to_owned());
    }
    for (r, &i) in terminals.iter().enumerate() {
        for (c, &j) in terminals.iter().enumerate() {
            y.add(i, j, local[r][c]);
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct DenseMatrix {
    n: usize,
    values: Vec<Complex64>,
}
impl DenseMatrix {
    fn zeros(n: usize) -> Self {
        Self {
            n,
            values: vec![Complex64::new(0.0, 0.0); n * n],
        }
    }
    fn add(&mut self, row: usize, col: usize, value: Complex64) {
        self.values[row * self.n + col] += value;
    }
    fn get(&self, row: usize, col: usize) -> Complex64 {
        self.values[row * self.n + col]
    }
}

#[derive(Debug)]
pub(crate) struct PreparedNetwork {
    pub(crate) index: NetworkIndex,
    pub(crate) passive: StampedMatrix,
    pub(crate) yref: StampedMatrix,
    pub(crate) fixed: Vec<Option<Complex64>>,
    pub(crate) unknown: Vec<usize>,
    pub(crate) loads: Vec<BranchLoad>,
    pub(crate) elements: Vec<PreparedElement>,
    pub(crate) factor: Option<RetainedComplexLu>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedElement {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) terminals: Vec<usize>,
    pub(crate) yprim: Vec<Vec<Complex64>>,
}

impl PreparedNetwork {
    pub(crate) fn prepare(instance: &McAcPfInstance) -> Result<Self, String> {
        let network = instance.network();
        if !instance.isolated_terminals().is_empty() {
            return Err(
                "isolated terminals are not supported by the fixed-point PF snapshot".to_owned(),
            );
        }
        if !instance.control_modes().is_empty() {
            return Err("active regulator/capacitor controls are unsupported; solve a fixed operating snapshot".to_owned());
        }
        if !network.untyped_objects().is_empty() {
            let names = network
                .untyped_objects()
                .iter()
                .map(|o| format!("{}.{},", o.class, o.name))
                .collect::<String>();
            return Err(format!(
                "network retains unsupported untyped active objects: {names}"
            ));
        }
        let index = NetworkIndex::new(network)?;
        let n = index.terminal_ids.len();
        let mut passive = StampedMatrix::zeros(n);
        let mut yref = StampedMatrix::zeros(n);
        let mut elements = Vec::new();
        let mut fixed = vec![None; n];
        let mut prescribed_source_terminals = BTreeSet::new();
        for bus in network.buses() {
            for terminal in &bus.grounded {
                fixed[index.resolve((&bus.id, terminal))?] = Some(Complex64::new(0.0, 0.0));
            }
        }

        // Ideal sources are exact Dirichlet terminals.  A second source may
        // share a terminal only when it prescribes the same complex voltage.
        for source in instance.sources() {
            if source.terminals.len() != source.v_magnitude.len()
                || source.terminals.len() != source.v_angle.len()
            {
                return Err(format!(
                    "source `{}` terminal and voltage dimensions are inconsistent",
                    source.source
                ));
            }
            for ((terminal, &mag), &angle) in source
                .terminals
                .iter()
                .zip(&source.v_magnitude)
                .zip(&source.v_angle)
            {
                if !mag.is_finite() || !angle.is_finite() || mag < 0.0 {
                    return Err(format!(
                        "source `{}` has a non-finite or negative prescribed voltage",
                        source.source
                    ));
                }
                let source_row = network
                    .sources()
                    .iter()
                    .find(|s| s.name == source.source)
                    .ok_or_else(|| {
                        format!(
                            "instance source `{}` is absent from its network",
                            source.source
                        )
                    })?;
                // The v0.11 typed source is an ideal prescribed source.  A
                // finite source impedance retained in raw extras must not be
                // silently discarded by this ideal-terminal solver.
                for key in [
                    "r1", "x1", "r0", "x0", "r2", "x2", "isc3", "isc1", "mvasc", "mvasc3",
                    "mvasc1", "z_source", "z1", "z0", "puz1", "puz0", "bus2",
                ] {
                    if source_row.extras.contains_key(key) {
                        return Err(format!("source `{}` carries finite impedance metadata `{key}`; normalize it to a Norton source before MC PF", source.source));
                    }
                }
                let i = index.resolve((&source_row.bus, terminal))?;
                if !prescribed_source_terminals.insert(i) {
                    return Err(format!(
                        "multiple ideal sources prescribe terminal `{}`:{}; reaction-current allocation is indeterminate",
                        source_row.bus, terminal
                    ));
                }
                let value = Complex64::from_polar(mag, angle);
                if let Some(previous) = fixed[i] {
                    if (previous - value).norm() > 1e-8 * (1.0 + value.norm()) {
                        return Err(format!(
                            "conflicting ideal source prescriptions at terminal `{terminal}`"
                        ));
                    }
                } else {
                    fixed[i] = Some(value);
                }
            }
        }

        for line in network.lines() {
            let code = network
                .line_codes()
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&line.linecode))
                .ok_or_else(|| {
                    format!(
                        "line `{}` references unresolved linecode `{}`",
                        line.name, line.linecode
                    )
                })?;
            elements.push(stamp_line(&mut passive, line, code, &index)?);
        }
        for switch in network.switches() {
            if !switch.open {
                return Err(format!(
                    "closed switch `{}` is unsupported by the fixed linear snapshot",
                    switch.name
                ));
            }
        }
        for shunt in network.shunts() {
            if shunt.g.len() != shunt.b.len()
                || shunt.g.iter().any(|r| r.len() != shunt.g.len())
                || shunt.b.iter().any(|r| r.len() != shunt.b.len())
                || shunt.g.iter().flatten().any(|value| !value.is_finite())
                || shunt.b.iter().flatten().any(|value| !value.is_finite())
                || shunt.terminal_map.len() != shunt.g.len()
            {
                return Err(format!(
                    "shunt `{}` has inconsistent matrix dimensions",
                    shunt.name
                ));
            }
            let map: Vec<_> = shunt
                .terminal_map
                .iter()
                .map(|t| index.resolve((&shunt.bus, t)))
                .collect::<Result<_, _>>()?;
            for (r, &i) in map.iter().enumerate() {
                for (c, &j) in map.iter().enumerate() {
                    passive.add(i, j, Complex64::new(shunt.g[r][c], shunt.b[r][c]));
                }
            }
            elements.push(PreparedElement {
                name: shunt.name.clone(),
                kind: "shunt".to_owned(),
                terminals: map,
                yprim: shunt
                    .g
                    .iter()
                    .zip(&shunt.b)
                    .map(|(gr, br)| {
                        gr.iter()
                            .zip(br)
                            .map(|(&g, &b)| Complex64::new(g, b))
                            .collect()
                    })
                    .collect(),
            });
        }
        for capacitor in network.capacitors() {
            if !capacitor.q_rated.is_finite()
                || capacitor.v_nom <= 0.0
                || capacitor.terminal_map.len() < 2
            {
                return Err(format!(
                    "capacitor `{}` has invalid fixed shunt data",
                    capacitor.name
                ));
            }
            let map: Vec<_> = capacitor
                .terminal_map
                .iter()
                .map(|t| index.resolve((&capacitor.bus, t)))
                .collect::<Result<_, _>>()?;
            let mut yprim = vec![vec![Complex64::default(); map.len()]; map.len()];
            let mut add_local_couple = |a: usize, b: usize, susceptance: f64| {
                let value = Complex64::new(0.0, susceptance);
                yprim[a][a] += value;
                yprim[b][b] += value;
                yprim[a][b] -= value;
                yprim[b][a] -= value;
            };
            match capacitor.configuration {
                powerio_dist::Configuration::SinglePhase => {
                    if map.len() != 2 {
                        return Err(format!(
                            "single-phase capacitor `{}` requires two terminals",
                            capacitor.name
                        ));
                    }
                    stamp_couple(
                        &mut passive,
                        map[0],
                        map[1],
                        capacitor.q_rated / capacitor.v_nom.powi(2),
                    );
                    add_local_couple(0, 1, capacitor.q_rated / capacitor.v_nom.powi(2));
                }
                powerio_dist::Configuration::Wye => {
                    if map.len() != 4 {
                        return Err(format!(
                            "wye capacitor `{}` requires exactly three phase terminals followed by an explicit neutral",
                            capacitor.name
                        ));
                    }
                    let b = capacitor.q_rated
                        / ((map.len() - 1) as f64)
                        / (capacitor.v_nom / 3f64.sqrt()).powi(2);
                    for &phase in &map[..map.len() - 1] {
                        stamp_couple(&mut passive, phase, *map.last().unwrap(), b);
                        add_local_couple(phase, map.len() - 1, b);
                    }
                }
                powerio_dist::Configuration::Delta => {
                    if map.len() < 2 {
                        return Err(format!(
                            "delta capacitor `{}` maps fewer than two terminals",
                            capacitor.name
                        ));
                    }
                    let loops = if map.len() == 2 { 1 } else { map.len() };
                    let b = capacitor.q_rated / loops as f64 / capacitor.v_nom.powi(2);
                    for a in 0..loops {
                        stamp_couple(&mut passive, map[a], map[(a + 1) % map.len()], b);
                        add_local_couple(a, (a + 1) % map.len(), b);
                    }
                }
                _ => {
                    return Err(format!(
                        "capacitor `{}` uses unsupported configuration",
                        capacitor.name
                    ))
                }
            }
            elements.push(PreparedElement {
                name: capacitor.name.clone(),
                kind: "capacitor".to_owned(),
                terminals: map,
                yprim,
            });
        }
        for transformer in network.transformers() {
            let primitive = prepare_transformer(transformer).map_err(|e| e.to_string())?;
            for grounded in &primitive.grounded_terminals {
                let i = index.resolve((&grounded.bus, &grounded.terminal))?;
                if let Some(previous) = fixed[i] {
                    if previous.norm() > 1e-12 {
                        return Err(format!(
                            "solid transformer neutral `{}`:{} conflicts with a prescribed voltage",
                            grounded.bus, grounded.terminal
                        ));
                    }
                } else {
                    fixed[i] = Some(Complex64::new(0.0, 0.0));
                }
            }
            let terminals = primitive
                .terminals
                .iter()
                .map(|t| index.resolve((&t.bus, &t.terminal)))
                .collect::<Result<Vec<_>, _>>()?;
            stamp_local_values(&mut passive, &terminals, &primitive.y_prim)?;
            elements.push(PreparedElement {
                name: primitive.name.clone(),
                kind: "transformer".to_owned(),
                terminals,
                yprim: primitive.y_prim,
            });
        }
        if !network.generators().is_empty() || !network.ibrs().is_empty() {
            return Err(
                "fixed-point MC PF does not yet support generator or IBR injections".to_owned(),
            );
        }
        let loads = instance
            .loads()
            .iter()
            .map(|l| {
                let source = network
                    .loads()
                    .iter()
                    .find(|x| x.name == l.load)
                    .ok_or_else(|| {
                        format!("instance load `{}` is absent from its network", l.load)
                    })?;
                let mut effective = source.clone();
                // The instance is the calculation boundary.  Its prescribed
                // operating powers may be edited without mutating the network.
                effective.p_nom = l.p_w.clone();
                effective.q_nom = l.q_var.clone();
                effective.terminal_map = l.terminals.clone();
                effective.voltage_model = l.voltage_model.clone();
                prepare_load(&effective, &index)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for load in &loads {
            load.stamp_yref(&mut yref);
        }

        let unknown: Vec<_> = (0..n).filter(|&i| fixed[i].is_none()).collect();
        let mut position = vec![usize::MAX; n];
        for (u, &i) in unknown.iter().enumerate() {
            position[i] = u;
        }
        let mut triplets_by_global = BTreeMap::<(usize, usize), Complex64>::new();
        for (&key, &value) in &passive.values {
            *triplets_by_global.entry(key).or_default() += value;
        }
        for (&key, &value) in &yref.values {
            *triplets_by_global.entry(key).or_default() += value;
        }
        let triplets = triplets_by_global
            .into_iter()
            .filter_map(|((i, j), value)| {
                let r = position[i];
                let c = position[j];
                (r != usize::MAX && c != usize::MAX && value.norm() > 0.0).then_some((r, c, value))
            })
            .collect::<Vec<_>>();
        let factor = if unknown.is_empty() {
            None
        } else {
            Some(RetainedComplexLu::factor(unknown.len(), &triplets)?)
        };
        Ok(Self {
            index,
            passive,
            yref,
            fixed,
            unknown,
            loads,
            elements,
            factor,
        })
    }

    pub(crate) fn fixed_voltage_vector(&self) -> Vec<Complex64> {
        self.fixed.iter().map(|x| x.unwrap_or_default()).collect()
    }
    pub(crate) fn source_rhs(&self) -> Vec<Complex64> {
        // The retained matrix is Ypassive + Yref and compensation is evaluated
        // on the full voltage vector.  Eliminate fixed terminals with the full
        // Yuf block so the fixed Yref coupling cancels exactly in the RHS.
        self.unknown
            .iter()
            .map(|&i| {
                -(self.passive.row_fixed_mul(i, &self.fixed)
                    + self.yref.row_fixed_mul(i, &self.fixed))
            })
            .collect()
    }
}

fn stamp_line(
    passive: &mut StampedMatrix,
    line: &powerio_dist::DistLine,
    code: &DistLineCode,
    index: &NetworkIndex,
) -> Result<PreparedElement, String> {
    let n = code.n_conductors;
    if n == 0
        || line.terminal_map_from.len() != n
        || line.terminal_map_to.len() != n
        || line.length <= 0.0
        || !line.length.is_finite()
    {
        return Err(format!(
            "line `{}` has invalid conductor dimensions or length",
            line.name
        ));
    }
    let square = |m: &[Vec<f64>], name: &str| -> Result<(), String> {
        if m.len() != n
            || m.iter()
                .any(|r| r.len() != n || r.iter().any(|v| !v.is_finite()))
        {
            Err(format!(
                "linecode `{}` has invalid {name} matrix",
                code.name
            ))
        } else {
            Ok(())
        }
    };
    square(&code.r_series, "r_series")?;
    square(&code.x_series, "x_series")?;
    square(&code.g_from, "g_from")?;
    square(&code.b_from, "b_from")?;
    square(&code.g_to, "g_to")?;
    square(&code.b_to, "b_to")?;
    let mut z = DenseMatrix::zeros(n);
    let mut yprim = vec![vec![Complex64::default(); 2 * n]; 2 * n];
    for r in 0..n {
        for c in 0..n {
            z.add(
                r,
                c,
                Complex64::new(
                    code.r_series[r][c] * line.length,
                    code.x_series[r][c] * line.length,
                ),
            );
        }
    }
    let zinv = invert_dense(&z)?;
    let from: Vec<_> = line
        .terminal_map_from
        .iter()
        .map(|t| index.resolve((&line.bus_from, t)))
        .collect::<Result<_, _>>()?;
    let to: Vec<_> = line
        .terminal_map_to
        .iter()
        .map(|t| index.resolve((&line.bus_to, t)))
        .collect::<Result<_, _>>()?;
    for r in 0..n {
        for c in 0..n {
            let y = zinv.get(r, c);
            let yf = Complex64::new(
                code.g_from[r][c] * line.length,
                code.b_from[r][c] * line.length,
            );
            let yt = Complex64::new(code.g_to[r][c] * line.length, code.b_to[r][c] * line.length);
            passive.add(from[r], from[c], y + yf);
            passive.add(to[r], to[c], y + yt);
            passive.add(from[r], to[c], -y);
            passive.add(to[r], from[c], -y);
            yprim[r][c] += y + yf;
            yprim[n + r][n + c] += y + yt;
            yprim[r][n + c] -= y;
            yprim[n + r][c] -= y;
        }
    }
    let mut terminals = from;
    terminals.extend(to);
    Ok(PreparedElement {
        name: line.name.clone(),
        kind: "line".to_owned(),
        terminals,
        yprim,
    })
}

fn stamp_couple(y: &mut StampedMatrix, a: usize, b: usize, susceptance: f64) {
    let value = Complex64::new(0.0, susceptance);
    y.add(a, a, value);
    y.add(b, b, value);
    y.add(a, b, -value);
    y.add(b, a, -value);
}

fn invert_dense(a: &DenseMatrix) -> Result<DenseMatrix, String> {
    let n = a.n;
    let mut m = a.values.clone();
    let mut out = DenseMatrix::zeros(n);
    for i in 0..n {
        out.add(i, i, Complex64::new(1.0, 0.0));
    }
    for k in 0..n {
        let mut pivot = k;
        for r in k..n {
            if m[r * n + k].norm() > m[pivot * n + k].norm() {
                pivot = r;
            }
        }
        if m[pivot * n + k].norm() <= 1e-14 {
            return Err("line series impedance matrix is singular".to_owned());
        }
        if pivot != k {
            for c in 0..n {
                m.swap(k * n + c, pivot * n + c);
                out.values.swap(k * n + c, pivot * n + c);
            }
        }
        let p = m[k * n + k];
        for c in 0..n {
            m[k * n + c] /= p;
            out.values[k * n + c] /= p;
        }
        let pivot_row = m[k * n..(k + 1) * n].to_vec();
        let pivot_inverse = out.values[k * n..(k + 1) * n].to_vec();
        for r in 0..n {
            if r != k {
                let q = m[r * n + k];
                for c in 0..n {
                    m[r * n + c] -= q * pivot_row[c];
                    out.values[r * n + c] -= q * pivot_inverse[c];
                }
            }
        }
    }
    Ok(out)
}
