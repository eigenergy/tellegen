# WebMCP Challenge

Tellegen lets an agent inspect and change the power system case already open
in the user’s browser. The agent receives typed network and sensitivity data,
while the user sees the same selected elements, proposed edits, exact solves,
and approval controls.

The challenge workflow adds capacity planning to Tellegen’s existing OPF
interface. A user supplies a weighted LMP objective and bounds on branch
capacity increases. Tellegen differentiates that objective through the solved
DC OPF, uses the local derivative to choose trials, and solves each trial
exactly. The result is an unapplied proposal with its first order predictions
and exact outcomes.

## Why WebMCP is used

The browser holds state that a headless solver does not have: the imported
local file, current formulation, committed edits, map selection, pending
proposal, and visible approval. WebMCP gives an agent typed access to that
state without screen scraping and routes accepted changes through the same
controller and WebAssembly solver as manual edits.

The tool boundary is described in [WebMCP](webmcp.md). Seven tools cover
general OPF inspection and edits. `propose_capacity_plan` and
`apply_capacity_plan` add the planning workflow. The proposal tool changes no
electrical state. The apply tool exists only for a current proposal and still
requires the user’s one use approval.

## Evaluation

Deterministic tests check registration, schemas, runtime validation, revision
expiry, queued mutations, rollback after failed solves, approval consumption,
planner bounds, and visible state. Native browser runs then check the bridge
itself, including a call that supplies no callback options.

Large case evidence is generated from checked in request specifications and
records:

- the PowerIO source digest;
- the exact PowerIO and Tellegen revisions;
- the planning request;
- baseline and proposed exact results;
- the number of exact solves;
- every accepted and rejected trial; and
- the final PowerIO solution module's kind, termination, objective, and
  canonical JSON digest.

The commands, schemas, and publication rule are in
[Challenge Evidence](challenge-evidence.md). No CATS or Texas result is
published here until the corresponding artifact can be reproduced from the
recorded command and revisions.

## Demo sequence

The submission video uses this sequence in the live browser:

1. Open a congested DC OPF case and show its solved state.
2. Call `inspect_case` and query the most loaded branches and highest marginal
   demand values.
3. Call `propose_capacity_plan` with a stated weighted LMP objective, candidate
   branches, budget, increment, line count, and exact solve budget.
4. Compare the first order prediction with the exact trial results while the
   case remains unchanged.
5. Attempt application before approval and show `APPROVAL_REQUIRED`.
6. Approve the proposal in the interface and call `apply_capacity_plan`.
7. Show the committed revision and exact before and after results.
8. Stage another proposal, edit the case manually, and show that the stale
   proposal expires.

Screenshots and video for the submission must come from native WebMCP calls.
Playwright remains the regression suite, not the source of submission evidence.

## References

- [WebMCP specification](https://github.com/webmachinelearning/webmcp)
- [Chrome WebMCP documentation](https://developer.chrome.com/docs/ai/webmcp)
- [WebMCP evaluations](https://developer.chrome.com/docs/ai/webmcp/evals)
- [WebMCP Challenge](https://openai.com/webmcp-challenge/)
