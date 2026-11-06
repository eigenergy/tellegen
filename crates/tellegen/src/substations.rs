//! Substation grouping for balanced networks.
//!
//! A transmission diagram draws substations, not buses: an overview shows substations joined by
//! lines, and a substation holds one busbar per voltage level with its transformers between them.
//! This module assigns every bus of the bus-branch analysis view to one substation.
//!
//! The grouping is a drawing aid, not an electrical claim. Sources, strongest first:
//!
//! 1. **Declared**: PowerIO's voltage levels (CGMES, XIIDM, RAWX, PSS/E), or a PowerWorld
//!    `SubNum` kept in the bus extras.
//! 2. **Co-located**: buses at exactly the same coordinate before they were spread for display
//!    (ACTIVSg buses carry their substation's coordinate). The caller passes these groups; see
//!    [`crate::geo::spread_stacks`].
//! 3. **Transformer**: the two ends of a transformer, unless both are located and far apart.
//! 4. **Jumper**: the two ends of a same-voltage branch whose reactance is a small fraction of the
//!    case's typical line (a bus-splitting jumper inside a substation), likewise unless far apart,
//!    and only while the group stays small.
//!
//! Two buses declared in different substations are never merged by the weaker rules.
//! Buses that none of these join form a substation of their own.

use std::collections::{BTreeMap, HashMap};

use powerio::BalancedNetwork;
use serde::Serialize;

use crate::geo::Coords;

/// How a substation's members were found. The strongest rule that joined any two of its buses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupSource {
    /// A single bus that no rule joined to another.
    Single,
    /// Joined only through bus-splitting jumpers.
    Jumper,
    /// Joined through transformers (and possibly jumpers).
    Transformer,
    /// Joined by sharing a coordinate (possibly with transformers).
    Colocated,
    /// Declared by the source data.
    Declared,
}

/// One substation of a [`SubstationGrouping`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Substation {
    /// Stable key: `sub:<declared id>` when declared, else `bus:<lowest bus id>`.
    pub key: String,
    /// The declared name, when the source gives one.
    pub name: Option<String>,
    pub source: GroupSource,
    /// Member bus rows in the analysis view, ascending. Not serialized: payloads carry the
    /// per-bus index ([`SubstationGrouping::bus_substation`]) instead.
    #[serde(skip_serializing)]
    pub buses: Vec<u32>,
    /// Distinct positive nominal voltages of the members in kV, highest first.
    pub kv: Vec<f64>,
}

/// Every bus of the analysis view assigned to one substation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SubstationGrouping {
    /// Substation index per bus row of the analysis view.
    pub bus_substation: Vec<u32>,
    /// Substations ordered by their lowest member row.
    pub substations: Vec<Substation>,
    /// Per branch row of the analysis view: whether it is a transformer ([`is_transformer`] at
    /// the grouping's tolerance). Computed once here so callers need not repeat the test.
    #[serde(skip_serializing)]
    pub transformer: Vec<bool>,
}

#[derive(Clone, Debug)]
pub struct GroupingOptions {
    /// Transformer ends both located further apart than this (km) are not joined. Real
    /// transformers sit inside one substation; a long "transformer" is usually a modelling
    /// artefact (a phase shifter on a line, an equivalent).
    pub max_transformer_span_km: f64,
    /// Endpoint nominal voltages differing by more than this fraction mark a branch as a
    /// transformer even when its tap and shift fields are zero.
    pub kv_ratio_tolerance: f64,
    /// A same-voltage branch with reactance below this fraction of the median reactance of the
    /// case's same-voltage branches is a jumper. ACTIVSg200's 23 jumpers sit at 0.07 of its
    /// median line; its shortest line between substations at 0.15. Zero disables the rule.
    pub jumper_fraction: f64,
    /// Jumpers do not grow a group past this many buses, so a chain of short lines cannot merge
    /// a region into one substation.
    pub max_jumper_group: usize,
}

impl Default for GroupingOptions {
    fn default() -> Self {
        Self {
            max_transformer_span_km: 2.0,
            kv_ratio_tolerance: 0.01,
            jumper_fraction: 0.1,
            max_jumper_group: 32,
        }
    }
}

/// Whether a branch row joins two voltage levels: PowerIO's transformer test, or endpoints whose
/// nominal voltages differ (some synthetic cases write tap 0 on a transformer row).
fn is_transformer(
    net: &BalancedNetwork,
    branch: usize,
    rows: &HashMap<usize, usize>,
    tolerance: f64,
) -> bool {
    let br = &net.branches()[branch];
    if br.is_transformer() {
        return true;
    }
    let kv = |id: usize| rows.get(&id).map(|&r| net.buses()[r].base_kv);
    match (kv(br.from.0), kv(br.to.0)) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => (a - b).abs() > tolerance * a.max(b),
        _ => false,
    }
}

/// Bus id to row in `net`.
fn bus_rows(net: &BalancedNetwork) -> HashMap<usize, usize> {
    net.buses()
        .iter()
        .enumerate()
        .map(|(row, bus)| (bus.id.0, row))
        .collect()
}

/// Group the buses of `net` (the analysis view, after three-winding lowering) into substations.
///
/// `colocated` lists bus ids that shared a coordinate before display spreading. `located` gives
/// geographic `(lon, lat)` per bus id for the transformer span check; pass `None` for drawing
/// coordinates or none at all.
pub fn group_substations(
    net: &BalancedNetwork,
    colocated: &[Vec<usize>],
    located: Option<&Coords>,
    options: &GroupingOptions,
) -> SubstationGrouping {
    let n = net.buses().len();
    let rows = bus_rows(net);
    let declared = declared_keys(net, &rows);
    let mut sets = Groups::new(&declared);
    let transformer: Vec<bool> = (0..net.branches().len())
        .map(|k| is_transformer(net, k, &rows, options.kv_ratio_tolerance))
        .collect();

    for group in colocated {
        let members: Vec<usize> = group
            .iter()
            .filter_map(|id| rows.get(id).copied())
            .collect();
        for pair in members.windows(2) {
            sets.union(pair[0], pair[1], GroupSource::Colocated);
        }
    }
    // Both ends located and further apart than a substation can be.
    let far = |a: usize, b: usize| {
        located
            .and_then(|coords| Some((coords.get(&a)?, coords.get(&b)?)))
            .is_some_and(|(p, q)| haversine_km(*p, *q) > options.max_transformer_span_km)
    };
    for (k, br) in net.branches().iter().enumerate() {
        let (Some(&a), Some(&b)) = (rows.get(&br.from.0), rows.get(&br.to.0)) else {
            continue;
        };
        // An out-of-service transformer couples nothing, so it joins nothing.
        if a == b || !br.in_service || !transformer[k] {
            continue;
        }
        if !far(br.from.0, br.to.0) {
            sets.union(a, b, GroupSource::Transformer);
        }
    }
    // Jumpers: same-voltage, in-service branches far shorter (in reactance) than the case's
    // typical line.
    if options.jumper_fraction > 0.0 {
        let same_kv: Vec<usize> = (0..net.branches().len())
            .filter(|&k| {
                let br = &net.branches()[k];
                br.in_service
                    && br.from != br.to
                    && rows.contains_key(&br.from.0)
                    && rows.contains_key(&br.to.0)
                    && !transformer[k]
            })
            .collect();
        let mut x: Vec<f64> = same_kv
            .iter()
            .map(|&k| net.branches()[k].x.abs())
            .filter(|v| v.is_finite())
            .collect();
        x.sort_by(f64::total_cmp);
        if let Some(&median) = x.get(x.len() / 2) {
            let threshold = options.jumper_fraction * median;
            for &k in &same_kv {
                let br = &net.branches()[k];
                if br.x.abs() < threshold && !far(br.from.0, br.to.0) {
                    let (a, b) = (rows[&br.from.0], rows[&br.to.0]);
                    sets.union_capped(a, b, GroupSource::Jumper, options.max_jumper_group);
                }
            }
        }
    }

    // The lowest bus id per component, for the `bus:<id>` key: rows need not be in id order.
    let mut lowest_id: HashMap<usize, usize> = HashMap::new();
    for row in 0..n {
        let id = net.buses()[row].id.0;
        let root = sets.find(row);
        lowest_id
            .entry(root)
            .and_modify(|v| *v = (*v).min(id))
            .or_insert(id);
    }
    // Number components by their lowest row so the output follows bus order.
    let mut index_of_root: HashMap<usize, u32> = HashMap::new();
    let mut bus_substation = Vec::with_capacity(n);
    let mut substations: Vec<Substation> = Vec::new();
    for row in 0..n {
        let root = sets.find(row);
        let next = substations.len() as u32;
        let s = *index_of_root.entry(root).or_insert_with(|| {
            let (key, name, source) = match &sets.declared[root] {
                Some(d) => (
                    format!("sub:{}", d.key),
                    d.name.clone(),
                    GroupSource::Declared,
                ),
                None => (format!("bus:{}", lowest_id[&root]), None, sets.source[root]),
            };
            substations.push(Substation {
                key,
                name,
                source,
                buses: Vec::new(),
                kv: Vec::new(),
            });
            next
        });
        substations[s as usize].buses.push(row as u32);
        bus_substation.push(s);
    }
    for sub in &mut substations {
        let mut kv: Vec<f64> = sub
            .buses
            .iter()
            .map(|&r| net.buses()[r as usize].base_kv)
            .filter(|kv| *kv > 0.0)
            .collect();
        kv.sort_by(|a, b| b.total_cmp(a));
        kv.dedup_by(|a, b| (*a - *b).abs() <= 1e-9 * a.abs().max(1.0));
        sub.kv = kv;
    }
    SubstationGrouping {
        bus_substation,
        substations,
        transformer,
    }
}

/// [`group_substations`] for drawing, on the coordinates a case is shown with. Synthetic
/// (placeholder) coordinates say nothing about substations, so they give neither co-located
/// groups nor transformer spans. Server and browser payloads both group through here.
pub fn group_for_display(
    net: &BalancedNetwork,
    colocated: &[Vec<usize>],
    located: Option<&Coords>,
    synthetic: bool,
) -> SubstationGrouping {
    if synthetic {
        group_substations(net, &[], None, &GroupingOptions::default())
    } else {
        group_substations(net, colocated, located, &GroupingOptions::default())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Declared {
    key: String,
    name: Option<String>,
}

/// The declared substation per bus row: PowerIO voltage levels first, then `SubNum` extras.
fn declared_keys(net: &BalancedNetwork, rows: &HashMap<usize, usize>) -> Vec<Option<Declared>> {
    let mut out: Vec<Option<Declared>> = vec![None; net.buses().len()];
    if let Some(detailed) = net.detailed_connectivity() {
        for level in &detailed.voltage_levels {
            let Some(substation) = &level.substation else {
                continue;
            };
            for bus in &level.buses {
                if let Some(&row) = rows.get(&bus.0) {
                    out[row].get_or_insert_with(|| Declared {
                        key: substation.to_string(),
                        name: Some(substation.local_id().to_owned()),
                    });
                }
            }
        }
    }
    for (row, bus) in net.buses().iter().enumerate() {
        if out[row].is_some() {
            continue;
        }
        let value = bus
            .extras
            .get("SubNum")
            .or_else(|| bus.extras.get("SubNumber"));
        let key = match value {
            Some(serde_json::Value::Number(number)) => Some(number.to_string()),
            Some(serde_json::Value::String(text)) => {
                let text = text.trim();
                (!text.is_empty()).then(|| text.to_owned())
            }
            _ => None,
        };
        // PowerWorld writes integer substation numbers; "12" and "12.0" are the same one.
        out[row] = key.map(|key| Declared {
            key: key
                .parse::<f64>()
                .ok()
                .filter(|v| v.fract() == 0.0)
                .map_or(key, |v| format!("{v:.0}")),
            name: None,
        });
    }
    out
}

/// Union-find that refuses to merge two components declared in different substations and keeps
/// the strongest rule that formed each component.
struct Groups {
    parent: Vec<usize>,
    rank: Vec<u8>,
    size: Vec<usize>,
    declared: Vec<Option<Declared>>,
    source: Vec<GroupSource>,
}

impl Groups {
    fn new(declared: &[Option<Declared>]) -> Self {
        let n = declared.len();
        let mut groups = Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
            size: vec![1; n],
            declared: declared.to_vec(),
            source: vec![GroupSource::Single; n],
        };
        // Buses sharing a declared key are one substation from the start.
        let mut first: BTreeMap<String, usize> = BTreeMap::new();
        for (row, d) in declared.iter().enumerate() {
            if let Some(d) = d {
                match first.get(&d.key) {
                    Some(&r) => groups.union(r, row, GroupSource::Declared),
                    None => {
                        first.insert(d.key.clone(), row);
                    }
                }
            }
        }
        groups
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize, why: GroupSource) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        if let (Some(da), Some(db)) = (&self.declared[ra], &self.declared[rb]) {
            if da.key != db.key {
                return;
            }
        }
        let (root, child) = if self.rank[ra] >= self.rank[rb] {
            (ra, rb)
        } else {
            (rb, ra)
        };
        if self.rank[ra] == self.rank[rb] {
            self.rank[root] += 1;
        }
        self.parent[child] = root;
        self.size[root] += self.size[child];
        if self.declared[root].is_none() {
            self.declared[root] = self.declared[child].take();
        }
        self.source[root] = self.source[root].max(self.source[child]).max(why);
    }

    /// [`Groups::union`], unless the merged group would exceed `cap` buses.
    fn union_capped(&mut self, a: usize, b: usize, why: GroupSource, cap: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb && self.size[ra] + self.size[rb] <= cap {
            self.union(a, b, why);
        }
    }
}

fn haversine_km((lon1, lat1): (f64, f64), (lon2, lat2): (f64, f64)) -> f64 {
    const R: f64 = 6371.0;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buses `(id, kv)` and branches `(from, to, tap)`.
    fn network(buses: &[(usize, f64)], branches: &[(usize, usize, f64)]) -> BalancedNetwork {
        let mut m = String::from("function mpc = t\nmpc.version = '2';\nmpc.baseMVA = 100;\n");
        m += "mpc.bus = [\n";
        for (i, &(id, kv)) in buses.iter().enumerate() {
            let kind = if i == 0 { 3 } else { 1 };
            m += &format!(" {id} {kind} 0 0 0 0 1 1 0 {kv} 1 1.1 0.9;\n");
        }
        m += "];\nmpc.gen = [\n";
        m += &format!(
            " {} 0 0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;\n",
            buses[0].0
        );
        m += "];\nmpc.branch = [\n";
        for &(f, t, tap) in branches {
            m += &format!(" {f} {t} 0.01 0.1 0 100 100 100 {tap} 0 1 -360 360;\n");
        }
        m += "];\nmpc.gencost = [\n 2 0 0 3 0.1 5 0;\n];\n";
        crate::model::parse_matpower(&m).expect("parse test network")
    }

    fn members(g: &SubstationGrouping) -> Vec<Vec<u32>> {
        g.substations.iter().map(|s| s.buses.clone()).collect()
    }

    #[test]
    fn transformers_join_voltage_levels_and_lines_do_not() {
        // 1(345) =T= 2(138) -- 3(138) =T= 4(69)
        let net = network(
            &[(1, 345.0), (2, 138.0), (3, 138.0), (4, 69.0)],
            &[(1, 2, 1.0), (2, 3, 0.0), (3, 4, 1.0)],
        );
        let g = group_substations(&net, &[], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 1], vec![2, 3]]);
        assert_eq!(g.bus_substation, vec![0, 0, 1, 1]);
        assert_eq!(g.substations[0].kv, vec![345.0, 138.0]);
        assert_eq!(g.substations[0].source, GroupSource::Transformer);
        assert_eq!(g.substations[0].key, "bus:1");
    }

    #[test]
    fn a_kv_change_marks_a_transformer_without_a_tap() {
        let net = network(&[(1, 230.0), (2, 115.0)], &[(1, 2, 0.0)]);
        let rows = bus_rows(&net);
        assert!(is_transformer(&net, 0, &rows, 0.01));
        let g = group_substations(&net, &[], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 1]]);
    }

    #[test]
    fn out_of_service_transformers_join_nothing() {
        let mut net = network(&[(1, 345.0), (2, 138.0)], &[(1, 2, 1.0)]);
        net.branches_mut()[0].in_service = false;
        let g = group_substations(&net, &[], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0], vec![1]]);
        // Still drawn as a transformer.
        assert_eq!(g.transformer, vec![true]);
    }

    #[test]
    fn keys_use_the_lowest_bus_id_not_the_lowest_row() {
        let net = network(&[(200, 138.0), (5, 138.0)], &[(200, 5, 0.0)]);
        let g = group_substations(&net, &[vec![200, 5]], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 1]]);
        assert_eq!(g.substations[0].key, "bus:5");
    }

    #[test]
    fn kv_ratio_tolerance_matches_the_svelte_fallback() {
        // `KV_RATIO_TOLERANCE` in packages/svelte/src/lib/diagram/transmission.ts repeats this
        // value for older payloads; change both together.
        assert_eq!(GroupingOptions::default().kv_ratio_tolerance, 0.01);
    }

    #[test]
    fn colocated_buses_form_one_substation() {
        let net = network(
            &[(1, 138.0), (2, 138.0), (3, 138.0)],
            &[(1, 2, 0.0), (2, 3, 0.0)],
        );
        let g = group_substations(&net, &[vec![3, 1]], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 2], vec![1]]);
        assert_eq!(g.substations[0].source, GroupSource::Colocated);
        assert_eq!(g.substations[1].source, GroupSource::Single);
    }

    #[test]
    fn far_apart_transformer_ends_stay_separate() {
        let net = network(&[(1, 345.0), (2, 138.0)], &[(1, 2, 1.0)]);
        // About 11 km apart.
        let coords = Coords::from([(1, (-97.0, 31.0)), (2, (-97.0, 31.1))]);
        let g = group_substations(&net, &[], Some(&coords), &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0], vec![1]]);
        let near = Coords::from([(1, (-97.0, 31.0)), (2, (-97.0, 31.001))]);
        let g = group_substations(&net, &[], Some(&near), &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 1]]);
    }

    #[test]
    fn declared_substations_win_over_weaker_rules() {
        let mut net = network(
            &[(1, 345.0), (2, 138.0), (3, 138.0), (4, 138.0)],
            &[(1, 2, 1.0), (2, 3, 0.0), (3, 4, 0.0)],
        );
        // 1 and 2 declared in different substations although a transformer joins them;
        // 3 declared with 1; 4 undeclared but co-located with 3.
        for (row, sub) in [(0, 10), (1, 20), (2, 10)] {
            net.buses_mut()[row]
                .extras
                .insert("SubNum".into(), serde_json::json!(sub));
        }
        let g = group_substations(&net, &[vec![3, 4]], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 2, 3], vec![1]]);
        assert_eq!(g.substations[0].key, "sub:10");
        assert_eq!(g.substations[0].source, GroupSource::Declared);
        assert_eq!(g.substations[1].key, "sub:20");
    }

    #[test]
    fn string_and_float_substation_numbers_match() {
        let mut net = network(&[(1, 138.0), (2, 138.0)], &[(1, 2, 0.0)]);
        net.buses_mut()[0]
            .extras
            .insert("SubNum".into(), serde_json::json!("7"));
        net.buses_mut()[1]
            .extras
            .insert("SubNum".into(), serde_json::json!(7.0));
        let g = group_substations(&net, &[], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 1]]);
        assert_eq!(g.substations[0].key, "sub:7");
    }

    /// Buses `(id, kv)` and branches `(from, to, tap, x)`.
    fn network_x(buses: &[(usize, f64)], branches: &[(usize, usize, f64, f64)]) -> BalancedNetwork {
        let mut net = network(
            buses,
            &branches.iter().map(|b| (b.0, b.1, b.2)).collect::<Vec<_>>(),
        );
        for (row, b) in branches.iter().enumerate() {
            net.branches_mut()[row].x = b.3;
        }
        net
    }

    #[test]
    fn jumpers_join_buses_split_inside_a_substation() {
        // 1 =jumper= 2 (x 0.003) inside one substation; lines 2-3, 3-4, 4-1 at x 0.05.
        let net = network_x(
            &[(1, 115.0), (2, 115.0), (3, 115.0), (4, 115.0)],
            &[
                (1, 2, 0.0, 0.003),
                (2, 3, 0.0, 0.05),
                (3, 4, 0.0, 0.05),
                (4, 1, 0.0, 0.05),
            ],
        );
        let g = group_substations(&net, &[], None, &GroupingOptions::default());
        assert_eq!(members(&g), vec![vec![0, 1], vec![2], vec![3]]);
        assert_eq!(g.substations[0].source, GroupSource::Jumper);
        let off = GroupingOptions {
            jumper_fraction: 0.0,
            ..GroupingOptions::default()
        };
        assert_eq!(
            group_substations(&net, &[], None, &off).substations.len(),
            4
        );
    }

    #[test]
    fn a_chain_of_jumpers_stops_at_the_cap() {
        // Ten buses joined by very short lines, plus enough long lines to set the median.
        let buses: Vec<_> = (1..=12).map(|i| (i, 115.0)).collect();
        let mut branches: Vec<_> = (1..10).map(|i| (i, i + 1, 0.0, 0.001)).collect();
        branches.extend((1..=12).map(|i| (i, if i == 12 { 1 } else { i + 1 }, 0.0, 0.05)));
        let net = network_x(&buses, &branches);
        let options = GroupingOptions {
            max_jumper_group: 4,
            ..GroupingOptions::default()
        };
        let g = group_substations(&net, &[], None, &options);
        assert!(g.substations.iter().all(|s| s.buses.len() <= 4));
    }

    #[test]
    fn grouping_is_linear_on_a_long_chain() {
        // Voltages 230, 115, 115, 230, ... so branches alternate transformer, line:
        // n/2 substations of two buses.
        let n = 20_000;
        let buses: Vec<_> = (1..=n)
            .map(|i| (i, if matches!(i % 4, 0 | 1) { 230.0 } else { 115.0 }))
            .collect();
        let branches: Vec<_> = (1..n)
            .map(|i| (i, i + 1, if i % 2 == 1 { 1.0 } else { 0.0 }))
            .collect();
        let net = network(&buses, &branches);
        let g = group_substations(&net, &[], None, &GroupingOptions::default());
        assert_eq!(g.substations.len(), n / 2);
        assert!(g.substations.iter().all(|s| s.buses.len() == 2));
    }
}
