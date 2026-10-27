# Changelog

Written by release-plz from the commits between tags. Releases through 0.1.1,
when all three surfaces moved together, are in the
[root `CHANGELOG.md`](../../CHANGELOG.md).

## [0.4.0](https://github.com/eigenergy/tellegen/compare/v0.3.0...v0.4.0) - 2026-10-24

Released with `@tellegen/engine` and `@tellegen/svelte` 0.4.0.

### Changed

- [**breaking**] A multiconductor session edit is constant-size.
  `McPfSession::replace_load_powers` returns the `McPfSummary` of the new
  operating point instead of the full result, and `McPfSession::result` and
  `McPfSession::instance` are removed. Call `build_result()` for the full
  terminal and equipment result, `detail()` for one bus and a page of
  equipment ports, and `base_instance()` or `edited_instance()` for the
  instance. ([#154](https://github.com/eigenergy/tellegen/pull/154))
- Built against PowerIO 0.11.6 (0.3.0 used 0.11.1), with PowerIO's `sincal`
  feature off, so no bundled SQLite reaches the build. PSS/E cases read
  closer to how PSS/E defines them: the load `YQ` sign and distributed
  generation, the two-winding magnetizing admittance at bus I, impedance
  scaling for a winding 2 ratio other than one, per-winding three-winding
  status and ratings, and switched shunt `MODSW`. MATPOWER cases that convert
  units after their literal data read with the converted values. Solves of
  such cases change. ([#195](https://github.com/eigenergy/tellegen/pull/195),
  [#203](https://github.com/eigenergy/tellegen/pull/203))

### Other

- Centre-tap transformer regression fixtures against OpenDSS, including a
  case that exercises the neutral impedance.
  ([#142](https://github.com/eigenergy/tellegen/pull/142))
- The README cites the ACM DXConf 2026 paper by its published title and
  states the MIT license.

## [0.3.0](https://github.com/eigenergy/tellegen/compare/v0.2.0...v0.3.0) - 2026-09-23

The release that accompanies the ACM DXConf 2026 paper *Interactive Power Flow
in the Browser* ([doi:10.1145/3847238.3849038](https://doi.org/10.1145/3847238.3849038)).

### Added

- Multiconductor AC power flow (`mc-pf` feature): a fixed-point current-injection
  solver for unbalanced distribution networks from OpenDSS, PMD JSON, and BMOPF
  inputs, with explicit neutral conductors, finite-leakage transformers, the
  OpenDSS load-voltage envelope, and fixed regulators. Terminal voltages,
  currents, and powers are compared with OpenDSS in CI. By Frederik Geth.
- Phase-to-phase single-phase and centre-tap transformers, assembled as a
  coupled three-winding primitive with winding polarity, fixed taps, excitation
  shunts, and floating, solid, or impedance-grounded centre taps. Arbitrary
  multiwinding transformers are still rejected. By Frederik Geth (#128).
- `McPfSession`: a retained session that keeps the prepared network, frozen
  compensation matrix, sparse LU, and last converged voltage, so per-load P/Q
  edits re-solve warm with one factorization. By Frederik Geth (#129).
- Persistent Studies with composable objectives and bounded exploration;
  *(studies)* persist demand adjustments and restore original network data.
- Study operations (`study_ops`) and a filesystem Study store shared by the
  CLI, the browser, and the new Python binding (`crates/tellegen-py`, not
  published to crates.io). By Qian Zhang (#133).

### Changed

- [**breaking**] The PowerIO 0.11 input contract is documented and required:
  portable electrical modules use PowerIO IR generation 2 through the PowerIO
  module API, for OPF and capacity planning alike. Depends on PowerIO 0.11.
- Study content IDs and preparation hashes use sha2 0.11 and stay available in
  reduced-feature builds.

### Fixed

- Saved distribution selection, feature-gated builds, and SHA-256 preparation
  IDs.
- Centre-tap terminal maps are refused when no neutral or earth convention
  identifies the centre tap, instead of lowering to an unchecked orientation.

## [0.2.0](https://github.com/eigenergy/tellegen/compare/v0.1.1...v0.2.0) - 2026-08-23

### Added

- [**breaking**] mark DcNetwork non-exhaustive

### Other

- Give every changelog a single owner
- Close core model findings: two panics and a wrong answer from untrusted case data
- Close review findings on the stacked release work
- Format and lint final PowerIO model changes
- Close final model and study review findings
- Close remaining PowerIO model invariants
- Reject ambiguous canonical element identities
- preserve final stack review fixes
- Automate release and deployment recovery
- Format PowerIO model changes
- Close release and deployment blockers
- Add trusted release engineering
- Consume PowerIO 0.9 byte and DC contracts
- Repin powerio, following its field renames and non-finite float spelling
- Upgrade to powerio 0.9
