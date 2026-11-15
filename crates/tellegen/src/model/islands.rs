//! Island energization: the one place that decides which islands a solve
//! carries and where each is referenced, before PowerIO prepares the problem.
//!
//! PowerIO owns the rule. `BalancedNetwork::calc_islands` partitions the
//! energized buses (joined by in-service branches, closed switches, and
//! in-service three-winding transformers), and
//! `BalancedNetwork::assign_island_references` de-energizes every island no
//! in-service generator supplies and designates a reference, at the bus of the
//! largest generator, in each supplied island that has none. This module picks
//! the policy and reports each decision as a [`SolveDiagnostic`].
//!
//! PowerIO's preparation already grounds every reference it is given, so a
//! network that needs neither decision passes through unchanged and uncopied.

use powerio::BalancedNetwork;
use powerio_tx::IslandReferencePolicy;

use crate::api::SolveDiagnostic;

/// Which stated references count when deciding whether a supplied island needs
/// one designated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferencePolicy {
    /// Any bus typed reference (PowerIO's `DesignateMissing`). The
    /// typed-instance preparations use the network's bus types as stated.
    Stated,
    /// Only a reference bus that hosts an in-service generator (PowerIO's
    /// `DesignateMissingSupplied`, which demotes one that hosts none to PQ).
    /// PowerIO normalization demotes a generator-less reference to PQ, so the
    /// paths that normalize first need a generator-hosted one.
    GeneratorHosted,
}

impl ReferencePolicy {
    fn powerio(self) -> IslandReferencePolicy {
        match self {
            Self::Stated => IslandReferencePolicy::DesignateMissing,
            Self::GeneratorHosted => IslandReferencePolicy::DesignateMissingSupplied,
        }
    }
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

/// De-energize the unsupplied islands of `network` and designate a reference
/// in each supplied island that has none under `policy`, through PowerIO's
/// `assign_island_references`.
pub(crate) fn energize(network: &BalancedNetwork, policy: ReferencePolicy) -> Energized {
    // PowerIO's partition says whether the assignment would change anything,
    // so a network that needs nothing is not copied.
    let generators = network.generators();
    let partition = network.calc_islands();
    let needs_assignment = partition.islands.iter().any(|island| {
        let hosts = |bus| {
            island
                .generators
                .iter()
                .any(|&row| generators[row].bus == bus)
        };
        !island.is_supplied()
            || island.references.is_empty()
            || (policy == ReferencePolicy::GeneratorHosted
                && island.references.iter().any(|&bus| !hosts(bus)))
    });
    if !needs_assignment {
        return Energized {
            network: None,
            diagnostics: Vec::new(),
        };
    }

    let mut adjusted = network.clone();
    let report = adjusted.assign_island_references(policy.powerio());
    let mw = if network.is_normalized() {
        network.base_mva()
    } else {
        1.0
    };
    let mut diagnostics: Vec<SolveDiagnostic> = report
        .de_energized_islands
        .iter()
        .map(|island| SolveDiagnostic {
            code: "island_deenergized".to_owned(),
            message: format!(
                "an island of {} bus(es) has no in-service generator and was left out of the \
                 solve; its {:.1} MW of load is unserved",
                island.buses.len(),
                island.unserved_load * mw
            ),
            buses: island.buses.iter().map(|bus| bus.0).collect(),
        })
        .collect();
    let missing = match policy {
        ReferencePolicy::Stated => "states no reference bus",
        ReferencePolicy::GeneratorHosted => "has no reference bus hosting an in-service generator",
    };
    diagnostics.extend(report.designated.iter().map(|bus| SolveDiagnostic {
        code: "island_reference_designated".to_owned(),
        message: format!(
            "an island {missing}; bus {bus} hosts its largest generator and became its reference"
        ),
        buses: vec![bus.0],
    }));
    let changed = !report.designated.is_empty()
        || !report.demoted.is_empty()
        || !report.de_energized.is_empty();
    Energized {
        network: changed.then_some(adjusted),
        diagnostics,
    }
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
        let kinds: Vec<powerio::BusType> = adjusted.buses().iter().map(|bus| bus.kind).collect();
        assert_eq!(kinds[0], powerio::BusType::Ref);
        assert_eq!(kinds[5], powerio::BusType::Ref);
        assert_eq!(kinds[6], powerio::BusType::Isolated);
        assert_eq!(kinds[7], powerio::BusType::Isolated);
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
}
