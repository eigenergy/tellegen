# Tellegen product showcase

Tellegen is a power-system laboratory shared by a user and an agent. The network
stays central while a persistent Study records the goal, alternatives explored,
exact results and recommendation. OPF supplies the operating point; an outer
Study objective describes what the user wants to improve.

For example:

> Lower demand-weighted prices in this region. Add at most 20 MW across two
> lines, and show how prices elsewhere change.

The goal interpretation resolves the region to equipment identities and weights.
The user can inspect and edit that interpretation before exploration. A combined
implicit derivative ranks feasible interventions, and exact solves determine
whether a trial improves the objective. The Study preserves accepted, rejected
and failed trials, constraint changes and the reason exploration stopped.
Prediction error belongs in the expandable evidence beside those trials.

## One Study across interfaces

The browser controls and WebMCP call the same Study controller. Both can create a
Study, inspect its history, revise a goal, branch from a saved state, compare
candidates, propose interventions and attach evidence. Capacity-tool compatibility
adapters create the same persistent Studies. Native Rust and CLI operations use
the same generated contracts; PowerMCP invokes the CLI directly.

Three pointers keep navigation clear: the inspected state, the recommended state
and the applied state. Viewing an alternative does not apply it. Human approval
binds a particular proposal, starting state and goal. A revised goal or changed
proposal expires that approval.

Electrical inputs, instances and solutions use PowerIO generation-2 IR. Study
semantics live in a separate document with hashed, deduplicated artifacts.
Browser IndexedDB and atomic filesystem storage preserve completed operations.
A portable bundle can move from a browser to a headless agent and back. Import
validates identities, hashes and references, and restores no approvals.

## Showcase sequence

1. Open a congested case and inspect dispatch, prices and limiting branches.
2. State the regional objective, resolve its weights and review the intervention budget.
3. Ask for a proposal. Inspect exact improvements and consequences elsewhere.
4. Select a rejected candidate in the branching history to explain the explored alternative.
5. Revise the goal to conserve total demand and redistribute it among selected buses.
6. Compare candidates under the chosen goal revision. Expand a trial's numerical evidence.
7. Apply the reviewed recommendation through the explicit user control.
8. Export, reload and resume the Study with another agent.

A small AC power-flow example instead minimizes squared voltage-target error
through demand transfers. This uses the existing AC power-flow solver. Nonlinear
AC OPF and multiconductor solving are outside Tellegen's supported calculations.

## Reproducible evidence

[Studies](studies.md) describes the document and operations. The checked-in
[Study declarations and runner](https://github.com/eigenergy/tellegen/tree/codex/webmcp-challenge-v1/evidence/studies)
record executable and input hashes, exact trial outcomes, solve budgets and
numerical tolerances. Browser tests run the same declared Study through native
and WASM implementations and compare semantic results.

The Texas7k example explicitly records a lower-convex-cost scenario because the
original input contains nonconvex piecewise cost curves. Its results describe
that scenario, not an optimum for the original economic model.

The [earlier capacity demonstration](challenge-evidence.md) retains its own call
records, revisions and hashes. Fresh native WebMCP evidence accompanies the
persistent Study release. The showcase can be reproduced independently of a
contest submission.

## Verified examples

Fresh [result records](https://github.com/eigenergy/tellegen/tree/codex/webmcp-challenge-v1/evidence/studies/results)
include the declared tolerances and all attempted trials. The values below are
the outer objectives specified by each declaration; capacity and demand examples
sum selected nodal prices, while the AC example measures squared voltage error.

| Study | Starting objective | Recommended objective | Planning solves |
|---|---:|---:|---:|
| CATS capacity | 76.11900127 | 53.61631934 | 6 |
| CATS demand redistribution | 76.11900127 | 74.25936675 | 5 |
| Texas7k convex-cost scenario, capacity | 87.08117943 | 85.74773920 | 8 |
| Texas7k convex-cost scenario, redistribution | 87.08117943 | 87.08117943 | 3 |
| Three-bus AC voltage target | 0.00043750235 | 0.00040678296 | 6 |

Each Study also has one creation solve. The Texas7k redistribution search
retains its starting state: the tested changes are below its recorded improvement
tolerance. The planner reports that limit instead of presenting numerical noise
as a gain. None of these searches applies its recommendation automatically.

The [native WebMCP demonstration](https://github.com/eigenergy/tellegen/tree/codex/webmcp-challenge-v1/evidence/studies/native-webmcp)
records all seven Study tools, reload, branching, goal revision and a rejected
stale request. Application and stale approvals have separate browser tests. On its synthetic three-bus case, a 5 MW
capacity increase lowers the target bus price but raises the price at another
bus. A second goal explores demand transfers from the original starting state.
The inspected capacity choice, demand recommendation and applied starting
point remain distinct.

![Native Study comparison showing the target improvement and prices elsewhere](assets/challenge/study-comparison.png)

## References

- [WebMCP tool contract](webmcp.md)
- [WebMCP specification](https://github.com/webmachinelearning/webmcp)
- [Chrome WebMCP documentation](https://developer.chrome.com/docs/ai/webmcp)
- [Chrome WebMCP evaluations](https://developer.chrome.com/docs/ai/webmcp/evals)
