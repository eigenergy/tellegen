# Challenge Evidence

The checked in harness under `evidence/webmcp` produces the large case records.
CATS and Texas7k each have one complete invocation spec. The runner parses the
named MATPOWER source through the PowerIO revision in `Cargo.lock`, invokes
`tellegen plan`, checks its solve accounting, and writes one machine readable
result.

Each result records:

- the SHA256 digests of the MATPOWER source, invocation spec, and lockfile;
- the exact Tellegen commit and tree and the PowerIO release commit;
- the complete `CapacityPlanSpec` request;
- for success, the exact baseline and proposed summaries, every accepted and
  rejected trial, plus the kind, termination, objective, and canonical JSON
  digest of the final `powerio.module/1` solution on the amended instance.

A spec can name one enumerated preparation failure. A matching clean run
records a typed error, its preparation stage, and zero completed exact solves.
Any other failure exits nonzero and writes nothing.

Run from a clean repository checkout:

```sh
node evidence/webmcp/run.mjs evidence/webmcp/specs/cats.json evidence/webmcp/results/cats.json
node evidence/webmcp/run.mjs evidence/webmcp/specs/texas7k.json evidence/webmcp/results/texas7k.json
```

The runner refuses tracked or untracked changes other than earlier generated
JSON files in `results/`, and it refuses to replace an existing result.
`--allow-dirty` is only a harness smoke test; its output always says
`reproducible: false` and is not submission evidence. A clean run can write
only the named result path for its case.

During review, the lock resolves every PowerIO component from one Git commit.
After publication, it records the common version and each registry checksum;
`evidence/webmcp/powerio-releases.json` then supplies the checked release tag to
commit mapping. Both paths retain `powerio_revision` in the result.

Publish a numerical case claim only when its result file comes from a clean run
and validates against `evidence/webmcp/result.schema.json`.
