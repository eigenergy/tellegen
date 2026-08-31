# Large case evidence

The files in `specs/` are the complete capacity planning requests for CATS and
Texas7k. The runner parses each named MATPOWER file through PowerIO, adds its
SHA256 digest to the retained source descriptor, runs `tellegen plan`, checks
the solve accounting, and writes one JSON artifact. A successful artifact
contains the request, exact baseline and proposed summaries, every trial, and
the final PowerIO solution module's kind, termination, objective, and canonical
JSON digest. The runner validates the complete module before reducing it to
this compact record.

Run from the repository root at a clean commit:

```sh
node evidence/webmcp/run.mjs evidence/webmcp/specs/cats.json evidence/webmcp/results/cats.json
node evidence/webmcp/run.mjs evidence/webmcp/specs/texas7k.json evidence/webmcp/results/texas7k.json
```

The runner refuses tracked or untracked changes other than earlier generated
JSON files in `results/`. It records the Tellegen commit and tree, the PowerIO
commit, the lockfile digest, the invocation spec digest, and the source digest.
Before PowerIO 1.0.0 is published, the common Git dependency supplies the exact
reviewed candidate commit. After publication, every component crate must
resolve to one registry version with a Cargo.lock checksum, and
`powerio-releases.json` must map that release tag to its commit. The runner
creates outputs with exclusive writes, so a second run cannot replace evidence
silently.

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
