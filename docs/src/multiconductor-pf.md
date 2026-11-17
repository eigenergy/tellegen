# Multiconductor AC power flow

Tellegen solves prescribed-load AC power flow on PowerIO's `MulticonductorNetwork`. Frederik Geth contributed the fixed-point solver, winding models, and independent comparison fixtures in [PR 109](https://github.com/eigenergy/tellegen/pull/109).

Import a BMOPF JSON case or load the 4-conductor example from Studies. Run AC power flow, then inspect terminal voltages, conductor currents, and source powers. Results use volts, degrees, amperes, watts, and var. A declared neutral also allows phase-to-neutral voltage comparisons. This calculation does not optimize generation or calculate LMPs.

Geographic GeoJSON can accompany the case or be attached to the selected case later. Bus points use equipment identities; line routes can use `bus_from` and `bus_to`. Geographic positions appear on the map. Drawing positions appear on a plain canvas. Attaching geometry retains the electrical input and any current result.

## Supported calculations

The solver uses one complex sparse LU factorization and updates compensated load currents until both voltage changes and physical KCL residuals meet their tolerances. Constant-power, constant-current, constant-impedance, ZIP, and exponential loads are included, along with ideal voltage sources, lines, shunts, explicit neutral conductors, and supported winding equipment. Phase-to-phase single-phase transformers, a two-winding entry expressed using BMOPF's n-winding representation, and PowerIO's three-coupled-winding lowering of a BMOPF centre-tap transformer are supported.

Finite source impedance, active device controls, generator/IBR injections, unsupported load models, unsupported per-phase taps, ideal zero-leakage winding models, and arbitrary multiwinding transformers require additional numerical models. Such data can remain available for inspection, but the calculation reports unsupported physics instead of silently simplifying it. Draft BMOPF 0.2 data remains subject to Task Force review.

A current result belongs to the input that produced it. Cancelling or failing a new calculation retains the preceding result. Numeric columns exported to PowerIO are ordered by bus/terminal and source/terminal identities, with missing or duplicate identities rejected.

After an initial solve, select a bus to edit each attached load branch's active and reactive power. The browser coalesces rapid changes and automatically warm-solves from the last converged voltages. It retains the same sparse factorization because load power changes alter the nonlinear current injection, not the passive network operator. The physical impedance and low- and high-voltage fallback terms still follow the edited power; only the fixed-point compensation reference is frozen. Resetting a row removes its override, and saved results contain the edited input and its qualified result. An edit changes only the solver's load laws: the network is not copied, and each edit returns a small summary in one round trip. The Studies panel then fetches only the selected bus and one page of equipment results. The portable input, optional canonical solution, and complete result are built only for save, export, geography attachment, or an explicit full rebuild. [Multiconductor PF performance](benchmark-results.md#mc-pf-performance) states the latency target and payload budgets with measurements.

## Solver settings and OpenDSS comparison

Open **Settings** from the header, Network or Studies to edit the next explicit run.
Choose **Apply settings**, then run explicitly. See the [common settings menu and API](settings.md). Network and
Studies share these settings. Editing a value does not solve; use the single
context-sensitive solve button. A successful solve scrolls Studies to the overview
with violations, losses and voltage ranges. Failed runs retain the preceding result
and its recorded settings. Reopening a saved study restores the settings that
produced that result.

| Setting | Default | Meaning |
| --- | --- | --- |
| Load-model fallback (`voltage_envelope`) | Enabled | Permit the low/high-voltage fallback described below. |
| Vlow (`v_low_pu`) | 0.50 pu | Below this breakpoint, use nominal constant impedance. |
| Vmin (`v_min_pu`) | 0.85 pu | Lower end of the original load-law band; below it, transition toward the low-voltage impedance law. |
| Vmax (`v_max_pu`) | 1.15 pu | Above this breakpoint, use impedance matched to the original law at Vmax. |
| Maximum iterations (`max_iterations`) | 100 | Maximum number of fixed-point iterations. |
| Damping (`damping`) | 1 | Full voltage update; values in (0, 1) damp the update. |
| Voltage-change tolerance (`tolerance`) | **1e-4 V** | Maximum absolute change in complex terminal voltage between iterations. |
| Zero-voltage tolerance (`zero_voltage_tolerance`) | 1e-9 V | Near-zero voltage guard when evaluating load currents; not a convergence tolerance. |
| Absolute KCL tolerance (`absolute_kcl_tolerance`) | **1e-4 A** | Absolute current-mismatch allowance at each unknown terminal. |
| Relative KCL tolerance (`relative_kcl_tolerance`) | **1e-5** | Dimensionless current-mismatch allowance scaled by incident-current magnitude. |

The three convergence tolerances were previously 1e-8 V, 1e-6 A and 1e-8,
respectively. The new defaults reduce unnecessary iterations in interactive
studies while retaining a physical current-balance check. All other defaults are
unchanged. These are stopping criteria, not guaranteed bounds on solution error;
use tighter tolerances for sensitive comparisons or small differences between
large quantities. Frozen numerical-oracle tests retain their explicit tight
settings.

Convergence requires **both** the voltage-change check and, at every unknown
terminal, `|KCL residual| <= absolute_kcl_tolerance + relative_kcl_tolerance * I_scale`.
`I_scale` is the larger of the summed incident device-current magnitudes and the
magnitude of the net load current. The reported scaled residual must be at most 1.
For example, at a 100 A scale the new KCL allowance is 0.0011 A.

OpenDSS documents a default solution tolerance of **0.0001**, applied to voltage
magnitude changes and normalized by the bus voltage base when one is defined.
Tellegen's **1e-4 V is not 1e-4 pu**, and its complex-voltage and KCL checks are not
OpenDSS's stopping rule. At a 240 V base, 1e-4 pu corresponds to 0.024 V, versus
Tellegen's 0.0001 V absolute update threshold. There is no universal conversion
from OpenDSS's voltage tolerance to Tellegen's KCL tolerances. This change follows
the practical goal of avoiding excessive iteration; it does not claim identical
accuracy, iteration counts, or results. See EPRI's [solution options](https://opendss.epri.com/Options.html)
and the OpenDSS maintainer's [explanation of tolerance](https://sourceforge.net/p/electricdss/discussion/861976/thread/15ea059b/).

OpenDSS's documented load defaults are **Vminpu=0.95, Vmaxpu=1.05, Vlowpu=0.50**.
Tellegen deliberately retains its wider **0.85–1.15 pu** band and its existing
100-iteration cap. Users can choose 0.95 and 1.05 for a closer load-band comparison,
but that alone does not make the two solvers equivalent. These load thresholds
change equations; they are separate from engineering voltage limits and the
Overview's violation counts. See EPRI's [load properties](https://opendss.epri.com/Properties7.html).

The fallback moves a constant-power load **toward constant impedance**, not toward
constant power. EPRI explains this as a way to maintain convergence at extreme
voltages in [Load Models That Nearly Always Converge](https://opendss.epri.com/LoadModelsThatNearlyAlwaysConver.html).
Tellegen makes the consequence explicit: after convergence it checks the original
load law at the final voltages. A result that used changed equations is labeled
**Modified load model — non-canonical result** in the UI and exports. Disabling
the fallback evaluates the original equations at all voltages, without promising
convergence. Merely going outside the band does not mark an unchanged impedance
load as modified.

## Energized-network scope and saved results

Loading a case detects bus islands without a voltage source. Network and Studies
show **Run power flow** when all islands have source coverage, or **Solve energized
network** with counts and an expandable exclusion list before the first run.
With no source-covered island, solving is disabled and the full case remains
inspectable. Clicking the energized action explicitly selects the source-covered
subset; loading alone does not select it. This connectivity check does not prove
conductor-level solvability. Retained inputs are prepared inside the engine, so
selecting energized islands and rerunning with new settings does not serialize
the full network into IR text.

All buses and equipment stay in the map, diagram and saved input. Excluded buses
have no calculated result; they are not reported as zero voltage or zero load.
Unsupplied loads are omitted from solved totals. Scope and load-model fidelity are
separate qualifications and both are retained in CSV and Study JSON. Component
coordinates on excluded equipment remain in the full input; only attachments to
retained equipment accompany the solved subset.

Version 2 studies save the input, effective settings, qualification and full
result. A canonical PowerIO solution module is included only for an original-model
result; otherwise `solution_module` is null. Version 1 studies are reassessed on
replay. Replay checks exclusions, model qualification, and terminal, source and
component quantities against the saved voltages and network equations without
running another power flow. Unconverged iterates remain unavailable.

## Operating-point overview

Above the detailed results, the Studies panel summarises the displayed operating point: limit violations, losses, the voltage range for each declared voltage reference, the most heavily loaded branch, and the largest voltage unbalance. Selecting a tile selects its bus or branch on the map. Issues are ranked with violations first, then by how far they exceed their limit: voltages outside the case's declared phase-to-neutral or phase-to-phase limits, and branches at 90 % of their declared rating or more (a violation above 100 %). A transformer's loading is its apparent power at each bus it connects, against its largest winding rating there, so the two halves of a centre tap are judged together; a transformer with several windings counts once in the losses and the issue list. A case that declares no limits or ratings shows no issues and says so; nothing is ranked against invented bounds.

For a selected bus, bars place each terminal-pair voltage within its declared band. Unbalance is the voltage unbalance factor |V2|/|V1| from terminal phasors, shown beside the phase-to-neutral magnitude spread; when the spread is much larger, the difference is neutral shift, which the unbalance factor excludes. For a selected branch, bars show the current in every conductor at both ends on one scale, with the neutral's share of the largest phase current.

*Show diagram* in the Overview replaces the map with the diagram renderer. The same button in the case's Network panel opens it before any solve, for topology and equipment alone: useful for cases the power flow cannot solve yet, such as those with generators or inverter-based resources. Colouring and values appear once a solve succeeds. It opens on the case's own positions, *Geographic* over the same basemap, or *Source positions* for a drawing, so the view continues from the map: in *Geographic* it opens at the map's own view when that shows part of the feeder, and closing from *Geographic* returns the map to the diagram's view. *Schematic* then switches to a schematic of the case, laid out by the engine; that layout is computed on opening, so the switch is immediate. Buses are placed by branch steps from the sources, with tree branches drawn as right-angled routes. *Tidy* packs subtrees as closely as their rows allow; *Leaf order* gives every leaf its own row; branches that close a loop and open switches are drawn straight so they cannot be mistaken for the tree. *Geographic* (or *Source positions* for a drawing) shows the case's own positions instead, and switching between the two moves each bus from where it is on screen to its new place, keeping the selection; it switches instantly when the system asks for reduced motion. Colour by voltage within the declared band or by loading as a percentage of the rating; a faint underlay keeps the network visible where results are unremarkable. PNG and SVG exports of the visible view carry a title block with the case, layout, colouring, camera and solve. *Layout* downloads the schematic as a PowerIO diagram layer (`.geo.json`, `powerio_geo.space = "diagram"`, positions marked as computed): one point per bus and one route per line, matched by line id. Load it together with the case to use the schematic as the case's drawing, or pass it to PowerIO tooling. In source positions the schematic follows the routes the case carries.

Equipment is drawn with symbols once they are large enough to read. A transformer shows as two overlapping winding circles, drawn once however many windings it has. A three-phase unit carries each winding's connection inside its circle (Δ, Y, or Y with a tail when its neutral is brought out, IEC 60617), a split-phase unit a tap brought out below its secondary, and a single-phase unit plain circles; zoomed out, the fixed-size transformer markers keep these marks. Closest in, a label gives the vector group without its clock number and the rating, such as "Dyn · 500 kVA" or "1φ split · 25 kVA". A switch is a blade between two contacts, lifted when the switch is open; the branch line breaks where the symbol sits, and a plain line has no symbol. Loads, generators, inverter-based resources, sources and shunts hang below their bus, one symbol per kind and connection with a count when several connect alike: three single-phase loads on different phases are three symbols. Each symbol's drop carries its conductors the way a line does (one slash per phase, a circled slash for the neutral), and closest in a label gives its terminals in the model's own names, led by its connection: "a–n", "Y 123n", "Δ abc". Several symbols of one kind hang side by side from a short rail, so they do not read as in series. A shunt is drawn as a general admittance block because BMOPF shunts can be conductive as well as capacitive. A shunt connected to its bus's neutral and nothing else is a neutral earth, such as an MEN electrode, and is drawn as an impedance box in series with the earth symbol (three shrinking bars, IEC 60617); it is not drawn as a solid earth, since a shunt always has a finite admittance. A bus's perfectly grounded terminals are drawn as a solid earth, the earth symbol alone, in a slot of their own; its tooltip names the terminals. In the schematic each bus has room below it for its symbols, so they never overlap the next row. Hover a symbol for its ids, or click it to select its branch or bus. When colouring by *Equipment and topology*, a legend lists the symbols in the drawing. Zoomed far out, the symbols give way to plain lines in their kind colours.

Closer in, each line carries its conductors across it: one slash per phase, and a slash ending in a small circle for the neutral. Hovering a line gives the same count in words. Closer still, buses are named. Long ids are shortened around an ellipsis, such as `lv_extension_0…c28d5`, and the full id shows on hover and in the overview. Buses and branches that no source reaches through closed branches are drawn faint in every colouring, and the toolbar counts them. Selecting a bus or branch outside the schematic, from an issue, search, a table or the profile, brings it to the middle of the uncovered part of the view, zooming in until its symbols are legible. Clicks in the schematic itself leave the view where it is.

Where branches are drawn along exactly the same segment, as when an MV and an LV line hang between buses at the same positions, the route shows a count badge (×2) once conductor marks are visible. Hovering the route fans its branches a few pixels apart, tapering into the shared ends so each still meets its buses, and a card lists them with kind, voltage level, conductors and loading. Each fanned line and each row can be hovered and selected; clicking the route or its badge keeps it open, and Escape or a click elsewhere closes it. With the cursor on a bus, Tab steps through the branches on shared routes there. *Separate overlapping buses and lines* in the key keeps every shared route fanned and every stack of buses spread, for example for screenshots; the true positions are unchanged. Clicking an issue that is already selected brings it back into view. When the diagram opens, an existing selection is centred if it is off screen, but not zoomed into, so the view still continues from the map.

In *Geographic*, buses the case places at exactly the same point, such as the MV and LV buses of a pole transformer, would hide each other. Such a stack shows a thin ring with a count badge (×2) once symbols are visible. Hovering it spreads its buses on a small circle of fixed screen size, with spokes back to the true point; their lines and symbols move with them. A card lists the buses with voltage level, attached equipment and, after a solve, the worst voltage; each bus and row can be hovered and selected. Clicking the stack keeps it open, and Escape or a click elsewhere closes it. A click on a stacked bus while the stack is closed opens the stack rather than guessing which bus was meant. The keyboard cursor spreads the stack it is on and says that its bus is one of several at that point.

The schematic can be walked from the keyboard. Focus it, then use Left to go to the feeding bus, Right to go onward, and Up and Down to move between buses fed from the same place, in drawing order. Home returns to the source, Enter selects the bus, and Escape stops. A ring and the bus name mark the cursor, the view follows it, and a screen reader hears where the bus sits, what feeds it, what is attached, and its worst voltage against the declared band. Shift with the arrow keys pans.

Once the power flow is solved, the close-up schematic shows values. Each load, generator, inverter-based resource, source and shunt carries its P and Q beside its symbol: consumption for loads and shunts, output for the others. Each branch has an arrow showing which way active power flows. The symbol key in the lower left can be collapsed. Hovering a bus or branch highlights it in the schematic and the results table together.

Each galvanic zone without a source is marked as one network: the buses that are joined by lines and closed switches, separated from the rest only by transformers, which on a feeder are the LV networks. In the schematic the outline follows the drawing column by column, so neighbouring networks stay apart. The outline is orange when a branch inside, or its transformer, is near its rating, and red when a voltage is outside its band or a branch is over its rating. Hover an outline for the transformer feeding it, its size and its state; click it to fit the view to that network. On geographic positions a network is drawn instead as a corridor: a translucent band of fixed screen width along its own lines and around its buses, shown while zoomed out. Neighbouring LV networks often interleave along streets, so outlines around them (such as convex hulls) would overlap even though every bus belongs to exactly one network; a corridor covers only where its network runs. Corridors of networks within limits share one quiet tint; orange and red stay reserved for networks with a problem.

Hovering shows where something sits, the same way on MV and LV. A network's feeders are the branches leaving the bus it is supplied at: on MV, the feeders leaving the source bus; on LV, the circuits leaving the transformer. Hovering a bus or line, here or in the tables, darkens its feeder strongly and the rest of its network faintly, and rings the network's supply point (its source bus, or its feeding transformer). Hovering a transformer shows the network it feeds, and hovering a corridor its whole network. With *Path to source* on, the hover also traces the drawn path back to the source in green, across transformers. Feeders follow the drawing's tree, so through a closed loop a feeder is the drawn one rather than a unique electrical path. Bus, line and load tooltips name their feeder, such as "feeder 2 of TX 1234". Once an outline is wide enough on screen, a tag at its corner names the feeding transformer and gives the network's size, and after a solve its lowest phase–neutral voltage and highest loading.

Lines are weighted and, in *Equipment and topology*, coloured by voltage level, as on a paper single-line diagram: MV and above (from 1 kV) heavier than LV, in shades of blue that darken from MV through subtransmission (from 25 kV) to transmission (from 100 kV), and LV in grey. A network's level is the declared rating of a transformer winding or source in it, else its declared limits, else its solved voltages; the key lists the levels in the drawing, such as "MV · 11 kV". Zoomed out, LV lines fade and LV bus dots are left out, so the MV backbone, the network outlines and their states read first; LV detail returns as you zoom in. In *Voltage within declared band* colouring every bus dot stays.

Sources, generators, inverter-based resources and transformers stay findable when zoomed out. They are drawn as landmarks: their symbol at a fixed size on screen, on a paper halo, handing over to the full symbols as you zoom in. Landmarks are placed by priority so the overview never fills with markers. Sources always show. Next come transformers whose network has a problem (drawn in its colour), then generators and inverter-based resources of at least 1 % of all generation, then other transformers by the size of the network they feed, and last the small inverter-based resources such as rooftop PV, which therefore appear only as there is room. Sources and generators carry their name. Hover a landmark for what it is, click it to select it. The *Landmarks* box in the key turns them off.

In *Geographic*, the schematic draws over the same basemap as the main map (CARTO Positron, without labels), kept in step as you pan and zoom. The basemap fades in as the buses move to their geographic places and fades out, staying where it was, as they move back to the schematic. The *Basemap* box in the toolbar turns it off. A bus the case gives no position (often an LV stub behind a pole transformer) is drawn next to the nearest connected bus that has one; the toolbar counts such buses and their tooltips say so. The main map leaves them out. PNG and SVG exports leave the basemap out. A transformer whose two buses share one location gets its symbol at that point, so it stays visible on the map.

For a selected branch, a multi-wire view draws every conductor between the two buses' terminal rails, so a crossed terminal map shows as crossing wires, with grounded terminals, terminal attachments and each conductor's current. A transformer is drawn as a winding box with wires meeting it from each side: its windings couple magnetically, with no conductor continuity across them.

Once a study is saved, the address bar records it with the selection and view (`?study=…&sel=bus:…&view=schematic`), and *Copy view link* copies that address. The link reopens the study in a browser that holds it, saved there or imported from an export; it does not carry the data itself.

Search finds buses and branches by ID. The bus and branch tables sort, filter to issues, and follow the selection; arrow keys move it. CSV exports keep full precision and begin with the case and solve that produced them. Cases with more than 25,000 buses compute the overview when asked rather than after every load edit, because reading a full result of that size takes a noticeable fraction of a second ([evidence](https://github.com/eigenergy/tellegen/tree/main/validation/evidence/diagram)).

Save a result in Studies to retain its input, solver options, terminal values, and, when the original load model is verified, its canonical PowerIO solution. Import/export moves the saved result between browsers. Reopening checks that its input and results agree without solving again. A geographic attachment updates both saved modules and retains all electrical values. These versioned multiconductor snapshots do not offer the balanced-network planning objectives or sensitivities.

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

`OPTIONS_JSON` also takes two keys that choose what the command prints; neither is a solver setting:

- `"output"` selects the view. `"full"` (the default) prints the complete `McPfResult`, as before. `"summary"` prints the constant-size `McPfSummary` (convergence, iterations, residuals, load voltage range and violation count, source power, passive loss, counts) without building per-terminal records. `"tables"` prints `{"summary": McPfSummary, "tables": McPfResultTables}` (the `mc_pf_tables_view` schema), the full result's rows as columns with each name stored once and complex values interleaved `[re, im]`.
- `"profile": true` adds a `profile` object to whichever view is printed: the `McPfProfile` phases in milliseconds (`parse_ms`, `prepare_ms`, `factor_ms`, `load_evaluation_ms`, `kcl_and_matvec_ms`, `linear_solve_ms`, `summary_ms`, `total_ms`, with `iterations` and `linear_solves`), plus two output phases the CLI times itself: `output_ms`, building the printed view from the solved session, and `serialize_ms`, writing that view as JSON text. `total_ms` covers the solve phases only. Writing the text to stdout and process start-up are outside the profile. The `mc_pf_solve_profile` schema describes this object.

```sh
tellegen solve-mc '{"output":"summary","profile":true}' < case.pio.json
tellegen solve-mc-bmopf '{"output":"tables"}' < case.bmopf.json
```

The views other than the default solve through `McPfSession`, so `solve-mc` then applies the same input checks as a multiconductor Study: the module must hold a `MulticonductorNetwork` or an `McAcPfInstance`. Every view except `profile` is deterministic for a given case and options; the profile's durations vary between runs.

`tellegen describe` includes the options and result schemas, the `McPfSummary`, `McPfResultTables` and `McPfProfile` schemas, and the printed `tables` view (`mc_pf_tables_view`) and `profile` object (`mc_pf_solve_profile`).

The browser's distribution graph carries the case's declared limits when present, in volts: `DistGraphBus.voltage_limits` lists phase-to-neutral (`vpn_min`, `vpn_max`) and phase-to-phase (`vpp_min`, `vpp_max`) bounds, the latter ordered by the pairs (1,2), (2,3), (3,1) of the phase terminals and named in `vpp_pairs`, phase-to-ground bounds keyed by phase terminal (`v_min_by_terminal`, `v_max_by_terminal`, from BMOPF `v_min`/`v_max` or PMD `vm_lb`/`vm_ub`), and the sequence and neutral-to-ground bounds (`vpos_min`, `vpos_max`, `vneg_max`, `vzero_max`, `vn_max`). `DistGraphEdge.rating` gives per-conductor ampacity (`i_max`) or, for a transformer, its largest winding rating in volt-amperes at each bus it connects (`s_rating_by_bus`). The overview ranks voltages against the phase-to-neutral, phase-to-phase and phase-to-ground bands. The `McWorkbench` component in `@tellegen/svelte` renders it from a result snapshot and the graph's inputs. `MulticonductorCase.overviewSnapshot` holds the active case's snapshot of `overviewResult`, built once per operating point and shared with the schematic, and `overviewInputs` and `overviewSearch` hold the network-only inputs, rebuilt only when the graph is replaced. `controller.multiFullResult(case)` reads the full result from a stored solve or the live session.

`mcSchematicLayout(graph, { strategy, dx, dy, attachment_gap })` lays out a distribution graph in the engine (`tellegen::schematic::schematic_layout` natively). It returns columnar bus positions, routed segments, each branch's role (tree route, loop, or open) and the display forest (breadth-first from the sources), which also orders the keyboard walk. `attachment_gap` (22 by default, 0 to disable) is the room reserved below a bus for each kind of attached equipment; `attachment_slots` reports the reserved slots per bus: one per kind, plus one when the bus has perfectly grounded terminals. The layout version is recorded in exported layers. The JSON result carries complex terminal voltages, currents, device powers, source reactions, iteration counts, and KCL residuals.

The browser package provides `solveMcModule(moduleJson, options, signal)` and `solveMcBmopf(text, options)`. The module operation runs in a separate worker so cancelling it leaves other calculations intact. Environments without workers check cancellation after synchronous execution and discard a cancelled result.

For interactive load changes, `createMcPfSession(input, options)` creates a `BrowserMcPfSession` in the shared engine worker. `input` is PowerIO IR text, or a `BrowserMcInput`: the module the engine kept when the case was ingested with `{ retain: true }`. A kept module never crosses the worker boundary as text, which matters for a whole-city model whose pretty-printed IR exceeds the 128 MiB input limit. `BrowserMcInput.moduleJson()` writes the text on request and `free()` releases the module. `initialSummary` describes its first solve. Its `replaceLoadPowers(edits)` method takes the complete absolute override set, warm-starts from the previous solution, reuses the prepared LU, and returns an `McPfSummary`: convergence, residuals, load voltage range and violation count, source power, passive loss, and counts. `lastTiming` records the edit's engine, transfer, and parse time, and `profile()` the engine's own phases. Detail is fetched on demand: `detail({ bus, element, port_offset, port_limit })` returns one bus's terminals and a page of equipment ports, `terminalIds()` with `terminalVoltages()` and `terminalCurrents()` return interleaved `Float64Array`s, and `result()` builds the complete `McPfResult`; `resultTables()` returns the same rows as columns (`McPfResultTables`: names interned once, values in transferable typed arrays), which whole-network views read, and `mcPfResultTables(result)` gives a stored result the same form. `loadBranches()`, `inputModule()`, `input()`, and `snapshot()` expose the editable identities and materialize the current solved state (`input()` keeps it in the engine as a `BrowserMcInput`); `materializationCount()` confirms that ordinary edits build no edited network. `summarizeMcPfResult` and `mcPfResultDetail` give a stored result the same shapes. Native callers use `McPfSession` (`from_module`, `summary`, `detail`, `terminal_voltages`, `build_result`, `edited_instance`, `input_module`) and `McLoadPowerEdit`. A session must be rebuilt after structural changes such as topology, taps, source voltages, load connection maps, voltage-model parameters, or solver options.

`solveMcStudy`, `replayMcStudy`, and `applyMcStudyGeo` create, reopen, and update saved multiconductor results. Native callers use `McStudySnapshot` for the same operations.

An agent can use `inspect_case`, `solve_multiconductor_pf`, and `query_network`. A solve requires the case ID and displayed revision. Querying `voltage_v` ranks buses by their largest terminal-to-ground voltage; `terminal_values` lists the individual terminals. `price` remains unavailable for AC power flow.

## Evidence

The test suite includes analytic resistive feeders and comparisons against OpenDSS reference results for grounded, floating, and impedance-grounded neutrals, three-phase lines, delta loads, multiple winding connections, and a loaded split-phase centre-tap transformer. The centre-tap oracle checks voltage plus aggregated physical-terminal current orientation and complex power. The published tolerances distinguish voltage, current, and complex-power comparisons. These fixtures establish the supported calculations; they do not establish universal convergence or multiconductor AC OPF support.

Further tests exercise the real WASM worker, unsupported-data rejection through IR reload, geographic points and multi-point routes after attachment, and unchanged electrical results after geographic edits. Session tests check that edits reuse the factorization and build no edited network, that the summary equals the complete result it summarizes, and that portable output is byte-identical to the previous implementation. A generated ~10,000-bus regression runs in CI ([performance](benchmark-results.md#mc-pf-performance)).


<a id="multiconductor-reference"></a>

<a id="multiconductor-reference-multiconductor-fixed-point-power-flow"></a>

## Multiconductor fixed-point power flow

In the web app, open **Studies** and choose **Load 4-conductor example**, or
upload a supported BMOPF JSON distribution case. The Studies panel changes to
**Distribution power flow** for multiconductor inputs. Choose **Run power
flow**, then select a bus or branch on the map to inspect its results.

Bus tables report each terminal's voltage magnitude to earth in volts and
angle in degrees, including an explicit neutral. When a bus has a neutral,
additional columns show the magnitude and angle of the complex difference
`V_terminal - V_neutral`. The neutral-to-neutral voltage is zero and its angle
is undefined. Branch tables report conductor
currents in amperes and terminal active/reactive powers. The summary reports
source power in kW/kvar and passive active losses in kW. These are simulation
results; the distribution Study does not expose balanced-network optimization
goals or sensitivities.

Solver availability follows the parsed input type and supported electrical
semantics. Balanced transmission cases retain their existing Study workflow.
Unsupported distribution cases can still be inspected on the map, with a
reason explaining why the power-flow action is unavailable. Raw OpenDSS
imports remain view-only until their source and load semantics can be retained
without approximation; use a supported normalized BMOPF input for this solver.

Saved distribution results use a separate, versioned simulation snapshot in
the browser's IndexedDB. A snapshot contains the typed input, solver options,
detailed terminal results, and load-model qualification. A canonical portable
PowerIO solution is included only when the original load model is verified.
Reopening validates the saved input/result association without running a new factorization.
Use **Save result** to retain it locally, **Export** to download the snapshot,
and **Import** or the **Saved study** selector to reopen it. Browser storage is
local to the app's origin; export a snapshot to move it to another browser.

The corresponding engine APIs are `solveMcStudy` and `replayMcStudy`;
Rust exposes `solve_mc_study_json` and `replay_mc_study_json`. These simulation
snapshots are distinct from the balanced-network `StudyDocument`.

After the first solve, selecting a bus exposes its load-branch P and Q values.
Changing either value automatically warm-solves the same fixed-point
current-injection problem. Rapid changes are coalesced, and **Reset** removes
that branch's override. The edited powers and matching result are included in
subsequent saved snapshots. Large portable input/solution modules are
materialized only when saving, exporting, attaching geography, or explicitly
rebuilding the calculation. An ordinary edit sends the edit set to the worker
and receives a constant-size summary in one round trip; the Studies panel
fetches the selected bus's terminals and one page of equipment results on
demand.

The interactive API is a retained `McPfSession` in Rust and
`createMcPfSession(input, options)` in TypeScript, where `input` is PowerIO IR
text or a `BrowserMcInput` the engine kept at import. A session from a kept
input shares its network tables, so no IR text crosses the worker boundary in
either direction. A session keeps the
prepared passive network, compensation matrix, sparse LU factorization, and
last converged voltage. `replace_load_powers`/`replaceLoadPowers` accepts the
complete set of absolute load-branch overrides relative to the session's base
input; an empty set restores the base powers. Failed validation or convergence
leaves the previous solved operating point intact.

Edits are an overlay on the base input. They replace the prepared load laws
(physical nominal admittance follows the edit; the compensation reference and
factor stay frozen) and never copy or rebuild the network. Each solve produces
an `McPfSummary`, computed from the converged phasors with the same output
finiteness checks as the complete `McPfResult`, which is built only on request
(`build_result` in Rust, `result()` in TypeScript), or as columns with each name
stored once (`result_tables` and `resultTables()`, which whole-network views
read). `edited_instance`,
`input_module_json`, and `snapshot` build the edited network on demand, and
`materialization_count` counts how often. Detail views use `detail` (one bus
and a page of equipment ports) and the interleaved terminal voltage and
current arrays. The session retains the stored module's records rather than
its JSON text, so portable output needs no re-parse and is byte-identical to
re-parsing. `profile` and `cold_profile` time load evaluation, KCL and
matrix-vector work, retained LU solves, and the summary, plus parse, prepare,
and factor at creation; WASM builds time them with `performance.now`. Topology, taps, source
voltages, load connection maps, load voltage-model parameters, and solver
options are structural session data: changing them requires a new session.

The multiconductor PF entry point is `tellegen::solve_bmopf_json`. It accepts
raw BMOPF JSON, validates fields that PowerIO 0.11.0 would otherwise collapse,
then parses the document through PowerIO and solves the resulting typed
`McAcPfInstance`. The native runner has the same path:

```text
cargo run -p tellegen --example mc_pf --features mc-pf -- case.json
```

The retained-session example scales every load branch and reports cold/warm
timings and phase profiles, iteration counts, and the factorization and
materialization counts:

```text
cargo run --profile release-py -p tellegen --example mc_pf_session --features mc-pf -- case.json 1.001
```

The generated feeder benchmark measures latency, payload, and memory at
32,412 to 676,530 terminals, natively and in the browser, and CI runs its
~10,000-bus preset as a deterministic regression; see
`docs/src/mc-pf-performance.md` and `crates/benchmarks/README.md`.

The browser build exposes the same operation as `solve_mc_bmopf(text,
options_json)`. The TypeScript transport is `solveMcBmopf(text, options)`.
For a serialized PowerIO MC module, use `solve_mc_module_json` in Rust,
`solve_mc_module` in WASM, or `solveMcModule` in TypeScript. Typed Rust callers
can pass an `McAcPfInstance` directly to `solve_mc_ac_pf_instance`.

Options are JSON fields `tolerance`, `max_iterations`, `damping`,
`zero_voltage_tolerance`, `absolute_kcl_tolerance`,
`relative_kcl_tolerance`, `voltage_envelope`, `v_low_pu`, `v_min_pu`, and
`v_max_pu`; defaults are `1e-4 V`, `100`, `1.0`, `1e-9 V`, `1e-4 A`, `1e-5`,
`true`, `0.5`, `0.85`, and `1.15`. Convergence requires both the
voltage-change tolerance and the physical KCL test: the residual must be below
`absolute_kcl_tolerance + relative_kcl_tolerance * incident_current_scale`.
Voltages are volts, currents are amperes, and terminal powers are VA. Complex
values cross JSON as `{ "re": ..., "im": ... }`.

The solver supports voltage-dependent loads, ideal voltage sources,
lines, passive shunts, and finite-leakage two-winding transformers, including
phase-to-phase `single_phase` records, two-winding `n_winding` records with
explicit BMOPF delta rolls, and fixed-tap `single_phase_autotransformer`
regulator snapshots. It also supports the three coupled windings into which
PowerIO lowers a BMOPF `center_tap` transformer, including anti-series
split-phase polarity, fixed taps, excitation, and neutral impedance.
Transformer admittance follows the
OpenDSS terminal primitive: common leakage base, winding voltage/tap maps,
actual WYE/DELTA coil incidence, explicit neutral impedance, and explicit
per-coil excitation shunts. Zero leakage (an ideal voltage constraint),
arbitrary transformers with more than two windings, ideal zero-impedance
regulators, finite source impedance, and unsupported controls
are rejected with an error rather than approximated.
The direct OpenDSS YPrim comparison for YY/DD/YD/DY fixed-tap and per-winding
delta-roll cases is frozen in
[`validation/evidence/mc-pf-yprim`](https://github.com/eigenergy/tellegen/tree/main/validation/evidence/mc-pf-yprim). Transformer unit tests add
a fixed-tap, excited centre-tap YPrim comparison, while the loaded centre-tap
oracle also checks terminal voltages, currents, powers, losses, and polarity.

Raw BMOPF inputs must use scalar or uniform taps. An `n_winding` raw record
cannot carry a tap because PowerIO 0.11.0 drops that field; a canonical typed
two-winding record can carry its fixed winding taps.
Non-constant-power loads must carry explicit nominal branch voltages.
Constant-power loads may omit them: the solver assigns each line-connected
voltage zone a line-to-neutral base from transformer winding ratings or other
explicit nameplates, falling back to its ideal source voltage when no rated
anchor exists. It converts that base to the load's WYE, DELTA, or single-phase
branch voltage. Missing or conflicting anchors are rejected instead of guessed.
Nonzero legacy `g_no_load`/`b_no_load` values on isolating transformers require
normalization to an explicit `no_load_shunt`; this avoids choosing between the
tagged schema and BMOPFTools legacy reference sides. Exact-zero legacy fields
are accepted as no-ops, and `single_phase_autotransformer` defines nonzero
legacy excitation across its from winding unambiguously. An OPF instance can
be projected to PF through PowerIO's typed conversion; its objective and active
constraints are discarded by that conversion and PowerIO emits the standard
projection diagnostic; the convenience raw solve proceeds with the projected
PF instance.

A declared solid transformer neutral (`r=x=0`) is returned as an exact fixed
zero-voltage terminal constraint; it is not replaced by a large admittance. A
negative canonical `r_neutral` means floating. Grounded bus terminals remain
the same exact constraint representation.

Within one prepared solve, the sparse factor is built once and reused for
every fixed-point iteration while topology, taps, the frozen compensation
admittance, and fixed terminals are unchanged. A retained session also reuses
that factor after load P/Q changes and starts from the previous converged
voltage. A solve with no unknown terminals has no factorization to perform.

<a id="multiconductor-reference-load-voltage-models"></a>

### Load voltage models

Each load branch uses its complex terminal-to-terminal voltage `U`, including
neutral displacement for an explicit WYE neutral and line-to-line voltage for
a DELTA branch. Let `r = abs(U) / v_nom`, with nominal powers `P0` and `Q0`.

| BMOPF model | Active power | Reactive power |
| --- | --- | --- |
| `constant_power` | `P0` | `Q0` |
| `constant_current` | `P0 * r` | `Q0 * r` |
| `constant_impedance` | `P0 * r^2` | `Q0 * r^2` |
| `zip` | `P0 * (alpha_z*r^2 + alpha_i*r + alpha_p)` | `Q0 * (beta_z*r^2 + beta_i*r + beta_p)` |
| `exponential` | `P0 * r^gamma_p` | `Q0 * r^gamma_q` |

For nonzero branch voltage, consumed current is `conj((P + j*Q) / U)`.
Constant-current loads therefore track the branch-voltage angle and retain
their power factor; they are not fixed complex current phasors. ZIP coefficients
are used as supplied, without silently normalizing their sums. Active and
reactive coefficients or exponents may differ.
With the default fallback outside the voltage band, all non-impedance load models use nominal
impedance below `v_low_pu`, linearly interpolate complex current between the
impedance and constant-power values from `v_low_pu` to `v_min_pu`, use the
declared model from `v_min_pu` through `v_max_pu`, and use the impedance fixed
at `v_max_pu` above it. This is the convergence fallback used by OpenDSS, with
the normal band widened here to the solver-wide 0.85–1.15 pu defaults. Constant
impedance is unchanged. Disabling `voltage_envelope` restores the raw equations;
at exactly zero voltage, raw laws without a continuous limit return a controlled
error. Non-finite currents or absorbed powers are always rejected.

The piecewise regions follow the [OpenDSS convergence-model
description](https://opendss.epri.com/LoadModelsThatNearlyAlwaysConver.html)
and its [`Load.pas`
implementation](https://github.com/tshort/OpenDSS/blob/master/Source/PCElements/Load.pas).
Explicit nominal voltages, ZIP coefficients and exponential exponents may be
supplied once for all branches or separately per branch. Unknown model names,
incomplete parameter sets and conflicting coefficient families are rejected;
they are not interpreted through PowerIO's permissive model fallback.

The compensation admittance stamped into the linear operator remains fixed.
Each iteration compensates its current against the selected load law, so these
models retain the single-factorization algorithm. A retained session updates
the physical nominal admittance used by impedance and fallback load laws
when P/Q changes, while leaving only the algebraic compensation reference
frozen. Reported load currents and powers use the operating voltage, rather
than merely copying nominal powers.

Convergence and voltage validity are reported separately. `voltage_valid` is
true only when every load branch lies within the configured `v_min_pu` and
`v_max_pu` band. This is a load-model band, not an engineering voltage limit
or a test of whether the load equations were modified. `min_voltage_pu`, `max_voltage_pu`, and
`voltage_violations` identify the observed range and each offending load branch.

These are the BMOPF voltage laws plus the OpenDSS low/high-voltage fallback,
not the complete OpenDSS load engine. ZIPV cutoff, time-dependent load
multipliers, and control behavior are not added by this extension. Equation-only
OpenDSS comparisons disable the fallback or remain within the matching range.
The frozen load references use OpenDSS model 2 for impedance, model 5 for
current, model 8 for ZIP with cutoff disabled, and model 4 with specified CVR
exponents for exponential loads. This comparison does not add support for
OpenDSS model-number input or its time-series/load-status semantics.


<a id="multiconductor-reference-settings-and-model-qualification"></a>

### Settings and model qualification

The [user guide](multiconductor-pf.md#solver-settings-and-opendss-comparison) lists every exposed default, explains the new tolerance values, and compares their units and acceptance criteria with OpenDSS.


**Power flow settings** in Studies exposes the load-model fallback and its
`v_low_pu`, `v_min_pu`, and `v_max_pu` thresholds separately from numerical
settings (maximum iterations, damping, and advanced voltage/KCL tolerances).
The form shares one set of next-run settings with the context-sensitive solve
button in both Network and Studies.
Edits and **Restore defaults** take effect on the next explicit run; editing
settings alone does not start a calculation. Invalid settings disable the run
actions. Each rerun creates a new retained session, preserving current load edits.
Reopened snapshots restore their recorded settings. Failed runs preserve the
previous result, whose assessment records the settings actually used for it.

Every new `McPfResult` and `McPfSummary` carries `load_model`:

- `status`: `original`, `modified`, or `unverified`.
- `modified_branch_count`: branches whose final currents differ from the
  original load law, allowing only floating-point roundoff (1e-12 times one
  ampere plus the larger current magnitude).
- `original_kcl_residual` (A) and `original_scaled_kcl_residual`: an independent
  check using the original load equations. A scaled residual at most one
  passes the configured KCL tolerances; both fields are null if the original
  equations are undefined or non-finite at the returned voltage.
- `options`: all effective solver settings.

The full result also includes `load_model_changes`: load/bus/branch identities,
branch voltage in pu, actual absorbed power, and original-law power at the same
voltage (W + j var, or null if undefined). A modified result remains inspectable
and saveable. Its warning persists in Studies, the network inspector, diagram
source information and CSV exports. Numerical convergence alone never establishes an
original-model solution. Legacy results without an assessment are unverified.

Constant-impedance and zero-power branches are not marked modified merely
because they fall outside the voltage band. Fallback use during intermediate
iterations does not qualify the final point. Disabling the fallback can yield
an original-model solution outside the band. A change to the thresholds changes
the equations; it does not waive engineering operating limits.

Simulation snapshots now use version 2. For modified or unverified results,
`solution_module` is null: the snapshot retains its input, options, assessment
and full numerical result without claiming a canonical `McAcPfSolution`.
For verified original-model results, the solution module also records the
assessment in the `tellegen.mc_pf` extension. Direct `to_powerio_solution`
conversion rejects modified or unverified results.

Version 1 snapshots remain readable. Replay validates their contents,
reassesses the original load laws at the saved voltages without re-solving,
and upgrades them to version 2; a misleading canonical solution is removed
when the saved point used modified equations. Version 2 replay verifies the
saved qualification against the input, settings and terminal voltages. Terminal, source and element currents and powers are independently recomputed from the effective network equations; the result is not used as its own reference.
Unconverged iterates are still not returned as solutions.

<a id="multiconductor-reference-explicit-energized-network-selection"></a>

#### Explicit energized-network selection

Import detects bus islands without a voltage source using lines, closed switches
and all transformer windings to establish connectivity. Network and Studies each
show one solve button: **Run power flow** when every island has source coverage,
or **Solve energized network** when unpowered islands are present. Counts and an
expandable list of affected buses and equipment are visible before solving. A
network with no source remains inspectable, with solving disabled and an explanation.

Detection is read-only. Clicking **Solve energized network** explicitly selects
source-covered islands, revalidates the exclusions against the current input, and
records the selection. Importing a replacement case runs detection again. Load and
coordinate edits do not change source coverage. This bus-level check does not
establish conductor-level solvability, change solver settings, or reconnect open
switches. The low-level full-network solver still rejects unpowered islands unless
an explicit selection is recorded.

The original full network remains in the map, diagram and saved input module.
The input's `tellegen.energized_network` extension lists each excluded island's
buses and equipment. Open boundary switches are listed with the excluded island;
a switch joining two excluded islands appears in both lists. Aggregate counts
deduplicate equipment by kind and name. Excluded loads are explicitly unsupplied
and absent from solved totals. Missing results are not synthesized as zero.

Full results carry `network_scope` and `excluded_islands`; compact session summaries
carry only scope counts. Saved Study JSON retains the full input and all exclusions.
A canonical PowerIO solution, when available, covers the selected network and
carries the exclusion record in its module extension. CSV source information carries both
network scope and load-model qualification. Replay checks exclusions against the
full input topology and checks the result against the selected instance. Load
edits update only the solved network's loads in the retained full input.

This selection is independent of load-model qualification: an energized subset
may solve either the original load equations or modified equations. The scope
and qualification are displayed separately. Importing a prepared input or
reopening a saved Study retains the explicit selection for subsequent runs.

Saved studies compact formatting whitespace in their embedded PowerIO modules.
All values, numeric spellings and string contents are retained, while large
full-network studies fit within the existing browser input limit more readily.
