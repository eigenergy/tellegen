# Transmission diagrams

Balanced (transmission) cases open as a single-line diagram. **Single-line diagram** beside the
map's zoom buttons (**Diagram** on a phone), or beside **Fit diagram** for a case drawn in diagram
coordinates, or **Show diagram** in the network panel, replaces the map with a canvas; **Show map**
or **Close** returns to the map at the view the diagram showed. The diagram opens at the map's view
when that shows the case at a useful size, and otherwise fits the case into the space the panels
leave free. While the diagram is open, the network panel's LMP, angle and demand buttons say they
apply to the map and to the diagram's **Map display** view; the diagram's own key explains the
other views.

## Geographic

The diagram first draws the case on its own positions: geographic coordinates over the map's
basemap, or drawing coordinates as given. Zoomed out, the largest generators and the transformers
stay findable as fixed-size landmarks, placed by priority so the view never fills with markers.
Closer in, transformers are drawn on their branch as two interlocking circles, each winding in the
colour of the voltage on its side, generators and loads hang below their bus, and buses are named.
Out-of-service branches are dashed. The colour of a conductor or busbar fills the whole stroke,
with a thin casing a shade darker so pale colours keep an edge against paper and the basemap.
Colour carries the data, so it takes the stroke's area: small marks need larger colour
differences to be told apart. Busbars are heavier than branch conductors.

Loads use the same demand-arrow shape as the multiconductor schematic, with a filled triangle in
a quiet grey: nearly every bus has load, so loads are context rather than the result. They
appear with the full equipment detail. Generators use the shared machine-circle glyph in symbol
ink and appear a step earlier. Both can be clicked or tapped to select their bus, and both fade
with buses outside a highlighted voltage level. A bus with generation and
load draws the symbols side by side, with visible connections back to the bus.

## Voltage levels

The key lists the case's own nominal voltages, highest first: "500 kV", "230 kV", "161 kV". It
holds at most six entries. A case with more levels merges adjacent levels into ranges, cheapest
merge first, where a merge costs the voltage ratio it spans weighted towards higher levels; so
closely spaced low levels merge first ("115–138 kV", generator terminals as "13.2–13.8 kV") and
the highest levels stay distinct longest.

On a map any two lines can touch, so colour alone reliably separates only a few classes. Each level
is therefore drawn with three cues together:

- **hue by tier**: magenta from 300 kV, blue from 100 kV, cyan from 35 kV, and neutral grey below.
  The three hues stay distinguishable from each other, and from the warning, violation and
  selection colours, under the common colour-vision deficiencies. When two of a case's levels fall
  in one tier and a lower hue is unused, the lower level takes that hue, so the main levels differ
  in hue: ACTIVSg200's 230 and 115 kV draw blue and cyan, 500/230/115 kV magenta, blue and cyan.
  Hues only move down, and never back up as voltage falls; 345/138/115/69 kV keeps 115 kV a
  lighter blue because 69 kV needs cyan;
- **shade within a tier**, darker for the higher level;
- **width by rank**, heaviest for the highest level.

The key groups the levels under their tier's name, whatever hue they draw in. Hovering a level in the key keeps it and fades the
others; clicking keeps it highlighted. Zoomed
out, levels below the top three fade so the backbone reads first.

## Results

On desktop the toolbar shows **Voltage levels**, **Operating limits**, **OPF economics**,
**Power transfers** and **Map display** as tabs; arrow keys move between them. On phones,
the labelled **View** selector lists all five views without a clipped horizontal tab strip.
The layout selector, fit and zoom controls remain beside **Display**, which reveals basemap,
shading and PNG export options. These settings start closed to leave room for the network.
A view that needs a result explains what is missing instead of being disabled.

Each result view starts with what the available data supports. Operating summaries report how
many lines and buses were checked, and distinguish missing flows from unknown rating metadata.
Missing voltages or limits prevent an all-clear summary. Out-of-service and diagnosed de-energized
equipment are excluded from checks. A checked voltage is compared with its available source bounds;
this is not a guarantee about limits absent from the case.

Economics reports the range of supplied prices without inferring thermal binding status from
price differences or uniformity. If constraint metadata is absent, its status is explicitly
unavailable. Transfers distinguish absent results, a previous calculation's retained result,
missing flows and confirmed zero active power. A largest transfer is qualified as being among
reported flows when other branch flows are missing.

**Details** expands the summary to show settings, lists and explanations; **Summary** collapses
it again. **Key and settings**, **Loading, binding and marginal relief explained**, and **MW,
MVAr and MVA explained** can be opened by touch or keyboard. Hover definitions remain available
as shortcuts. A documentation link accompanies the expanded explanation.

Prices are in objective units per MW: the units of the case's generator costs per hour. MATPOWER
files give costs in $/h, so for them this is $/MWh; payloads do not carry a currency, so the
diagram does not claim one. The diagram opens on **Operating limits**. AC power flow and AC OPF show
solved voltage magnitudes and branch utilization together, independent of the map's LMP or
demand setting. DC shows utilization and explicitly states that voltage magnitudes are not
solved. SOCWR is labelled as a relaxation.

Voltage uses a fixed 0.90–1.10 pu scale and each bus's source bounds. Downward blue and upward
red markers identify lower and upper violations, including uniform violating series.
**Shading** (on by default) shades the area around the network by where each bus voltage sits in
its own band: clear in the middle half, blue towards the minimum and brown towards the maximum,
at full strength at a limit. Shading interpolates bus values in drawing space; it is not a
measurement at every point on the map. Low- or high-voltage regions show by area before any number is read
(Overbye et al., IEEE TPWRS 2003, found contours gave the fastest violation detection). Substation
borders preserve the worst member violation. A station summary retains minimum and maximum
values with bus identities, and links to the worst bus; it never substitutes an average.

Branches below the warning threshold are thin and grey. Branches from the warning up to and
including the rating are wide and amber, and overloaded branches wider still and red. Stressed
branches are drawn on top of the others, so they stay visible at the case overview. **Key and settings** exposes the warning threshold (0–99%, default
90%), remembered in browser storage. Local gauges show 0–125% with a visible 100% tick; their
labels are not clamped to the gauge range. A leader joins each gauge to the middle of its branch.
A heavy amber outline marks operation at the rating and a filled red gauge an overload, so the
state does not rest on hue alone. Violations, warnings and selection receive priority when the
view is crowded; a gauge moves around its branch to find space before it is left out. Gauges
can be selected by mouse, touch or keyboard. Close in, static
chevrons show active-power direction.

DC utilization is absolute MW over the effective MW rating; AC utilization is the larger end's
apparent MVA over its MVA rating. Unknown ratings and synthesized DC bounds are faded and omit
equipment-percentage gauges. Out-of-service lines stay dashed, while diagnosed de-energized
buses are excluded from voltage extrema and violations. Display comparisons allow `1e-5` pu
voltage and `1e-5` fractional loading tolerance; these do not change solver feasibility.
At-rating operation does not establish economic binding or enforcement.

Open **Exceptions** to search voltage violations and loading exceptions by equipment ID. Voltage
violations are ordered by distance beyond the case bound. Loading exceptions are ordered by
overload, at rating and warning, then by utilization. Separate counts show how much voltage and
loading data is unknown.

Hovering or focusing a row in a result list highlights its equipment on the diagram. Hover
equipment for a preview. Select an exception or an item on the diagram to keep the equipment details
visible. **Frame** brings the equipment into view. **Clear** or Escape clears the selection.
The inspector shows the bus voltage and source bounds, solved generation, modeled demand and
installed capacity. For branches it shows each available endpoint MW and MVAr value, the basis and
effective rating, the limiting end and remaining capacity. When one end supplies the other, a
sentence states the flow in words ("555 MW flows from bus 189 to bus 187"). Positive endpoint flow
enters the branch from that bus. Values are rounded for reading: per-unit voltage to four
decimals, percentages to one, and MW, MVA, MVAr and prices to about three significant figures.
Hover a value for its exact figure. Missing quantities are marked unavailable.

The key retains formulation, solver status and freshness. Calculations in progress and errors
identify retained results as potentially stale; legacy payloads without status say it is
unavailable. These are base-case displays and do not establish contingency security.

**Voltage levels** preserves nominal kV identification in its own colour view. Nominal kV
labels and tooltips remain available in operating mode, where transformer strokes stay neutral
so nominal-voltage colours do not compete with result colours. **Map display** retains the map's
selected series; uniform economic or demand series use neutral grey. PNG export includes the
operating gauges, result context and station range.

### OPF economics

Choose **OPF economics** to see total locational marginal prices in objective units per MW. The
default price scale is 0–50, with wider and signed presets. The scale stays fixed as results change,
and values outside it use the end colours. A uniform price has a single numerical readout.
When prices differ, **Shading** shades the area around the network by each bus's distance from
the median LMP: blue where power is cheaper and brown where it is dearer. The full-strength
distance is the largest in the result, but at least 5% of the median or one price unit, so
small spreads stay faint. The shading shows relative price differences, not their cause, and interpolates between buses.
It uses a different reference from the bus colours, which keep the fixed absolute scale.

A wide purple branch identifies a binding DC thermal limit with positive marginal relief; other
branches stay thin and grey. The
binding list orders limits by the objective reduction from one more MW of symmetric rating.
Select a branch to frame it and see the thermal status and exact equipment values. The final solver
program supplies enforcement and binding independently of equipment utilization. If it used a
synthesized solver bound, the inspector labels it.

Solved interface and other linear constraints list their bounds, value and signed shadow price,
with links to branch members where available. The sign describes the objective reduction for an
increase in the signed limit. AC OPF and SOCWR can show total LMP where supplied. SOCWR stays
labelled as a relaxation, and the view marks unsupported or legacy constraint data as unavailable.

#### Loading is not marginal value

These quantities answer different questions:

| Readout | What it establishes | What it does not establish |
| --- | --- | --- |
| 100% utilization | The reported flow reaches the effective rating, within display tolerance. | Whether the solver enforced that rating. |
| Binding constraint | An enforced limit is reached, within reporting tolerance. | Whether relaxing it would reduce optimal cost. |
| Positive marginal rating relief | Increasing the rating has a positive local rate of cost reduction. | That the actual flow will rise by the same amount, or that a large upgrade has this constant value. |

For example, an enforced line limit can sit at 100% with zero marginal relief. Another line at 100% can
have relief of 20 objective units/MW: a small increase in its rating then has an estimated
cost benefit at that local rate. The second line is purple; both can appear in the binding list.
Changing the display warning threshold changes neither the optimization nor these values.
Equal bus prices alone do not identify a unique marginal generator. Different prices can
reflect constraints, separate islands, and, for AC, losses; interpret them with the reported
constraint data rather than diagnosing a cause from colour alone.

Nominal **kV** identifies the voltage level. **pu** compares a solved voltage with that nominal
value: 1.00 pu is nominal and 0.95 pu is 5% below it. **MW** is active power; **MVAr** is reactive
power. Both contribute to **MVA**, the apparent power used for the AC equipment-loading check.

### Power transfers

Choose **Power transfers** to see active power moving through the network. Blue line width
shows the larger reported endpoint |P| in MW on a bounded square-root scale. The default cap
is 500 MW, with 100 MW and 1,500 MW presets. Static
chevrons follow the local direction of the route when one end supplies the other. Grey distinguishes
a reported zero from faded missing flow. Red gauges and the **Overloads** list retain the independent
rating check in percent.

Use **Largest active transfers** or select equipment on the diagram to inspect signed endpoint
values. Positive flow enters the branch from the named bus. The lossless DC view gives equal and
opposite endpoint MW. AC and SOCWR show both reported active and reactive values, and omit a
through-direction arrow when the two ends supply the branch. Bus details separate solved
generation where supplied, model demand including committed edits, and installed capacity.

**Summary** and **Details** collapse the results panel to its summary (the view, its counts and
scale) and expand it again. The price-scale selector is hidden in the collapsed economics panel. It starts collapsed on a phone, where it would cover much of the
diagram, and opens when equipment is selected.

On phones, the toolbar and voltage-key controls have 44-pixel touch targets, and the renderer
allows a wider picking radius. Explicit zoom buttons supplement pinch zoom and the keyboard.
The results key stays below the toolbar when it wraps. The equipment and voltage keys can be
collapsed, and scroll within the available height. Tapping a symbol selects its bus and opens the
same details used on desktop; information is also available through search and the panels.

## Schematic

**Schematic** switches to a substation overview laid out in the engine. Each substation is a box
with one busbar per bus, the highest voltage on the left, and the transformers between its busbars.
Every connection has its own row in the box, so lines leave from the side facing the substation at
their other end without overlapping. Substations sit at, or as near as space allows to, their
geographic position, and substations in dense areas are drawn smaller, so the overview stays
registered to the basemap: switching between the two layouts moves each bus from its geographic
point into its substation. Lines run through the gaps between substations, spaced apart, and step
around any substation in their way. Each substation is named once, at the corner of its box; closer in, its
busbars carry their voltage.

A case without coordinates is placed on the map by a synthetic layout that says nothing about the
grid. Its schematic lays the substations out by their connections instead: sparse stress
(Ortmann, Klimenta and Brandes 2017) on the graph of substations, so substations a few lines
apart are drawn near each other, fitted into the same frame as the synthetic positions. ACTIVSg2000
from its MATPOWER file lays out in about 30 ms, ACTIVSg70k in a little over a second.

The method follows Birchfield and Overbye (2018): independent substation interiors, greedy
placement at the free point nearest each substation's position, and line routing through the
channels of a Delaunay triangulation of the substations. The
[layout evidence](https://github.com/eigenergy/tellegen/blob/main/docs/evidence/diagram/README.md)
records its quality and speed: ACTIVSg200 lays out in under a millisecond with no overlapping
substations and no line crossing another substation; 70,000 buses take about a third of a second.

## Substations

The engine groups buses into substations from the strongest evidence a case offers:

1. substations declared by the source: PowerIO voltage levels (CGMES, XIIDM, RAWX, PSS/E) or a
   PowerWorld `SubNum`;
2. buses at exactly the same coordinate (ACTIVSg buses carry their substation's coordinate);
3. the two ends of a transformer, unless both are located more than 2 km apart;
4. the two ends of a bus-splitting jumper: a branch between buses at one voltage whose reactance is
   under a tenth of the case's typical line, likewise unless more than 2 km apart, and only while
   the group stays under 32 buses.

Two buses declared in different substations are never joined by the weaker rules, and any bus none
of them joins is a substation of its own. A branch is a transformer when PowerIO says so (a tap or
phase shift) or when its ends have different nominal voltages. The grouping is a drawing aid, not
an electrical claim.

Network payloads carry the result: `NetworkBus.base_kv`, `NetworkBus.substation` (an index into
`Network.substations`) and `NetworkBranch.transformer`. Each substation has a stable `key`
(`sub:<declared id>` or `bus:<lowest bus id>`), a `name` when declared, the rule that grouped it
(`source`) and its distinct voltages (`kv`). Cases without coordinates group by transformers and
jumpers: ACTIVSg200 from its MATPOWER file gives the 111 substations its aux file declares, its 23
jumpers having a fifteenth of the median line's reactance.

## Engine API

`txSchematicLayout(input, options)` in `@tellegen/engine` (`tellegen::tx_schematic` natively)
lays out the columns of a network: substation per bus, nominal kV, branch ends, service status and
seed positions. It returns bus positions, busbars, substation boxes, routed segments and each
branch's role (line, inside a substation, or out of service), in the units of the seeds.
`seeding: "stress"` lays substations out by their connections rather than their positions;
`route_iterations: 0` draws lines straight. `txGraph` and `txLayoutInput` in `@tellegen/svelte`
build the input from a `Network`.
