# Saved Studies

A Study is **a saved case with its changes and results**. Open **Studies**, give
the current case a name, and select **Save study**. A planning goal is optional.
Saving keeps the current result when it matches the case; an unsolved case can
also be saved. Saving does not move the network or reset the camera.

## Case, History, and Plan

- **Case** shows the selected saved state. Demand rows show base demand, current
  demand, and the accumulated change in MW. Edits at different buses remain
  part of one cumulative set of changes.
- **History** holds saved states and activity. Select a state to inspect it or
  branch from it to explore another option. Observations, edits, solves,
  planning trials, and decisions have distinct labels. An observation adds
  evidence without inventing an electrical state.
- **Plan** is optional. Choose a goal, then explicitly choose the whole network,
  an area, or selected equipment. Searchable tables show bus names and IDs,
  line endpoints, weights, limits, and units. The displayed counts include
  every selected element; there are no hidden bus or line limits.

**Reset to base case** restores the original electrical inputs and retains
previous states and activity. The base is the network data supplied to the
Study, which can differ from its starting state when demand was already edited.
Older imported Studies without a base input report that reset is unavailable.

The saved state currently being inspected, the recommended candidate, and the
applied state remain distinct. Selecting a candidate changes the displayed
network. An agent proposal requires an explicit **Apply** action. **Live case**
returns to the case outside the Study.

## Planning

The OPF objective describes operating cost inside the power-system calculation.
A planning goal describes what to improve across candidate cases, such as a
weighted LMP or voltage target. Custom expressions remain available through
the structured API: weighted observables, sums, scaling, squared target
deviations, and direct intervention penalties.

Line capacity uses MW for DC OPF and MVA for SOCWR. Demand uses MW. Limits and
increments apply to the cumulative changes from the goal's starting state.
Demand placement allocates a stated additional total; redistribution preserves
the total through paired transfers.

**Find a proposal** uses the objective gradient to choose candidate edits,
solves each candidate, and retains verified improvements. Every attempted solve
counts against **Solve budget**, including failed trials and any required
starting-point solve. The result is the best verified candidate found within
that budget. It does not establish a global optimum. Expand the evidence for
prediction error, active-constraint changes, failed trials, and numerical
settings.

The derivative uses a combined adjoint calculation, including direct penalties,
without constructing a dense observable-by-decision matrix. Changing the goal
creates a new revision and retains earlier results. Changing a proposal's goal
or starting state invalidates its approval.

## Save, reopen, and continue

The browser stores completed operations atomically in IndexedDB. **Export**
creates a portable bundle containing PowerIO generation-2 inputs and solutions,
deduplicated SHA-256 artifacts, goals, states, and activity. Geographic layers,
line paths, drawings, and the selected view travel with it. **Import** checks
versions, hashes, identities, and references. It never restores approval tokens
or executes imported text. Older journals remain historical evidence when their
electrical states are unavailable.

A failed save reports the problem instead of discarding history. Free storage
or export the saved Study before retrying. Cancellation retains completed trials
and the best candidate after the running solve finishes.

## Native and agent access

```sh
cargo build -p tellegen-cli --features conic
tellegen describe
tellegen study create study.json < create-request.json
tellegen study inspect study.json
tellegen study run study.json < operation-request.json
tellegen study export study.json > portable-study.json
```

`tellegen describe` lists commands and generated JSON schemas. Rust definitions
also generate the TypeScript types. Browser controls, WebMCP, and the CLI use
the same Study operations and revision checks. PowerMCP invokes the CLI directly.

WebMCP `list_cases` and `select_case` change the visible case without proposal
approval. Network queries identify the displayed case and saved state, so an
LMP question refers to the same result shown on screen. Creating a Study is
needed only when saving or planning is requested.

Supported calculations are DC OPF, AC power flow, and, with the `conic` build
option, SOCWR. Unsupported objective and solver combinations are rejected
before planning.

## Model details

Texas7k uses an explicitly configured convex quadratic or linear fit to its
piecewise generator costs. **Model details** records the method, original and
prepared costs, affected generators, units, and measured breakpoint errors.
Those errors describe an approximation; they are not a relaxation bound.
The prepared costs persist through saved cases, exports, and reset.

The same preparation is available for any balanced case through
`tellegen prepare-model`. The command returns prepared PowerIO IR and its
approximation report. Other cases retain their declared costs by default.
