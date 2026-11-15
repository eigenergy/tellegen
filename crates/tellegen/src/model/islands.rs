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
//! network that needs neither decision passes through unchanged.

use powerio::BalancedNetwork;
use powerio_tx::diagnostics::codes;
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
    let mut adjusted = network.clone();
    let report = adjusted.assign_island_references(policy.powerio());
    // PowerIO states one finding per island it changed, in partition order,
    // with the island's de-energized buses and designated reference recorded
    // alongside in the same order. Its findings are reported as they stand:
    // the de-energized finding names the in-service load left unserved and,
    // for an island an HVDC line fed, the lines taken out of service with it
    // and the power they scheduled into it. A reference demoted because it
    // hosts no in-service generator is not reported, as before.
    let mut de_energized = report.de_energized_islands.iter();
    let mut designated = report.designated.iter();
    let diagnostics = report
        .diagnostics
        .iter()
        .filter_map(|finding| {
            let code = finding.code();
            if code == codes::CANONICALIZE_ISLAND_DE_ENERGIZED.code
                || code == codes::CANONICALIZE_ISLAND_HVDC_FED_DE_ENERGIZED.code
            {
                let island = de_energized.next()?;
                Some(SolveDiagnostic {
                    code: "island_deenergized".to_owned(),
                    message: finding.message().to_owned(),
                    buses: island.buses.iter().map(|bus| bus.0).collect(),
                })
            } else if code == codes::CANONICALIZE_ISLAND_REFERENCE_DESIGNATED.code {
                let bus = designated.next()?;
                Some(SolveDiagnostic {
                    code: "island_reference_designated".to_owned(),
                    message: finding.message().to_owned(),
                    buses: vec![bus.0],
                })
            } else {
                None
            }
        })
        .collect();
    // PowerIO's report says whether the assignment changed anything; a
    // network that needed nothing is prepared as given.
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
                ("island_reference_designated", &[6][..]),
                ("island_deenergized", &[7, 8][..]),
            ]
        );
        // PowerIO reports the islands in partition order.
        assert!(
            energized.diagnostics[1]
                .message
                .contains("20 MW of in-service load"),
            "{}",
            energized.diagnostics[1].message
        );
        let adjusted = energized.network(&network);
        let kinds: Vec<powerio::BusType> = adjusted.buses().iter().map(|bus| bus.kind).collect();
        assert_eq!(kinds[0], powerio::BusType::Ref);
        assert_eq!(kinds[5], powerio::BusType::Ref);
        assert_eq!(kinds[6], powerio::BusType::Isolated);
        assert_eq!(kinds[7], powerio::BusType::Isolated);
    }

    #[test]
    fn a_single_referenced_island_passes_through_unchanged() {
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

    #[test]
    fn an_island_fed_only_over_hvdc_reports_the_line_it_loses() {
        // Bus 4 has load and no generator; the line from bus 3 schedules 10 MW
        // into it. PowerIO de-energizes the island and takes the line out of
        // service with it, and its finding names the line and the infeed.
        let case = crate::model::CASE3.replace(
            " 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;\n",
            " 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;\n 4 1 5 0  0 0 1 1 0 230 1 1.1 0.9;\n",
        );
        let mut network = crate::model::parse_matpower(&case).expect("parse");
        let mut line = powerio::Hvdc::new(powerio::BusId(3), powerio::BusId(4));
        line.pf = 10.0;
        line.pt = 10.0;
        network.hvdc_mut().push(line);
        let energized = energize(&network, ReferencePolicy::Stated);
        assert_eq!(energized.diagnostics.len(), 1);
        let finding = &energized.diagnostics[0];
        assert_eq!(finding.code, "island_deenergized");
        assert_eq!(finding.buses, [4]);
        assert!(
            finding.message.contains("(3-4) schedule 10 MW into it"),
            "{}",
            finding.message
        );
        assert!(!energized.network(&network).hvdc()[0].in_service);
    }
}
