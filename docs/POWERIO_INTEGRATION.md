# PowerIO 0.10 integration

This file defines the boundary between Tellegen and the PowerIO 0.10 public
beta. It also defines how the two repositories can change at the same time
without sharing branches, worktrees, or partially updated dependencies.

PowerIO and Tellegen solve different parts of the workflow. PowerIO reads,
classifies, represents, checks, and writes power system data. Tellegen builds
and solves calculations, computes implicit derivatives, and presents the live
browser state. Neither repository should duplicate the other one's public
types to make an integration test pass.

## The boundary

PowerIO owns:

- `Source`, format detection, and parser selection;
- `PioModule<T>`, the built in `PioValue` variants, and `PioValueKind`;
- structured diagnostics, source locations, and transformation history;
- `BalancedNetwork` and `MulticonductorNetwork`;
- `OperatingPoint<N>`, `TimeSeries<T>`, and `ScenarioSet<T>`;
- `DcPfInstance`, `AcPfInstance`, `McAcPfInstance`, `DcOpfInstance`,
  `AcOpfInstance`, `McAcOpfInstance`, and `AcScucInstance`;
- portable solution values;
- source faithful writing and explicit cross format conversion; and
- sparse matrix and graph outputs.

Tellegen owns:

- formulation choice and solver algorithms;
- private indexed arrays, equations, factorizations, and KKT systems;
- the reusable in-browser solver session;
- exact solves, implicit derivatives, previews, and bounded planning search;
- proposed and committed case edits;
- experiment records and the visible experiment journal; and
- browser selection, map state, approval, comparison, and undo.

The integration adapter lives in Tellegen. It converts PowerIO values into
Tellegen's private solver data and converts results back to a portable PowerIO
value when an exported result needs one. It must not recreate
`NetworkPackage`, publish normalized solver rows, or turn Tellegen's arrays
into a competing network model.

## What PowerIO means by IR

PowerIO's intermediate representation is a family of source neutral, typed
power system values. It is not one universal network schema.

`PioModule<T>` contains one typed value and the diagnostics, source map, and
history produced while reading or transforming it. A normal automatic parse
returns `PioModule<PioValue>` because the value kind is learned at run time.
Rust code can narrow that module to `PioModule<BalancedNetwork>`,
`PioModule<AcOpfInstance>`, or another registered type by moving the value and
the module records. Narrowing must not parse again, serialize through JSON, or
clone the network.

`PioValue` is the finite dynamic boundary used by automatic parsing, stored
`.pio.json`, and language bindings. It does not restrict typed Rust code:
`PioModule<ApplicationType>` remains valid even when `ApplicationType` is not
a `PioValue` variant.

This follows the useful compiler split:

1. PowerIO reads source syntax into a typed, source neutral value.
2. PowerIO performs explicit checked transformations between value families.
3. Tellegen lowers a supported network or calculation instance into private
   solver data.
4. Tellegen solves and derives sensitivities without changing the PowerIO
   value into its internal matrix layout.
5. PowerIO writes a portable result or converts a value to another source
   format when the user requests it.

The analogy stops there. PowerIO does not need an LLVM operation tree, global
context, dialect registry, or one preferred exchange format. `.pio.json` is
one versioned serialization of a dynamic module, not the definition of every
in-memory value.

## Terms used by Tellegen

An `OperatingPoint<N>` is a complete realized electrical state for network
`N`. It can include continuous electrical quantities and discrete equipment
states when the network model defines them. A solution adds the calculation
claim, termination information, and residuals needed to say how an operating
point was obtained.

`TimeSeries<T>` is ordered. `ScenarioSet<T>` contains named alternatives and
has no inherent time order. They compose as `ScenarioSet<TimeSeries<T>>`.
Tellegen should preserve that distinction in the browser and in WebMCP tool
results.

The current Tellegen API uses `Study` for the build-once solver object behind
`preview`, `commit`, and sensitivity calls. That word must not become a second
PowerIO data container or the name of the experiment record. Before the public
Tellegen API freezes, review whether `SolverSession` states the role more
clearly. If `Study` remains for compatibility, its documentation must define
it as one live solver session over one imported case and formulation.

The WebMCP interface records `Experiment` values in an experiment journal.
Capacity planning is one current experiment kind beside exact solves,
sensitivity requests, counterfactual previews, committed edits, and
formulation changes. Validation runs, operating interventions, and time or
scenario comparisons are future experiment kinds; they do not have to ship in
the challenge branch.

## Import and solve path

The target browser path is:

```text
named bytes or directory entries
  -> PowerIO Source
  -> automatic parse
  -> PioModule<PioValue>
  -> inspect kind and diagnostics
  -> select or narrow a supported typed value
  -> Tellegen private solver data
  -> exact solve, derivative, preview, or plan
  -> experiment record
  -> optional portable PowerIO result or source format write
```

The module stays alive while Tellegen needs source locations, diagnostics, or
same format writing. A browser adapter must not drop the module after copying
out a bare network.

For a time series or scenario set, selection is explicit. Tellegen can solve a
selected state, compare several states, or run a bounded batch, but it must not
silently treat an ordered series as unordered scenarios or clone a shared
network for every entry.

## Browser and WebAssembly requirements

The PowerIO path used by Tellegen must support:

- named in-memory input with no filesystem assumption;
- the facade and required component crates on `wasm32`;
- automatic value kind detection;
- typed module inspection and narrowing;
- read-only access to diagnostics, source spans, and stable element identity;
- balanced and multiconductor network values;
- calculation instances, operating points, time series, and scenario sets as
  their Tellegen workflows become available;
- explicit matrix requests with documented signs, units, and row mappings;
- same format writing and `.pio.json` when those operations are supported in
  the browser; and
- bounded summaries suitable for a WebMCP result.

The C ABI is not part of the WebAssembly path. The browser should use the Rust
and wasm bindings directly. JSON is acceptable as an external file or tool
encoding; it is not an excuse to serialize and reparse between two in-process
owners.

Local case bytes remain in the browser unless the user explicitly exports or
sends them. Experiment records store stable IDs, revisions, digests, edits,
solver settings, numerical summaries, and diagnostics needed to repeat a
permitted experiment when the same source digest and solver version are
available. They do not store raw imported source text in tool activity or
analytics.

## Structured diagnostics

PowerIO diagnostics remain structured through the browser boundary. Tellegen
must retain at least:

- severity;
- stable code;
- message;
- target;
- source span when present;
- related locations and details when present; and
- ordering.

The ordinary UI can render a short explanation. The full object remains
available for inspection, filtering, journal entries, PowerMCP, and WebMCP.
Do not flatten diagnostics into one string or require consumers to decode a
JSON string to reach fields already present in memory.

Source text, labels, diagnostic messages, and source targets are untrusted
input. WebMCP results mark them accordingly, cap their size, and never treat
their contents as instructions.

## Stable identity and revisions

Element IDs come from the typed PowerIO value and remain stable across the
Tellegen adapter. Dense matrix positions are separate and always carry an
explicit mapping. A browser edit, proposal, or journal entry names the stable
element ID, not a transient dense row number.

A Tellegen case revision covers all state that changes the meaning of a tool
call: imported module identity, selected time or scenario entry, formulation,
committed edits, and relevant solver settings. A short noncryptographic hash
of the case name and edits is not enough. Replacing a local file under the same
display name must invalidate existing proposals.

Planning proposals also carry a stable proposal ID, the starting case
revision, the objective definition, the feasible decision set, and the exact
candidate result. Applying a proposal rejects a stale revision and applies
only the edits that were reviewed. A failed exact solve leaves the active case
unchanged.

## WebMCP surface

The existing seven tools cover inspection, bounded network queries,
sensitivity columns, visible focus, a read-only edit preview, an exact update,
and reset. The next layer adds general experiment records and bounded planning
without turning every engine method into a browser tool.

The first planning objective is a weighted function of the optimal LMP vector:

```text
Phi(c) = w' * lambda*(c)
```

where `c` is the branch rating vector. The engine computes the derivative as a
vector-Jacobian product with one weighted adjoint solve. It must not construct
the complete buses-by-branches derivative matrix for this operation.

The planner uses that direction to generate feasible rating changes, solves a
small bounded set of candidates exactly, recomputes after an active-set change,
and returns an unapplied proposal. The decision set states the candidate
branches, per-branch bounds, increment, total MW budget, maximum changed lines,
and exact solve budget.

`propose_capacity_plan` leaves the electrical case, formulation, committed
edits, exact solution, selection, and revision unchanged. It creates bounded
ephemeral proposal and journal state and stages the proposal in the visible
interface. Classify its WebMCP annotation against the current official meaning
of `readOnlyHint`; do not call it read only merely because the electrical case
does not change. `apply_capacity_plan` is revision-bound and proposal-bound.
The browser shows the proposal before it can be applied. A visible Approve
control grants one use for the exact proposal and revision; the tool cannot
approve its own proposal. Applying without that approval returns
`APPROVAL_REQUIRED`. Apply consumes the approval, and any revision change
invalidates it. The experiment journal links the starting exact solve,
predicted changes, exact candidates, selected proposal, human decision,
applied solve, and prediction error through one experiment ID.

Planning does not define the journal. The current journal also records ordinary
OPF solves, formulation changes, direct counterfactual edits, and sensitivity
experiments. Validation and time or scenario comparisons are later experiment
kinds.

## Sequential repository handoff

This root-checkout copy is bootstrap input for the Tellegen WebMCP goal. Once
the goal commits it, the exact remote branch version and its recorded SHA-256
digest are canonical. Neither goal relies on later edits to this untracked
bootstrap copy.

PowerIO finishes first. Tellegen starts only after PowerIO and PowerIO.jl have
frozen green release-candidate heads and PowerIO has emitted its final producer
receipt and immutable CI evidence.

| Repository | Writer | May read | Must not change |
|---|---|---|---|
| PowerIO and PowerIO.jl | PowerIO remediation session, first | Tellegen only for migration inventory | Tellegen files, branches, worktrees, PRs, or issues |
| Tellegen | WebMCP session, second | Frozen PowerIO heads and producer evidence | PowerIO or PowerIO.jl files, branches, worktrees, PRs, or issues |

Each session uses its own worktree. Neither session runs `git checkout`,
`git rebase`, `git fetch`, or stack commands in the other session's repository
or worktree. A session that needs to compile the other repository creates a
session-unique clone with `mktemp -d` under `/private/tmp`, records the exact
remote SHA, and never reuses or removes the other session's temporary path.

The handoff sequence is:

1. PowerIO completes its own implementation, binding, release, oracle,
   performance, and in-scope consumer gates.
2. PowerIO commits `evals/integration/powerio-candidate.json`. The tracked data
   names the tested subject commit, observed PowerIO.jl release-pair commit,
   package version, C ABI version, stored schema version, wasm feature set,
   public schema digests, and public interface changes.
3. PowerIO pushes, waits for green checks, freezes both heads, and emits
   immutable CI evidence using `GITHUB_SHA`, the run ID, and tracked receipt
   digest. It reports `POWERIO RC READY: YES` and
   `TELLEGEN INTEGRATION: PENDING`.
4. Tellegen verifies that exact handoff before changing its adapter. It checks
   out the frozen subject commit in a session-unique isolated clone rather than
   reading the PowerIO working checkout.
5. Tellegen records `evals/powerio/compatibility.json` with the tested commits,
   commands, pass/fail/skip counts, schema digests, and browser and WebAssembly
   paths exercised. PowerIO.jl is an observed paired release identity; Tellegen
   does not claim to test Julia.
6. If the defect belongs to Tellegen, only Tellegen changes. If it belongs to
   the general PowerIO interface, Tellegen reports the failing operation and
   regression test and the PowerIO remediation resumes separately.

A later PowerIO change to a public Rust or binding API, wasm feature closure,
diagnostic shape, stable ID or matrix mapping, sign or unit rule, or serialized
form invalidates the Tellegen receipt. An internal or prose-only change gets a
recorded diff review instead of an automatic rebuild.

After the Tellegen branch is pushed and frozen, its CI creates the consumer
attestation using `GITHUB_SHA`, the PowerIO subject commit, producer receipt
digest, run ID, and tracked Tellegen receipt digest. No tracked file claims its
own containing commit SHA.

## Integration checkpoints

Check the frozen PowerIO and PowerIO.jl heads before the first Tellegen change,
after the adapter and browser profile pass, and before final Tellegen review.
Record the PR number, full SHA, check state, producer digest, and whether the
head changed. Any change invalidates the handoff and requires a new producer
receipt before Tellegen continues.

## Full PowerIO 0.10 integration gates

These gates define the complete integration target against the frozen PowerIO
candidate. Capabilities outside Tellegen's bounded browser profile remain
explicitly unsupported or deferred unless the challenge demo or submission
claims them.

The full PowerIO integration is complete only when:

1. Browser import returns or retains the PowerIO module rather than a copied
   bare network.
2. Automatic parse, typed value inspection, diagnostics, and same format
   writing work for the supported browser profile.
3. The WebAssembly build uses the final documented PowerIO feature set and no
   C ABI.
4. Every exposed balanced or multiconductor browser path preserves stable
   element IDs. An unsupported kind returns a structured refusal without data
   loss.
5. Matrix signs, units, and row mappings match the final PowerIO candidate.
6. Structured diagnostics remain accessible as fields and render clearly.
7. When time series or scenario selection is exposed, it preserves order and
   shared data. An unimplemented selection returns a structured refusal.
8. Local file bytes remain local during WebMCP workflows.
9. Immutable CI attestations name the exact frozen PowerIO and Tellegen heads,
   tracked receipt digests, and run IDs. PowerIO.jl is identified separately
   as the observed release pair.
10. A later public PowerIO change invalidates and reruns the affected receipt.

The broader WebMCP stack has additional engine, planning, journal, browser,
security, performance, and challenge evidence gates in its implementation
goal. Passing this file's integration checks alone does not make that stack
ready to merge.
