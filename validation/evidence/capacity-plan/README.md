# Large case evidence

The files in `specs/` are the complete capacity planning requests for CATS and
Texas7k. The runner parses each named MATPOWER file through PowerIO, adds its
SHA256 digest to the retained source descriptor, runs `tellegen plan`, checks
the solve accounting, and writes one JSON artifact. A successful artifact
contains the request, exact baseline and proposed summaries, every trial, and
the final PowerIO solution module's type, termination, objective, and canonical
JSON digest. The runner validates the complete module before reducing it to
this compact record.

Run from the repository root at a clean commit:

```sh
node validation/evidence/capacity-plan/run.mjs validation/evidence/capacity-plan/specs/cats.json validation/evidence/capacity-plan/results/cats.json
node validation/evidence/capacity-plan/run.mjs validation/evidence/capacity-plan/specs/texas7k.json validation/evidence/capacity-plan/results/texas7k.json
```

The runner refuses tracked or untracked changes other than earlier generated
JSON files in `results/`. It records the Tellegen commit and tree, the PowerIO
commit, the lockfile digest, the invocation spec digest, and the source digest.
For a published PowerIO release, every component crate must resolve to that one
registry version with a Cargo.lock checksum, and `powerio-releases.json` maps
the release tag to its commit. A candidate run
against an unpublished PowerIO revision instead pins one Git revision for all
eight component crates in `Cargo.toml` and `Cargo.lock`, plus `powerio-archive`
when the release links it (PowerIO 0.12 does); a linked `powerio-archive` must
resolve to the same source and version as the others. The runner
creates outputs with exclusive writes, so a second run cannot replace evidence
silently.

The checked-in CATS, Texas7k, and native-browser result files use the historical
generation-1 result format and identify their old PowerIO revision in `software`.
They remain audit records, not evidence for the v0.12 pin. A clean rerun writes
the generation-2 result format, whose `solution.version` is the PowerIO 0.12
`pio-ir` release string (for example `"0.12.0"`) rather than the integer `2`
that PowerIO 0.11 wrote, and must replace them before they are cited as
current.

`--allow-dirty` exists for local harness debugging and always marks the
artifact `reproducible: false`; do not check in such an artifact. Without that
flag, each case can write only its named path in `results/`. No large case
result is present or claimed until a clean run writes it.

An invocation can instead name one of the preparation failures enumerated by
`spec.schema.json`. If that exact failure occurs before the baseline solve, the
runner writes the same provenance plus a typed error and
`exact_solves_completed: 0`. An unlisted error, a message that does not match
the named failure, or success when failure was expected exits nonzero and
writes nothing.

## Native browser evidence

`native/in-app-browser.json` records the inspect, solve, query, proposal,
approval, apply, rejected mutation rollback, stale revision, and navigation
invalidation checks run through native WebMCP in the Codex in-app browser. It includes hashes for
the two screenshots and the demo video under `docs/assets/challenge/`.
`native/video-run.json` records the calls shown in that video, including the
second proposal that expires after a committed edit.
