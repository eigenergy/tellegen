# Multiconductor AC power flow

Tellegen solves prescribed-load AC power flow on PowerIO's `MulticonductorNetwork`. Frederik Geth contributed the fixed-point solver, winding models, and independent comparison fixtures in [PR 109](https://github.com/eigenergy/tellegen/pull/109).

Import a BMOPF JSON case or load the 4-conductor example from Studies. Run AC power flow, then inspect terminal voltages, conductor currents, and source powers. Results use volts, degrees, amperes, watts, and var. A declared neutral also allows phase-to-neutral voltage comparisons. This calculation does not optimize generation or calculate LMPs.

Geographic GeoJSON can accompany the case or be attached to the selected case later. Bus points use equipment identities; line routes can use `bus_from` and `bus_to`. Geographic positions appear on the map. Drawing positions appear on a plain canvas. Attaching geometry retains the electrical input and any current result.

## Supported calculations

The solver uses one complex sparse LU factorization and updates compensated load currents until both voltage changes and physical KCL residuals meet their tolerances. Constant-power loads, ideal voltage sources, lines, shunts, explicit neutral conductors, and supported two-winding equipment are included. A two-winding entry expressed using BMOPF's n-winding representation is also supported.

Finite source impedance, active device controls, generator/IBR injections, unsupported load models, unsupported per-phase taps, ideal zero-leakage winding models, and more than two windings require additional numerical models. Such data can remain available for inspection, but the calculation reports unsupported physics instead of silently simplifying it. Draft BMOPF 0.2 data remains subject to Task Force review.

A current result belongs to the input that produced it. Cancelling or failing a new calculation retains the preceding result. Numeric columns exported to PowerIO are ordered by bus/terminal and source/terminal identities, with missing or duplicate identities rejected.

Save a result in Studies to retain its input, solver options, terminal values, and PowerIO solution. Import/export moves the saved result between browsers. Reopening checks that its input and results agree without solving again. A geographic attachment updates both saved modules and retains all electrical values. These versioned multiconductor snapshots do not offer the balanced-network planning objectives or sensitivities.

Raw OpenDSS files remain available for inspection. The multiconductor calculation requires supported BMOPF data or a typed PowerIO AC power flow input with explicit source and device settings.

## Native and browser APIs

The native API accepts a typed `McAcPfInstance`:

```rust,ignore
let result = tellegen::solve_mc_ac_pf_instance(&instance, &tellegen::McPfOptions::default())?;
let portable = result.to_powerio_solution(&instance)?;
```

The command line accepts either PowerIO IR or BMOPF JSON:

```sh
tellegen solve-mc < case.pio.json
tellegen solve-mc-bmopf '{"max_iterations":200}' < case.bmopf.json
```

`tellegen describe` includes the options and result schemas. The JSON result carries complex terminal voltages, currents, device powers, source reactions, iteration counts, and KCL residuals.

The browser package provides `solveMcModule(moduleJson, options, signal)` and `solveMcBmopf(text, options)`. The module operation runs in a separate worker so cancelling it leaves other calculations intact. Environments without workers check cancellation after synchronous execution and discard a cancelled result.

`solveMcStudy`, `replayMcStudy`, and `applyMcStudyGeo` create, reopen, and update saved multiconductor results. Native callers use `McStudySnapshot` for the same operations.

An agent can use `inspect_case`, `solve_multiconductor_pf`, and `query_network`. A solve requires the case ID and displayed revision. Querying `voltage_v` ranks buses by their largest terminal-to-ground voltage; `terminal_values` lists the individual terminals. `price` remains unavailable for AC power flow.

## Evidence

The test suite includes analytic resistive feeders and comparisons against OpenDSS reference results for grounded, floating, and impedance-grounded neutrals, three-phase lines, delta loads, and multiple winding connections. The published tolerances distinguish voltage, current, and complex-power comparisons. These fixtures establish the supported calculations; they do not establish universal convergence or multiconductor AC OPF support.

Further tests exercise the real WASM worker, unsupported-data rejection through IR reload, geographic points and multi-point routes after attachment, and unchanged electrical results after geographic edits.
