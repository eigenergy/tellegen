---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Multiconductor graphs carry declared voltage limits and ratings, and the Studies
view gains an operating-point overview.

In `@tellegen/engine`, `DistGraphBus.voltage_limits` holds the model's declared
phase–neutral and phase–phase voltage bounds in volts, and `DistGraphEdge.rating`
holds per-conductor ampacity (`i_max`) or, for a transformer, its largest winding
rating at each bus it connects (`s_rating_by_bus`), the same on every winding-pair
edge. Both are present only when the model declares them.

In `@tellegen/svelte`, `McWorkbench` summarises a multiconductor power-flow result:
violations, losses, voltage ranges per declared reference, maximum loading and
maximum unbalance; a ranked issue list that selects the bus or branch; search
over buses and branches; per-phase voltage bars against declared limits and
per-conductor currents at both ends; sortable bus and branch tables; and CSV export. Cases
larger than 25,000 buses compute the overview on request. Setting
`MulticonductorCase.schematic` makes `TellegenMap` show the case as a deck.gl
schematic or at its source positions, with an animated switch between them,
result colouring, the shared selection and PNG/SVG export. `overviewResult` holds
the operating point the overview and schematic share. The schematic layout runs in
the engine: `mcSchematicLayout` in `@tellegen/engine` returns tidy or leaf-order
geometry and the display forest, reserving room below each bus for its attached
equipment. The schematic draws transformer, switch (open or closed) and attachment
symbols (load, generator, inverter-based resource, source, shunt) with a legend.
Lines show their conductors at close zoom; buses are named with short ids; parts not
energised from any source are drawn faint; and outside selections fly into view.
The schematic can be walked bus by bus from the keyboard, with a spoken description.
Solved P/Q and power-flow arrows show at close zoom, hover is linked across the
schematic and tables, and each LV network (galvanic zone) is outlined (on geographic
positions, as a corridor along its lines) and tinted by its worst state, with a summary tag. MV lines draw heavier than LV, and LV
detail fades when zoomed out. Sources, generators, large IBRs and transformers stay
visible as fixed-size landmarks, placed by priority. Geographic positions draw over the main
map's basemap, which fades in and out with the layout switch. The diagram opens on the case's own
positions and switches to the schematic from there. Branches drawn along the same route fan
apart on hover, with a card to pick between them, and buses at the same point spread
apart the same way. The schematic can be downloaded as a PowerIO
diagram layer, follows a case's own routes in source positions, and a selected
branch gets a multi-wire view of its conductors. Unbalance is the voltage
unbalance factor from terminal phasors, shown beside the magnitude spread.
`controller.multiFullResult`
reads the full operating point from a stored result or the live session, and
`TellegenShell` keeps `overviewResult` current for the active case
(`controller.followOverview`, `readOverview` and `overviewOnRequest`). A
multi-winding transformer counts once in losses and issues, and its loading is
judged per connected bus. Graph buses carry `nominal_voltage`, the declared rating
of a winding or source there; the diagram colours lines by voltage level, and a
hover shows the feeder and network of what it is over, with an optional path back
to the source. Graph buses also carry `attachments` (each attachment's terminals in
its own order and its connection) and transformer edges `windings` (bus, wye or
delta, terminals, rating). The diagram draws each transformer once with its winding
connections or centre tap, and each load or shunt with its conductors and, closest
in, its terminals. The
renderer-independent logic lives in `src/lib/diagram`, ported from the diagram
projection draft (#177).
