# Transmission diagrams

Balanced (transmission) cases open as a single-line diagram. **Show diagram** in the network panel
replaces the map with a canvas; **Show map** or **Close** returns to the map at the view the diagram
showed.

## Geographic

The diagram first draws the case on its own positions: geographic coordinates over the map's
basemap, or drawing coordinates as given. Zoomed out, the largest generators and the transformers
stay findable as fixed-size landmarks, placed by priority so the view never fills with markers.
Closer in, transformers are drawn on their branch as two interlocking circles, each winding in the
colour of the voltage on its side, generators and loads hang below their bus, and buses are named.
Out-of-service branches are dashed.

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
  selection colours, under the common colour-vision deficiencies;
- **shade within a tier**, darker for the higher level;
- **width by rank**, heaviest for the highest level.

The key groups the levels under their tier. Hovering a level in the key keeps it and fades the
others; clicking keeps it highlighted. Zoomed
out, levels below the top three fade so the backbone reads first.

## Results

Once the case is solved, **Colour by: Results** colours buses by the map's current display (LMP,
angle, voltage or demand) and branches by loading, from grey through amber to red at the rating;
branches above 60 % of their rating carry their loading. When the display series is the same at
every bus (a DC OPF without congestion has one price), buses are drawn neutral grey and the legend
gives the value, rather than the middle of the colour ramp. Closer in, a chevron on each branch points
the way active power flows; on a transformer it sits beside the symbol. Hover a bus or branch for its voltage,
substation, rating and solved values; click to select it. Selections made elsewhere (search, the
panels) are brought into view.

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
