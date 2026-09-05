# Changelog

Written by release-plz from the commits between tags. Releases through 0.1.1,
when all three surfaces moved together, are in the
[root `CHANGELOG.md`](../../CHANGELOG.md).

## [0.3.0](https://github.com/eigenergy/tellegen/compare/v0.2.0...v0.3.0) - 2026-09-05

### Added

- *(studies)* persist demand adjustments and restore original network data

### Other

- [**breaking**] document the PowerIO 0.11 input contract
- Pin final PowerIO transformer corrections and conic Study contracts
- Verify singular outer derivatives and document cumulative Study budgets
- Add persistent Studies with composable objectives and bounded exploration
- Integrate final PowerIO 0.11 candidate and export verified WebMCP experiments
- Document the universal PowerIO display parser
- Fix v0.11 lockfile and Rust formatting
- Align Tellegen with PowerIO v0.11 IR generation 2
- Move Tellegen onto the PowerIO 1.0 module API
- Remove release bypasses and stale claims
- Use PowerIO modules for OPF and capacity planning

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
