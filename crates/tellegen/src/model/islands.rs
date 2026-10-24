//! Island energization: the one place that decides which islands a solve
//! carries and where each is referenced, before PowerIO prepares the problem.
//!
//! An island is a set of non-isolated buses joined by in-service branches and
//! in-service three-winding transformers. An island with no in-service
//! generator cannot supply its own load, so it is de-energized: its buses are
//! typed isolated (and a three-winding transformer touching them is taken out
//! of service, so its star bus does not survive alone), which removes the
//! island and every element on it from the numerical problem. A supplied island
//! that states no reference bus gets one at the bus of its largest generator.
//! Each decision is reported as a [`SolveDiagnostic`].
//!
//! PowerIO's preparation already grounds every reference it is given, so a
//! network that needs neither decision passes through unchanged and uncopied.
//! A per-island reference policy in PowerIO itself can replace
//! [`designate_references`] without touching the solvers.

use std::collections::HashMap;

use powerio::{BalancedNetwork, BusId, BusType};

use crate::api::SolveDiagnostic;

/// Which stated references count when deciding whether a supplied island needs
/// one designated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferencePolicy {
    /// Any bus typed reference. The typed-instance preparations use the
    /// network's bus types as stated.
    Stated,
    /// Only a reference bus that hosts an in-service generator. PowerIO
    /// normalization demotes a generator-less reference to PQ, so the paths
    /// that normalize first need a generator-hosted one.
    GeneratorHosted,
}

/// The outcome of [`energize`]: the adjusted network when anything changed, and
/// what changed.
pub(crate) struct Energized {
    network: Option<BalancedNetwork>,
    pub(crate) diagnostics: Vec<SolveDiagnostic>,
}

impl Energized {
    /// The network to prepare: the adjusted copy, or `original` when nothing
    /// changed.
    pub(crate) fn network<'a>(&'a self, original: &'a BalancedNetwork) -> &'a BalancedNetwork {
        self.network.as_ref().unwrap_or(original)
    }

    /// The adjusted copy, when anything changed.
    pub(crate) fn into_network(self) -> Option<BalancedNetwork> {
        self.network
    }
}

/// Find the islands of `network`, de-energize the unsupplied ones, and
/// designate a reference in each supplied island that has none under `policy`.
pub(crate) fn energize(network: &BalancedNetwork, policy: ReferencePolicy) -> Energized {
    let islands = Islands::of(network);
    let supplied = islands.supplied(network);
    let mut diagnostics = Vec::new();
    let mut isolate = Vec::new();
    if supplied.iter().any(|&supplied| !supplied) {
        let mw = if network.is_normalized() {
            network.base_mva()
        } else {
            1.0
        };
        let mut load_mw = vec![0.0; islands.members.len()];
        for load in network.loads().iter().filter(|load| load.in_service) {
            if let Some(island) = islands.island_of(load.bus) {
                load_mw[island] += load.p * mw;
            }
        }
        for (island, buses) in islands.members.iter().enumerate() {
            if supplied[island] {
                continue;
            }
            let ids: Vec<usize> = buses.iter().map(|&row| network.buses()[row].id.0).collect();
            diagnostics.push(SolveDiagnostic {
                code: "island_deenergized".to_owned(),
                message: format!(
                    "an island of {} bus(es) has no in-service generator and was left out \
                     of the solve; its {:.1} MW of load is unserved",
                    ids.len(),
                    load_mw[island]
                ),
                buses: ids,
            });
            isolate.extend_from_slice(buses);
        }
    }
    let designate = designate_references(network, &islands, &supplied, policy);
    let missing = match policy {
        ReferencePolicy::Stated => "states no reference bus",
        ReferencePolicy::GeneratorHosted => "has no reference bus hosting an in-service generator",
    };
    for &row in &designate {
        let bus = network.buses()[row].id.0;
        diagnostics.push(SolveDiagnostic {
            code: "island_reference_designated".to_owned(),
            message: format!(
                "an island {missing}; bus {bus} hosts its largest generator and became its \
                 reference"
            ),
            buses: vec![bus],
        });
    }
    if isolate.is_empty() && designate.is_empty() {
        return Energized {
            network: None,
            diagnostics,
        };
    }

    let mut adjusted = network.clone();
    for &row in &isolate {
        adjusted.buses_mut()[row].kind = BusType::Isolated;
    }
    for &row in &designate {
        adjusted.buses_mut()[row].kind = BusType::Ref;
    }
    let isolated: std::collections::HashSet<BusId> =
        isolate.iter().map(|&row| network.buses()[row].id).collect();
    for transformer in adjusted.transformers_3w_mut() {
        if transformer
            .windings
            .iter()
            .any(|winding| isolated.contains(&winding.bus))
        {
            transformer.in_service = false;
        }
    }
    Energized {
        network: Some(adjusted),
        diagnostics,
    }
}

/// The source bus row to promote to reference in each supplied island that has
/// no reference under `policy`: the bus of its largest in-service generator by
/// `pmax` (the first on a tie, a NaN bound counting as smallest), the rule
/// PowerIO normalization applies to a whole network.
fn designate_references(
    network: &BalancedNetwork,
    islands: &Islands,
    supplied: &[bool],
    policy: ReferencePolicy,
) -> Vec<usize> {
    let generator_buses: std::collections::HashSet<BusId> = network
        .generators()
        .iter()
        .filter(|generator| generator.in_service)
        .map(|generator| generator.bus)
        .collect();
    let mut referenced = vec![false; islands.members.len()];
    for (row, bus) in network.buses().iter().enumerate() {
        let counts = match policy {
            ReferencePolicy::Stated => true,
            ReferencePolicy::GeneratorHosted => generator_buses.contains(&bus.id),
        };
        if bus.kind == BusType::Ref && counts {
            if let Some(island) = islands.island_of_row[row] {
                referenced[island] = true;
            }
        }
    }
    let mut largest: Vec<Option<(f64, usize)>> = vec![None; islands.members.len()];
    for generator in network.generators().iter().filter(|g| g.in_service) {
        let Some(row) = islands.row_of.get(&generator.bus).copied() else {
            continue;
        };
        let Some(island) = islands.island_of_row[row] else {
            continue;
        };
        let pmax = if generator.pmax.is_nan() {
            f64::NEG_INFINITY
        } else {
            generator.pmax
        };
        if largest[island].is_none_or(|(best, _)| pmax > best) {
            largest[island] = Some((pmax, row));
        }
    }
    (0..islands.members.len())
        .filter(|&island| supplied[island] && !referenced[island])
        .filter_map(|island| largest[island].map(|(_, row)| row))
        .collect()
}

/// Connected components of the non-isolated buses.
struct Islands {
    /// Source bus row by bus id, for every bus.
    row_of: HashMap<BusId, usize>,
    /// Island of each source bus row; `None` for an isolated bus.
    island_of_row: Vec<Option<usize>>,
    /// Source bus rows of each island, in row order.
    members: Vec<Vec<usize>>,
}

impl Islands {
    fn of(network: &BalancedNetwork) -> Self {
        let buses = network.buses();
        let row_of: HashMap<BusId, usize> = buses
            .iter()
            .enumerate()
            .map(|(row, bus)| (bus.id, row))
            .collect();
        let active = |id: BusId| {
            row_of
                .get(&id)
                .copied()
                .filter(|&row| buses[row].kind != BusType::Isolated)
        };
        let mut parent: Vec<usize> = (0..buses.len()).collect();
        let mut join = |a: usize, b: usize| {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent[rb] = ra;
            }
        };
        for branch in network.branches().iter().filter(|b| b.in_service) {
            if let (Some(from), Some(to)) = (active(branch.from), active(branch.to)) {
                join(from, to);
            }
        }
        for transformer in network.transformers_3w().iter().filter(|t| t.in_service) {
            let rows: Vec<usize> = transformer
                .windings
                .iter()
                .filter_map(|winding| active(winding.bus))
                .collect();
            for pair in rows.windows(2) {
                join(pair[0], pair[1]);
            }
        }
        let mut island_of_root = HashMap::new();
        let mut island_of_row = vec![None; buses.len()];
        let mut members: Vec<Vec<usize>> = Vec::new();
        for (row, bus) in buses.iter().enumerate() {
            if bus.kind == BusType::Isolated {
                continue;
            }
            let root = find(&mut parent, row);
            let island = *island_of_root.entry(root).or_insert_with(|| {
                members.push(Vec::new());
                members.len() - 1
            });
            members[island].push(row);
            island_of_row[row] = Some(island);
        }
        Islands {
            row_of,
            island_of_row,
            members,
        }
    }

    fn island_of(&self, bus: BusId) -> Option<usize> {
        self.row_of
            .get(&bus)
            .and_then(|&row| self.island_of_row[row])
    }

    /// Whether each island hosts at least one in-service generator.
    fn supplied(&self, network: &BalancedNetwork) -> Vec<bool> {
        let mut supplied = vec![false; self.members.len()];
        for generator in network.generators().iter().filter(|g| g.in_service) {
            if let Some(island) = self.island_of(generator.bus) {
                supplied[island] = true;
            }
        }
        supplied
    }
}

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}

/// Group dense buses into islands over the branches of a prepared model. Returns
/// each bus's island label, labels numbered in order of each island's first bus.
#[cfg(feature = "sensitivity")]
pub(crate) fn dense_islands(n: usize, br_from: &[usize], br_to: &[usize]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    for (&from, &to) in br_from.iter().zip(br_to) {
        let (ra, rb) = (find(&mut parent, from), find(&mut parent, to));
        if ra != rb {
            parent[rb] = ra;
        }
    }
    let mut label = HashMap::new();
    (0..n)
        .map(|bus| {
            let root = find(&mut parent, bus);
            let next = label.len();
            *label.entry(root).or_insert(next)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupplied_islands_are_isolated_and_unreferenced_ones_designated() {
        let network = crate::model::parse_matpower(crate::model::CASE_ISLANDS).expect("parse");
        let energized = energize(&network, ReferencePolicy::Stated);
        let codes: Vec<(&str, &[usize])> = energized
            .diagnostics
            .iter()
            .map(|d| (d.code.as_str(), d.buses.as_slice()))
            .collect();
        assert_eq!(
            codes,
            [
                ("island_deenergized", &[7, 8][..]),
                ("island_reference_designated", &[6][..]),
            ]
        );
        assert!(energized.diagnostics[0].message.contains("20.0 MW"));
        let adjusted = energized.network(&network);
        let kinds: Vec<BusType> = adjusted.buses().iter().map(|bus| bus.kind).collect();
        assert_eq!(kinds[0], BusType::Ref);
        assert_eq!(kinds[5], BusType::Ref);
        assert_eq!(kinds[6], BusType::Isolated);
        assert_eq!(kinds[7], BusType::Isolated);
    }

    #[test]
    fn a_single_referenced_island_passes_through_uncopied() {
        let network = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        let energized = energize(&network, ReferencePolicy::Stated);
        assert!(energized.diagnostics.is_empty());
        assert!(energized.into_network().is_none());
    }

    #[test]
    fn a_generator_less_reference_counts_only_when_stated_references_do() {
        // Bus 1 is the reference but its generator is out of service; bus 3
        // keeps one.
        let mut network = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        network.generators_mut()[0].in_service = false;
        let stated = energize(&network, ReferencePolicy::Stated);
        assert!(stated.diagnostics.is_empty());
        let hosted = energize(&network, ReferencePolicy::GeneratorHosted);
        assert_eq!(hosted.diagnostics.len(), 1);
        assert_eq!(hosted.diagnostics[0].buses, [3]);
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn dense_islands_label_components_in_first_bus_order() {
        assert_eq!(
            dense_islands(5, &[0, 3, 2], &[1, 4, 4]),
            vec![0, 0, 1, 1, 1]
        );
    }
}
