//! Equipment edits on a balanced case: add a generator, load, shunt or line,
//! move one to another bus, change its values, take it out. The bus types
//! follow (a bus that gains its first generator regulates voltage, one that
//! loses its last turns PQ), a line is not removed when that splits the
//! network, and every edit reports how the PSS/E machine ids of the case
//! changed, so the DYR records that name units by `(bus, id)` can follow
//! ([`crate::rms::dyr_rekey`]).
//!
//! Loads and shunts are named by their row in the case (0-based), lines and
//! transformers by the 1-based row id the drawing uses. A new bus starts
//! isolated and turns PQ when a line or transformer connects it.

use powerio::{BalancedNetwork, Branch, Bus, BusId, BusType, Generator, Load, Location, Shunt};
use serde::{Deserialize, Serialize};

/// One edit; units are named by `(bus, machine id)` as in DYR records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CaseEdit {
    /// A new generator at `bus`. Omitted values follow `pg`: `pmax` twice
    /// it (at least 100 MW), `qmax`/`qmin` ±half of `pmax`, `mbase` the
    /// rating at 0.9 power factor; `vg` is the bus voltage.
    AddGenerator {
        bus: usize,
        #[serde(default = "default_pg")]
        pg: f64,
        #[serde(default)]
        pmax: Option<f64>,
        #[serde(default)]
        qmax: Option<f64>,
        #[serde(default)]
        qmin: Option<f64>,
        #[serde(default)]
        vg: Option<f64>,
        #[serde(default)]
        mbase: Option<f64>,
    },
    MoveGenerator {
        bus: usize,
        id: String,
        to: usize,
    },
    RemoveGenerator {
        bus: usize,
        id: String,
    },
    SetGenerator {
        bus: usize,
        id: String,
        #[serde(default)]
        pg: Option<f64>,
        #[serde(default)]
        pmax: Option<f64>,
        #[serde(default)]
        qmax: Option<f64>,
        #[serde(default)]
        qmin: Option<f64>,
        #[serde(default)]
        vg: Option<f64>,
        #[serde(default)]
        mbase: Option<f64>,
        #[serde(default)]
        in_service: Option<bool>,
    },
    /// A new load at `bus`; `q` defaults to 0.3 × `p` (power factor 0.96).
    AddLoad {
        bus: usize,
        #[serde(default = "default_load_p")]
        p: f64,
        #[serde(default)]
        q: Option<f64>,
    },
    SetLoad {
        index: usize,
        #[serde(default)]
        p: Option<f64>,
        #[serde(default)]
        q: Option<f64>,
        #[serde(default)]
        in_service: Option<bool>,
    },
    MoveLoad {
        index: usize,
        to: usize,
    },
    RemoveLoad {
        index: usize,
    },
    /// A new shunt at `bus`, in MW / MVAr at 1 pu (`b` > 0 a capacitor).
    AddShunt {
        bus: usize,
        #[serde(default)]
        g: f64,
        #[serde(default = "default_shunt_b")]
        b: f64,
    },
    SetShunt {
        index: usize,
        #[serde(default)]
        g: Option<f64>,
        #[serde(default)]
        b: Option<f64>,
        #[serde(default)]
        in_service: Option<bool>,
    },
    MoveShunt {
        index: usize,
        to: usize,
    },
    RemoveShunt {
        index: usize,
    },
    /// A new line between two buses, per unit on the system base; omitted
    /// values are those of a short overhead line (r 0.01, x 0.1, b 0.02,
    /// 100 MVA).
    AddLine {
        from: usize,
        to: usize,
        #[serde(default)]
        r: Option<f64>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        b: Option<f64>,
        #[serde(default)]
        rate: Option<f64>,
    },
    SetLine {
        branch: usize,
        #[serde(default)]
        r: Option<f64>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        b: Option<f64>,
        #[serde(default)]
        rate: Option<f64>,
        #[serde(default)]
        in_service: Option<bool>,
    },
    RemoveLine {
        branch: usize,
    },
    /// A two-winding transformer; omitted values: r 0.002, x 0.1 pu, tap
    /// 1.0, no phase shift, 100 MVA.
    AddTransformer {
        from: usize,
        to: usize,
        #[serde(default)]
        r: Option<f64>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        tap: Option<f64>,
        #[serde(default)]
        shift: Option<f64>,
        #[serde(default)]
        rate: Option<f64>,
    },
    /// Tap ratio and phase shift (degrees) of a transformer.
    SetTransformer {
        branch: usize,
        #[serde(default)]
        tap: Option<f64>,
        #[serde(default)]
        shift: Option<f64>,
    },
    /// A new bus: `id` (default: one above the largest), base voltage
    /// (default: that of `near`, else the first bus), a name, and a point
    /// in the case's coordinate space when it has one.
    AddBus {
        #[serde(default)]
        id: Option<usize>,
        #[serde(default)]
        base_kv: Option<f64>,
        #[serde(default)]
        near: Option<usize>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        at: Option<[f64; 2]>,
    },
    SetBus {
        bus: usize,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        base_kv: Option<f64>,
        #[serde(default)]
        at: Option<[f64; 2]>,
    },
    /// Remove a bus with nothing connected to it.
    RemoveBus {
        bus: usize,
    },
}

const fn default_pg() -> f64 {
    50.0
}

const fn default_load_p() -> f64 {
    20.0
}

const fn default_shunt_b() -> f64 {
    10.0
}

/// A unit key, `(bus, machine id)`.
pub type UnitKey = (usize, String);

/// What an edit did to the units of the case.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditOutcome {
    /// The unit the edit added, moved or changed, by its new key.
    pub unit: Option<UnitKey>,
    /// Units whose key changed, `(from, to)` (a move, and the ids a move or
    /// removal shifted on the buses involved).
    pub renames: Vec<(UnitKey, UnitKey)>,
    /// Units that are gone.
    pub removed: Vec<UnitKey>,
    /// The load or shunt row (0-based) or line id (1-based) the edit added,
    /// moved or changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<usize>,
}

/// A load, shunt or line of the case as the editor shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadView {
    pub index: usize,
    pub bus: usize,
    pub p: f64,
    pub q: f64,
    pub in_service: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShuntView {
    pub index: usize,
    pub bus: usize,
    pub g: f64,
    pub b: f64,
    pub in_service: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineView {
    /// 1-based row id, as on the drawing.
    pub id: usize,
    pub from: usize,
    pub to: usize,
    pub r: f64,
    pub x: f64,
    pub b: f64,
    /// Rating A (MVA).
    pub rate: f64,
    pub in_service: bool,
    pub transformer: bool,
    /// Tap ratio and phase shift, for a transformer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tap: Option<TransformerTap>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BusView {
    pub id: usize,
    pub name: Option<String>,
    pub base_kv: f64,
    /// `pq`, `pv`, `ref` or `isolated`.
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformerTap {
    pub tap: f64,
    pub shift: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EquipmentView {
    pub buses: Vec<BusView>,
    pub loads: Vec<LoadView>,
    pub shunts: Vec<ShuntView>,
    pub lines: Vec<LineView>,
}

/// The loads, shunts and lines of a case.
#[must_use]
pub fn case_equipment(net: &BalancedNetwork) -> EquipmentView {
    EquipmentView {
        buses: net
            .buses()
            .iter()
            .map(|b| BusView {
                id: b.id.0,
                name: b.name.clone(),
                base_kv: b.base_kv,
                kind: match b.kind {
                    BusType::Pq => "pq",
                    BusType::Pv => "pv",
                    BusType::Ref => "ref",
                    _ => "isolated",
                }
                .into(),
            })
            .collect(),
        loads: net
            .loads()
            .iter()
            .enumerate()
            .map(|(index, l)| LoadView {
                index,
                bus: l.bus.0,
                p: l.p,
                q: l.q,
                in_service: l.in_service,
            })
            .collect(),
        shunts: net
            .shunts()
            .iter()
            .enumerate()
            .map(|(index, s)| ShuntView {
                index,
                bus: s.bus.0,
                g: s.g,
                b: s.b,
                in_service: s.in_service,
            })
            .collect(),
        lines: net
            .branches()
            .iter()
            .enumerate()
            .map(|(i, br)| LineView {
                id: i + 1,
                from: br.from.0,
                to: br.to.0,
                r: br.r,
                x: br.x,
                b: br.b,
                rate: br.rate_a,
                in_service: br.in_service,
                transformer: br.is_transformer(),
                tap: br.is_transformer().then(|| TransformerTap {
                    tap: br.calc_effective_tap(),
                    shift: br.shift,
                }),
            })
            .collect(),
    }
}

/// Connected groups of buses over the in-service branches.
fn islands(net: &BalancedNetwork) -> usize {
    let ids: Vec<usize> = net.buses().iter().map(|b| b.id.0).collect();
    let at = |id: usize| ids.iter().position(|&b| b == id);
    let mut parent: Vec<usize> = (0..ids.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for br in net.branches().iter().filter(|b| b.in_service) {
        if let (Some(a), Some(b)) = (at(br.from.0), at(br.to.0)) {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            parent[ra] = rb;
        }
    }
    (0..ids.len())
        .filter(|&i| root(&mut parent, i) == i)
        .count()
}

fn finite(values: &[Option<f64>]) -> Result<(), String> {
    if values.iter().flatten().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err("values must be finite".into())
    }
}

fn row<T>(rows: &[T], index: usize, what: &str) -> Result<usize, String> {
    if index < rows.len() {
        Ok(index)
    } else {
        Err(format!("the case has no {what} {index}"))
    }
}

/// Endpoints of a new line that were isolated become PQ (PV with a
/// generator) and start from the voltage of the bus they join, and so do
/// the set points of their generators: a sound first point for the load flow.
fn connect(work: &mut BalancedNetwork, ends: [usize; 2]) -> Result<(), String> {
    for (bus, other) in [(ends[0], ends[1]), (ends[1], ends[0])] {
        let k = bus_index(work, bus)?;
        if work.buses()[k].kind != BusType::Isolated {
            continue;
        }
        let o = &work.buses()[bus_index(work, other)?];
        let (vm, va) = if o.kind == BusType::Isolated {
            (1.0, 0.0)
        } else {
            (o.vm, o.va)
        };
        let b = &mut work.buses_mut()[k];
        b.kind = BusType::Pq;
        b.vm = vm;
        b.va = va;
        for g in work.generators_mut().iter_mut().filter(|g| g.bus.0 == bus) {
            g.vg = vm;
        }
        retype(work, bus)?;
    }
    Ok(())
}

/// A point for a new bus, when the case is drawn from its own coordinates
/// (some bus carries a location); stamped as placed by hand.
fn located(work: &BalancedNetwork, at: Option<[f64; 2]>) -> Option<Location> {
    let [x, y] = at?;
    if !(x.is_finite() && y.is_finite()) || work.buses().iter().all(|b| b.location.is_none()) {
        return None;
    }
    Some(Location {
        x,
        y,
        kind: Some(powerio::CoordsKind::Manual),
    })
}

/// Loads, shunts, lines, transformers and buses: the edited row (a bus by
/// its id), or `None` for a removal.
fn apply_network_edit(
    work: &mut BalancedNetwork,
    edit: &CaseEdit,
) -> Result<Option<usize>, String> {
    match edit {
        CaseEdit::AddLoad { bus, p, q } => {
            bus_index(work, *bus)?;
            finite(&[Some(*p), *q])?;
            work.loads_mut()
                .push(Load::new(BusId(*bus), *p, q.unwrap_or(0.3 * p)));
            Ok(Some(work.loads().len() - 1))
        }
        CaseEdit::SetLoad {
            index,
            p,
            q,
            in_service,
        } => {
            let i = row(work.loads(), *index, "load")?;
            finite(&[*p, *q])?;
            let l = &mut work.loads_mut()[i];
            if let Some(p) = p {
                l.p = *p;
            }
            if let Some(q) = q {
                l.q = *q;
            }
            if let Some(s) = in_service {
                l.in_service = *s;
            }
            Ok(Some(i))
        }
        CaseEdit::MoveLoad { index, to } => {
            let i = row(work.loads(), *index, "load")?;
            bus_index(work, *to)?;
            work.loads_mut()[i].bus = BusId(*to);
            Ok(Some(i))
        }
        CaseEdit::RemoveLoad { index } => {
            let i = row(work.loads(), *index, "load")?;
            work.loads_mut().remove(i);
            Ok(None)
        }
        CaseEdit::AddShunt { bus, g, b } => {
            bus_index(work, *bus)?;
            finite(&[Some(*g), Some(*b)])?;
            work.shunts_mut().push(Shunt::new(BusId(*bus), *g, *b));
            Ok(Some(work.shunts().len() - 1))
        }
        CaseEdit::SetShunt {
            index,
            g,
            b,
            in_service,
        } => {
            let i = row(work.shunts(), *index, "shunt")?;
            finite(&[*g, *b])?;
            let sh = &mut work.shunts_mut()[i];
            if sh.control.is_some() && (g.is_some() || b.is_some()) {
                return Err(format!(
                    "shunt {index} is a switched shunt; its steps are set in the case file"
                ));
            }
            if let Some(g) = g {
                sh.g = *g;
            }
            if let Some(b) = b {
                sh.b = *b;
            }
            if let Some(s) = in_service {
                sh.in_service = *s;
            }
            Ok(Some(i))
        }
        CaseEdit::MoveShunt { index, to } => {
            let i = row(work.shunts(), *index, "shunt")?;
            bus_index(work, *to)?;
            let sh = &mut work.shunts_mut()[i];
            sh.bus = BusId(*to);
            sh.control = None;
            Ok(Some(i))
        }
        CaseEdit::RemoveShunt { index } => {
            let i = row(work.shunts(), *index, "shunt")?;
            work.shunts_mut().remove(i);
            Ok(None)
        }
        CaseEdit::AddLine {
            from,
            to,
            r,
            x,
            b,
            rate,
        } => {
            bus_index(work, *from)?;
            bus_index(work, *to)?;
            if from == to {
                return Err("a line needs two different buses".into());
            }
            finite(&[*r, *x, *b, *rate])?;
            let x = x.unwrap_or(0.1);
            if x.abs() < 1e-6 {
                return Err("a line needs a series reactance".into());
            }
            let mut br = Branch::new(BusId(*from), BusId(*to), r.unwrap_or(0.01), x);
            br.b = b.unwrap_or(0.02);
            br.rate_a = rate.unwrap_or(100.0);
            work.branches_mut().push(br);
            connect(work, [*from, *to])?;
            Ok(Some(work.branches().len()))
        }
        CaseEdit::AddTransformer {
            from,
            to,
            r,
            x,
            tap,
            shift,
            rate,
        } => {
            bus_index(work, *from)?;
            bus_index(work, *to)?;
            if from == to {
                return Err("a transformer needs two different buses".into());
            }
            finite(&[*r, *x, *tap, *shift, *rate])?;
            let x = x.unwrap_or(0.1);
            if x.abs() < 1e-6 {
                return Err("a transformer needs a leakage reactance".into());
            }
            let tap = tap.unwrap_or(1.0);
            if tap <= 0.0 {
                return Err("the tap ratio must be positive".into());
            }
            let mut br = Branch::new(BusId(*from), BusId(*to), r.unwrap_or(0.002), x);
            br.tap = tap;
            br.shift = shift.unwrap_or(0.0);
            br.rate_a = rate.unwrap_or(100.0);
            work.branches_mut().push(br);
            connect(work, [*from, *to])?;
            Ok(Some(work.branches().len()))
        }
        CaseEdit::SetTransformer { branch, tap, shift } => {
            let i = row(work.branches(), branch.wrapping_sub(1), "transformer")?;
            finite(&[*tap, *shift])?;
            if tap.is_some_and(|t| t <= 0.0) {
                return Err("the tap ratio must be positive".into());
            }
            let br = &mut work.branches_mut()[i];
            if !br.is_transformer() {
                return Err(format!("row {branch} is a line, not a transformer"));
            }
            if let Some(t) = tap {
                br.tap = *t;
            }
            if let Some(s) = shift {
                br.shift = *s;
            }
            Ok(Some(i + 1))
        }
        CaseEdit::AddBus {
            id,
            base_kv,
            near,
            name,
            at,
        } => {
            let next = work.buses().iter().map(|b| b.id.0).max().unwrap_or(0) + 1;
            let id = id.unwrap_or(next);
            if id == 0 || work.buses().iter().any(|b| b.id.0 == id) {
                return Err(format!("bus {id} already exists"));
            }
            let kv = match (base_kv, near) {
                (Some(kv), _) => *kv,
                (None, Some(n)) => work.buses()[bus_index(work, *n)?].base_kv,
                (None, None) => work.buses().first().map_or(1.0, |b| b.base_kv),
            };
            finite(&[Some(kv)])?;
            let mut b = Bus::new(BusId(id), BusType::Isolated, kv);
            b.name = name.clone();
            b.location = located(work, *at);
            work.buses_mut().push(b);
            Ok(Some(id))
        }
        CaseEdit::SetBus {
            bus,
            name,
            base_kv,
            at,
        } => {
            let k = bus_index(work, *bus)?;
            finite(&[*base_kv])?;
            let loc = located(work, *at);
            let b = &mut work.buses_mut()[k];
            if let Some(n) = name {
                b.name = Some(n.clone()).filter(|n| !n.trim().is_empty());
            }
            if let Some(kv) = base_kv {
                b.base_kv = *kv;
            }
            if loc.is_some() {
                b.location = loc;
            }
            Ok(Some(*bus))
        }
        CaseEdit::RemoveBus { bus } => {
            let k = bus_index(work, *bus)?;
            let used = work.generators().iter().any(|g| g.bus.0 == *bus)
                || work.loads().iter().any(|l| l.bus.0 == *bus)
                || work.shunts().iter().any(|s| s.bus.0 == *bus)
                || work
                    .branches()
                    .iter()
                    .any(|b| b.from.0 == *bus || b.to.0 == *bus);
            if used {
                return Err(format!(
                    "bus {bus} still has equipment or lines; remove or move them first"
                ));
            }
            if work.buses()[k].kind == BusType::Ref {
                return Err(format!("bus {bus} is the slack bus"));
            }
            work.buses_mut().remove(k);
            Ok(None)
        }
        CaseEdit::SetLine {
            branch,
            r,
            x,
            b,
            rate,
            in_service,
        } => {
            let i = row(work.branches(), branch.wrapping_sub(1), "line")?;
            finite(&[*r, *x, *b, *rate])?;
            if x.is_some_and(|x| x.abs() < 1e-6) {
                return Err("a line needs a series reactance".into());
            }
            let br = &mut work.branches_mut()[i];
            if let Some(r) = r {
                br.r = *r;
            }
            if let Some(x) = x {
                br.x = *x;
            }
            if let Some(b) = b {
                br.b = *b;
                br.charging = None;
            }
            if let Some(rate) = rate {
                br.rate_a = *rate;
            }
            if let Some(s) = in_service {
                br.in_service = *s;
            }
            Ok(Some(i + 1))
        }
        CaseEdit::RemoveLine { branch } => {
            let i = row(work.branches(), branch.wrapping_sub(1), "line")?;
            work.branches_mut().remove(i);
            Ok(None)
        }
        _ => unreachable!("generator edits are applied by apply_case_edit"),
    }
}

fn keys(net: &BalancedNetwork) -> Vec<UnitKey> {
    net.generators()
        .iter()
        .zip(powerio::psse_machine_ids(net))
        .map(|(g, id)| (g.bus.0, id))
        .collect()
}

fn find(net: &BalancedNetwork, bus: usize, id: &str) -> Result<usize, String> {
    keys(net)
        .iter()
        .position(|(b, i)| *b == bus && i.trim() == id.trim())
        .ok_or_else(|| format!("no generator '{id}' at bus {bus}"))
}

fn bus_index(net: &BalancedNetwork, bus: usize) -> Result<usize, String> {
    net.buses()
        .iter()
        .position(|b| b.id.0 == bus)
        .ok_or_else(|| format!("bus {bus} is not in the case"))
}

/// Bus types after the generators changed at `bus`: PQ gains voltage
/// control with its first in-service generator, PV loses it with the last.
fn retype(net: &mut BalancedNetwork, bus: usize) -> Result<(), String> {
    let k = bus_index(net, bus)?;
    let has = net
        .generators()
        .iter()
        .any(|g| g.bus.0 == bus && g.in_service);
    let b = &mut net.buses_mut()[k];
    match (b.kind, has) {
        (BusType::Pq, true) => b.kind = BusType::Pv,
        (BusType::Pv, false) => b.kind = BusType::Pq,
        (BusType::Ref, false) => {
            return Err(format!(
                "bus {bus} is the slack bus and keeps an in-service generator"
            ))
        }
        _ => {}
    }
    Ok(())
}

/// The first generator cost of the case, for a new generator: an OPF needs
/// one, and the case's own is the least surprising.
fn typical_cost(net: &BalancedNetwork) -> Option<powerio::GenCost> {
    net.generators().iter().find_map(|g| g.cost.clone())
}

fn rating(pmax: f64) -> f64 {
    ((pmax / 0.9) / 10.0).ceil() * 10.0
}

/// Apply `edit` to `net`.
///
/// # Errors
/// An unknown bus or unit, or an edit that would leave the slack bus
/// without a generator; `net` is then unchanged.
pub fn apply_case_edit(net: &mut BalancedNetwork, edit: &CaseEdit) -> Result<EditOutcome, String> {
    if !matches!(
        edit,
        CaseEdit::AddGenerator { .. }
            | CaseEdit::MoveGenerator { .. }
            | CaseEdit::RemoveGenerator { .. }
            | CaseEdit::SetGenerator { .. }
    ) {
        let mut work = net.clone();
        let element = apply_network_edit(&mut work, edit)?;
        if matches!(
            edit,
            CaseEdit::RemoveLine { .. }
                | CaseEdit::SetLine {
                    in_service: Some(false),
                    ..
                }
        ) && islands(&work) > islands(net)
        {
            return Err("taking this line out splits the network into separate islands".into());
        }
        *net = work;
        return Ok(EditOutcome {
            element,
            ..EditOutcome::default()
        });
    }
    let before = keys(net);
    let mut work = net.clone();
    let (index, touched) = match edit {
        CaseEdit::AddGenerator {
            bus,
            pg,
            pmax,
            qmax,
            qmin,
            vg,
            mbase,
        } => {
            let k = bus_index(&work, *bus)?;
            let pmax = pmax.unwrap_or_else(|| (2.0 * pg).max(100.0));
            let mut g = Generator::new(BusId(*bus));
            g.pg = *pg;
            g.pmax = pmax;
            g.pmin = 0.0;
            g.qmax = qmax.unwrap_or(0.5 * pmax);
            g.qmin = qmin.unwrap_or(-0.5 * pmax);
            g.vg = vg.unwrap_or(work.buses()[k].vm);
            g.mbase = mbase.unwrap_or_else(|| rating(pmax));
            g.cost = typical_cost(&work);
            if work.buses()[k].kind == BusType::Pq {
                work.buses_mut()[k].vm = g.vg;
            }
            work.generators_mut().push(g);
            (work.generators().len() - 1, vec![*bus])
        }
        CaseEdit::MoveGenerator { bus, id, to } => {
            let i = find(&work, *bus, id)?;
            let k = bus_index(&work, *to)?;
            let vm = work.buses()[k].vm;
            let g = &mut work.generators_mut()[i];
            g.bus = BusId(*to);
            g.regulated_bus = None;
            g.regulating_terminal = None;
            if work.buses()[k].kind == BusType::Pq {
                work.generators_mut()[i].vg = vm;
            }
            (i, vec![*bus, *to])
        }
        CaseEdit::RemoveGenerator { bus, id } => {
            let i = find(&work, *bus, id)?;
            work.generators_mut().remove(i);
            (usize::MAX, vec![*bus])
        }
        CaseEdit::SetGenerator {
            bus,
            id,
            pg,
            pmax,
            qmax,
            qmin,
            vg,
            mbase,
            in_service,
        } => {
            let i = find(&work, *bus, id)?;
            let g = &mut work.generators_mut()[i];
            for (field, value) in [
                (&mut g.pg, pg),
                (&mut g.pmax, pmax),
                (&mut g.qmax, qmax),
                (&mut g.qmin, qmin),
                (&mut g.vg, vg),
                (&mut g.mbase, mbase),
            ] {
                if let Some(v) = value {
                    if !v.is_finite() {
                        return Err("generator values must be finite".into());
                    }
                    *field = *v;
                }
            }
            if let Some(s) = in_service {
                g.in_service = *s;
            }
            (i, vec![*bus])
        }
        _ => unreachable!("network edits returned above"),
    };
    for bus in touched {
        retype(&mut work, bus)?;
    }
    let after = keys(&work);
    let mut out = EditOutcome {
        unit: after.get(index).cloned(),
        ..EditOutcome::default()
    };
    // Generators keep their order except the one removed (or appended), so
    // the old and new keys line up by position.
    let removed_at = match edit {
        CaseEdit::RemoveGenerator { bus, id } => {
            let i = before
                .iter()
                .position(|(b, x)| b == bus && x.trim() == id.trim());
            out.removed.extend(i.map(|i| before[i].clone()));
            i
        }
        _ => None,
    };
    let old = before
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != removed_at)
        .map(|(_, k)| k);
    for (from, to) in old.zip(&after) {
        if from != to {
            out.renames.push((from.clone(), to.clone()));
        }
    }
    *net = work;
    Ok(out)
}

/// [`apply_case_edit`] for a list of edits, the outcomes in order.
///
/// # Errors
/// As [`apply_case_edit`]; `net` is unchanged when any edit fails.
pub fn apply_case_edits(
    net: &mut BalancedNetwork,
    edits: &[CaseEdit],
) -> Result<Vec<EditOutcome>, String> {
    let mut work = net.clone();
    let out = edits
        .iter()
        .map(|e| apply_case_edit(&mut work, e))
        .collect::<Result<Vec<_>, _>>()?;
    *net = work;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The WECC 240-bus case (`tests/data/rms_examples/wecc240`).
    fn wecc240() -> BalancedNetwork {
        let raw = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/rms_examples/wecc240/case.raw");
        let module = powerio::parse(powerio::Source::open(raw).unwrap()).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        net.clone()
    }

    fn kind(net: &BalancedNetwork, bus: usize) -> BusType {
        net.buses()[bus_index(net, bus).unwrap()].kind
    }

    #[test]
    fn add_move_remove_retype_and_rekey() {
        let mut net = wecc240();
        let n = net.generators().len();
        assert_eq!(kind(&net, 1002), BusType::Pq);
        let out = apply_case_edit(
            &mut net,
            &CaseEdit::AddGenerator {
                bus: 1002,
                pg: 30.0,
                pmax: None,
                qmax: None,
                qmin: None,
                vg: None,
                mbase: None,
            },
        )
        .unwrap();
        assert_eq!(out.unit, Some((1002, "1".into())));
        assert!(out.renames.is_empty());
        assert_eq!(net.generators().len(), n + 1);
        assert_eq!(kind(&net, 1002), BusType::Pv);
        net.validate().unwrap();

        // A second unit at 1002, then the first one moves to 1004: bus 1002
        // keeps one unit, and its id shifts only where it has no retained id.
        apply_case_edit(
            &mut net,
            &CaseEdit::AddGenerator {
                bus: 1002,
                pg: 10.0,
                pmax: None,
                qmax: None,
                qmin: None,
                vg: None,
                mbase: None,
            },
        )
        .unwrap();
        let out = apply_case_edit(
            &mut net,
            &CaseEdit::MoveGenerator {
                bus: 1002,
                id: "1".into(),
                to: 1004,
            },
        )
        .unwrap();
        assert_eq!(out.unit, Some((1004, "1".into())));
        assert!(out
            .renames
            .contains(&((1002, "1".into()), (1004, "1".into()))));
        assert!(out
            .renames
            .contains(&((1002, "2".into()), (1002, "1".into()))));
        assert_eq!(kind(&net, 1004), BusType::Pv);
        assert_eq!(kind(&net, 1002), BusType::Pv);

        let out = apply_case_edit(
            &mut net,
            &CaseEdit::RemoveGenerator {
                bus: 1002,
                id: "1".into(),
            },
        )
        .unwrap();
        assert_eq!(out.removed, vec![(1002, "1".into())]);
        assert_eq!(kind(&net, 1002), BusType::Pq);
        net.validate().unwrap();

        // The slack keeps its last generator; a failed edit leaves the case
        // as it was.
        let slack = net
            .buses()
            .iter()
            .find(|b| b.kind == BusType::Ref)
            .unwrap()
            .id
            .0;
        let at_slack = |net: &BalancedNetwork| -> Vec<String> {
            let ids = powerio::psse_machine_ids(net);
            net.generators()
                .iter()
                .zip(ids)
                .filter(|(g, _)| g.bus.0 == slack)
                .map(|(_, id)| id)
                .collect()
        };
        while at_slack(&net).len() > 1 {
            let id = at_slack(&net)[0].clone();
            apply_case_edit(&mut net, &CaseEdit::RemoveGenerator { bus: slack, id }).unwrap();
        }
        let last = at_slack(&net)[0].clone();
        let before = net.clone();
        let err = apply_case_edit(
            &mut net,
            &CaseEdit::RemoveGenerator {
                bus: slack,
                id: last,
            },
        );
        assert!(err.is_err());
        assert_eq!(keys(&net), keys(&before));
    }

    #[test]
    fn loads_shunts_and_lines() {
        let mut net = wecc240();
        let n_load = net.loads().len();
        let edits = [
            CaseEdit::AddLoad {
                bus: 1004,
                p: 30.0,
                q: None,
            },
            CaseEdit::AddShunt {
                bus: 1004,
                g: 0.0,
                b: 15.0,
            },
            CaseEdit::AddLine {
                from: 1004,
                to: 1003,
                r: None,
                x: None,
                b: None,
                rate: None,
            },
        ];
        let out = apply_case_edits(&mut net, &edits).unwrap();
        assert_eq!(out[0].element, Some(n_load));
        assert_eq!(net.loads()[n_load].q, 9.0);
        let line = out[2].element.unwrap();
        assert_eq!(net.branches().len(), line);
        net.validate().unwrap();
        let view = case_equipment(&net);
        assert_eq!(view.lines.last().unwrap().from, 1004);
        assert_eq!(view.shunts.last().unwrap().b, 15.0);

        apply_case_edit(
            &mut net,
            &CaseEdit::MoveLoad {
                index: n_load,
                to: 1101,
            },
        )
        .unwrap();
        assert_eq!(net.loads()[n_load].bus.0, 1101);
        apply_case_edit(&mut net, &CaseEdit::RemoveLine { branch: line }).unwrap();
        // Bus 2407 hangs on one line: taking it out islands it, refused.
        let radial = case_equipment(&net)
            .lines
            .into_iter()
            .find(|l| l.from == 2407 || l.to == 2407)
            .unwrap()
            .id;
        assert!(apply_case_edit(&mut net, &CaseEdit::RemoveLine { branch: radial }).is_err());
        assert!(apply_case_edit(
            &mut net,
            &CaseEdit::SetLine {
                branch: radial,
                r: None,
                x: None,
                b: None,
                rate: None,
                in_service: Some(false)
            }
        )
        .is_err());
        assert!(apply_case_edit(
            &mut net,
            &CaseEdit::AddLine {
                from: 1003,
                to: 1003,
                r: None,
                x: None,
                b: None,
                rate: None
            }
        )
        .is_err());
    }

    /// A new bus joined by a transformer, with a generator and a load,
    /// runs a fault in RMS; it comes out again once empty.
    #[test]
    fn new_bus_and_transformer_simulate() {
        use crate::rms::{
            dyr_dynamize, DynamizeRequest, LoadModel, RmsEvent, RmsRequest, RmsSimulation,
        };
        let mut net = wecc240();
        let new_bus = net.buses().iter().map(|b| b.id.0).max().unwrap() + 1;
        let out = apply_case_edits(
            &mut net,
            &[
                CaseEdit::AddBus {
                    id: None,
                    base_kv: Some(13.8),
                    near: None,
                    name: Some("NEW GEN".into()),
                    at: None,
                },
                CaseEdit::AddTransformer {
                    from: 1101,
                    to: new_bus,
                    r: None,
                    x: Some(0.08),
                    tap: Some(1.02),
                    shift: None,
                    rate: None,
                },
                CaseEdit::AddGenerator {
                    bus: new_bus,
                    pg: 40.0,
                    pmax: None,
                    qmax: None,
                    qmin: None,
                    vg: Some(1.02),
                    mbase: None,
                },
                CaseEdit::AddLoad {
                    bus: new_bus,
                    p: 5.0,
                    q: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(out[0].element, Some(new_bus));
        assert_eq!(kind(&net, new_bus), BusType::Pv);
        net.validate().unwrap();
        let view = case_equipment(&net);
        let t = view.lines.last().unwrap();
        assert!(t.transformer);
        assert_eq!(t.tap.as_ref().unwrap().tap, 1.02);
        // An empty new bus stays isolated and does not stop the case.
        apply_case_edit(
            &mut net,
            &CaseEdit::AddBus {
                id: Some(99),
                base_kv: None,
                near: Some(1101),
                name: None,
                at: None,
            },
        )
        .unwrap();
        assert_eq!(kind(&net, 99), BusType::Isolated);
        let req = DynamizeRequest {
            machine: "GENROU".into(),
            exciter: Some("SEXS".into()),
            governor: Some("TGOV1".into()),
            ..DynamizeRequest::default()
        };
        let dyr = powerio::parse_dyr(&dyr_dynamize(&net, "", &req).unwrap()).unwrap();
        let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
        let res = sim
            .run(&RmsRequest {
                t_end: 1.5,
                dt: 0.005,
                output_step: 0.01,
                load_model: LoadModel::ConstantImpedance,
                events: vec![
                    RmsEvent::BusFault {
                        time: 0.5,
                        bus: 1101,
                        r: 0.0,
                        x: 0.05,
                    },
                    RmsEvent::ClearFault {
                        time: 0.6,
                        bus: 1101,
                    },
                ],
                small_signal: false,
                record: None,
            })
            .unwrap();
        assert!(res.channels[&format!("omega:{new_bus}")]
            .iter()
            .all(|w| (w - 1.0).abs() < 0.05));
        assert!(apply_case_edit(&mut net, &CaseEdit::RemoveBus { bus: new_bus }).is_err());
        apply_case_edit(&mut net, &CaseEdit::RemoveBus { bus: 99 }).unwrap();
        assert!(net.buses().iter().all(|b| b.id.0 != 99));
    }
}
