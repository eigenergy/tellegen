# Diagram projection for single line and multi wire views

Status: proposed design, 11 October 2026. The accompanying contracts and fixture
are design artifacts, not released PowerIO or Tellegen APIs.

PowerIO should expose a faithful equipment and terminal connectivity view.
Tellegen should project that view into single-line or multi-wire symbols and
connections, then lay out and render the result. Both views retain references to
the same source equipment, terminals and component parts.

The first implementation targets a diagram viewer with selection, study overlays
and saved placement. Geospatial context remains a separate display layer linked
by source identity. Electrical editing, automatic physical conductor inference,
and reconstruction of missing substation detail are outside this proposal.

## Accompanying contracts and example

| File | Purpose |
| --- | --- |
| [powerio.rs](powerio.rs) | Complete proposed Rust connectivity data types with Serde wire tags |
| [tellegen.ts](tellegen.ts) | Complete proposed TypeScript projection, layout and scene types |
| [feeder.connectivity.json](feeder.connectivity.json) | Concrete PowerIO-shaped electrical fixture |
| [feeder.ts](feeder.ts) | Complete hand-authored single-line and multi-wire projections of the fixture |
| [Visual reference gallery](gallery/index.html) | Interactive feeder drawings, paper/dark themes, selection and SVG export |
| [Symbol specimens](symbols/index.html) | Equipment and equivalent-circuit review sheet, paper/dark and monochrome, with SVG export |
| [Symbol guide](symbols/README.md) | Model boundaries, vector registry and review checklist |
| [Gallery guide](gallery/README.md) | How to open, rebuild and verify the local reference gallery |
| [Performance gallery](performance/index.html) | deck.gl baseline with synthetic 1k/10k/100k layouts and external BMOPF cases, including the 98,633-bus SAPN collection |
| [Performance guide](performance/README.md) | Fixture generation, metrics and benchmark limitations |

The Rust and TypeScript files define contracts, not constructors, validators or
general projection algorithms. The example intentionally specifies connectivity
without numerical line, source or transformer parameters; it is a rendering
fixture, not a solvable power-flow case. Array order in the JSON is deterministic
for review but never establishes electrical connectivity.

## Ownership and data flow

| Concern | PowerIO | Tellegen |
| --- | --- | --- |
| Equipment, terminals, winding and load connections | Authoritative model and interpretation | Consume |
| Identity and provenance | Preserve canonical source identity and import conventions | Preserve references on visual objects |
| Neutral and earth semantics | Resolve declared conventions and expose unknowns | Choose glyphs, labels and colours |
| Connectivity projection | Produce and validate renderer-independent facts | Transport through engine and WASM |
| Single-line collapse and multi-wire detail | Supply facts | Own projection policy |
| Symbol dimensions, visual ports and bundles | — | Own |
| Automatic layout and wire routing | — | Own |
| Portable drawing documents | Own interchange schema and parsing | Produce, edit and consume |
| Camera, selection, hover and solver overlays | — | Own |

```text
PowerIO module
  -> ConnectivityView
  -> Tellegen DiagramProjection
  -> measured symbols and layout constraints
  -> DiagramLayout
  -> DiagramScene + study overlays
  -> GPU interactive backend / SVG export + Svelte controls
```

The connectivity view is a derived, read-only contract. It does not replace the
source network, become a solver intermediate representation, or contain a second
copy of impedance matrices. A consumer resolves omitted numerical properties
through source references. In particular, rendering must not depend on a solver
supporting every device in the source module.

Implement the electrical projection close to the PowerIO family models, with a
shared public contract exposed through the PowerIO facade. A separate crate is
not a prerequisite. Put Tellegen projection and layout logic in a framework
independent TypeScript module; `@tellegen/svelte` should consume it. A separately
published `@tellegen/diagram` package can follow once that boundary is stable.

## Why the existing graph is insufficient

This proposal is based on the locally pinned PowerIO 0.11.1 API. Its distribution
`DistGraph` is explicitly a collapsed graph:

- A transformer becomes one edge for each winding pair. Its conductor pairs zip
  terminal lists by position; they are not a physical primary-to-secondary wire
  map.
- Loads and other attachments retain family and ID at bus terminals but lose
  their internal connection description.
- Line and switch `n_phases` values are derived from terminal-map lengths, which
  can include a neutral. A renderer must not equate conductor count with phase
  count.

The richer distribution model already retains load configurations, transformer
windings, terminal maps and winding neutral impedances. Expose those semantics
without first passing through the collapsed graph. Keep `DistGraph` compatible
for existing consumers; add `ConnectivityView` alongside it.

Relevant Tellegen integration points are the [distribution ingest adapter](../../../crates/tellegen-wasm/src/dist.rs),
the [browser distribution types](../../../packages/engine/src/index.ts),
the [distribution drawing adapter](../../../packages/svelte/src/lib/multiconductor.ts),
and [DiagramCanvas](../../../packages/svelte/src/lib/components/DiagramCanvas.svelte).
The current [display contract](../../src/display-data.md) already separates drawing
and geographic coordinate spaces.

## PowerIO connectivity contract

### Identity and connection points

The proposed top-level Rust type is:

```rust
pub struct ConnectivityView {
    pub schema_version: u32,
    pub model_ref: ModelRef,
    pub buses: Vec<BusGroup>,
    pub points: Vec<ConnectionPoint>,
    pub equipment: Vec<Equipment>,
    pub grounding: Vec<GroundConnection>,
    pub diagnostics: Vec<ConnectivityDiagnostic>,
}
```

`ModelRef { network, revision }` separates persistent network identity from the
snapshot used for validation and results. `EquipmentRef { family, key }` prevents
a line and transformer with the same source name from colliding. References are
scoped to `model_ref.network`; revision is carried once by the containing view.

`SourceRef` addresses a bus, bus terminal, equipment record or equipment part.
Part kinds include `end`, `pin`, `conductor`, `pole`, `winding`, `coil`, `element`
and `grounding`. Part IDs are unique within their equipment and part kind. The
schema must document those namespaces, and validation must resolve each part.
Unknown family/part-kind strings may be retained by an importer, but do not imply
that a renderer knows how to expand them.

A `ConnectionPoint` is one electrical connection location. Its owner is a bus,
equipment or model reference scope. A `BusGroup` groups points for presentation;
it does not electrically union them. `S.A` and `S.N` remain different nodes even
when a single-line port visually bundles them.

Points have a label, semantic role, nature, provenance and source references.
`TerminalRole::Phase { designation }` distinguishes a source terminal name from
its interpreted phase designation. An unfamiliar label is `Unknown`, not a
neutral. Roles must follow PowerIO's source-format interpretation; a renderer
must not repeat that interpretation from naming heuristics.

`PointNature::ModelOnly` identifies reference and normalization points that are
not evidence of an explicit physical conductor. A declared ground/reference and
a wire are separate concepts. Preserve enough import provenance to distinguish
a source neutral from a normalized implicit ground terminal.

### Equipment ends and pins

```rust
pub struct Equipment {
    pub id: EquipmentRef,
    pub label: String,
    pub availability: Availability,
    pub pins: Vec<Pin>,
    pub ends: Vec<ConnectionEnd>,
    pub model: EquipmentModel,
    pub provenance: Provenance,
}

pub struct Pin {
    pub id: LocalId,
    pub binding: PinBinding,
}

pub struct ConnectionEnd {
    pub id: LocalId,
    pub label: String,
    pub pins: Vec<LocalId>,
}
```

A pin belongs to one equipment record. An end groups its external pins, such as
the four pins at one cable end or the pins of one winding. Every external pin
belongs to exactly one end. Shared internal points belong to the equipment and
do not require an invented external end.

`PinBinding` is `Connected { point }`, `Disconnected` or `Unknown`. These states
are different: an open switch normally has connected external pins and an open
internal pole. Do not represent an open switch by removing one of its pins.

Endpoints of an internal coil or load element use `Contact::Pin { pin }` or
`Contact::Point { point }`. A point contact can reference an equipment-local point
or a model reference, allowing internal star points and centre taps. External
bus connections must pass through pins. This makes the connection boundary
explicit even for equipment with shared internal nodes.

### Typed equipment relationships

```rust
pub enum EquipmentModel {
    ConductorPaths { paths: Vec<ConductorPath> },
    Switch { poles: Vec<SwitchPole> },
    Transformer { windings: Vec<Winding>, magnetic: MagneticDetail },
    TerminalDevice {
        configuration: ConnectionConfiguration,
        elements: Vec<TerminalElement>,
    },
    Aggregate { description: String },
    Opaque { reason: String },
}
```

The [complete Rust file](powerio.rs) supplies all supporting types and Serde tags.
The variants have these meanings:

| Variant | Electrical facts retained |
| --- | --- |
| `ConductorPaths` | Each conductor's two equipment pins, stable ID, nature and provenance; finite impedance remains in the source model |
| `Switch` | Per-pole endpoint pins and open, closed or unknown state |
| `Transformer` | Winding identity, external end, configuration, coil endpoint pairs, polarity knowledge and magnetic detail |
| `TerminalDevice` | Explicit two-terminal elements, such as A–N, B–N, C–N loads or A–B, B–C, C–A loads |
| `Aggregate` | A representation such as a balanced branch, without an assertion of explicit conductor topology |
| `Opaque` | Known external pins with unavailable or unsupported internal structure |

A path, coil or load element is not an ideal node merge. Consumers must not use
this structure as a union-find input without interpreting relationship types.
Likewise, it is not by itself an energization algorithm: voltage references,
equipment models, switches and magnetic coupling have different meanings.

The explicit coil/element endpoints are authoritative within this derived view;
`configuration` supports validation and labels. PowerIO elaborates endpoints
using the source model's conventions. If the source does not establish the
connection, emit `Opaque` with a diagnostic rather than guessing from terminal
order. A generic multiport shunt described only by an admittance matrix may use
that fallback; off-diagonal numerical coefficients alone do not establish a
physical wiring diagram.

Each conductor path distinguishes an explicit conductor from a model-only path
or one of unknown nature, with its own provenance. Equipment-level provenance
alone is insufficient when normalization adds one path to an otherwise explicit
cable. A model-only return path must be identified as such in a multi-wire view;
it is not evidence that a physical neutral or earth wire exists. The fixture's
six paths are all declared explicit conductors.

`MagneticDetail::Unspecified` means the equipment is a transformer but detailed
coil associations are unavailable. `CoilGroups` can retain source-declared or
convention-derived associations with provenance. These groups are descriptive;
they do not encode numerical mutual inductance, vector-group phase shifts or a
general transformer equivalent circuit. Unknown coil polarity must not produce
polarity dots.

`Availability` and pole state are independent. All equipment remains in the
physical arrangement when out of service. A source status change does not
delete a component or force automatic placement to move its neighbours.

### Grounding and balanced inputs

`GroundConnection` records the target point, scoped reference, impedance kind,
source references and provenance. `GroundImpedance` distinguishes ideal,
finite impedance and unknown connections. Numerical impedance values carry ohm
units; the source model remains authoritative. Equal earth labels in unrelated
reference scopes do not establish a connection.

Grounding annotations do not create an explicit earth-return conductor. An
explicit protective-earth wire is a conductor path with an appropriate terminal
role. Preserve that distinction in both views.

A balanced input can use one aggregate point per bus and aggregate equipment
models. Its single-line diagram remains useful. A multi-wire request must report
the unavailable detail and retain a clearly marked aggregate placeholder. It
must not invent A, B, C or N terminals. Converting a balanced network into an
explicit phase model would be a separate, recorded transformation.

## Tellegen projection contract

The TypeScript projection contains visual intent but no positions:

```ts
interface DiagramProjection {
  schema_version: 1;
  model_ref: ModelRef;
  mode: "single_line" | "multi_wire";
  symbols: DiagramSymbol[];
  ports: DiagramPort[];
  connections: DiagramConnection[];
  groups: DiagramGroup[];
  decorations: MagneticDecoration[];
  notices: DetailNotice[];
}

interface DiagramPort {
  id: DiagramId;
  symbol: DiagramId;
  grouping: "individual" | "bundle" | "unresolved";
  points: PointId[];
  pins: PinRef[];
  side_hint?: "north" | "south" | "east" | "west";
  order_hint?: number;
}
```

The [complete TypeScript file](tellegen.ts) also defines source references, symbol
glyphs, groups, connection meanings, layout output and scene primitives. Its
PowerIO reference types are illustrative mirrors; production TypeScript types
should be generated from the Rust schema.

An individual resolved port has one connection point. A bundle lists its
members without merging them. Several visual ports can refer to the same point,
for example the shared neutral on three separate load elements. An unresolved
port preserves unknown/disconnected pin identity and has no invented point.

Symbols refer to a versioned glyph registry through semantic names such as
`coil`, `load` and `terminal_rails`. A multi-wire `terminal_rails` glyph contains
separate rails for each point; it never draws one bar through all phases. Symbol
port order influences presentation only. Connection identity determines wiring.

Every drawable wire states its meaning:

```ts
type WireMeaning =
  | { kind: "attachment" }
  | { kind: "conductor"; equipment: EquipmentRef; paths: string[] }
  | { kind: "switch_pole"; equipment: EquipmentRef; poles: string[] }
  | { kind: "grounding"; grounding: string };
```

An attachment joins two visual occurrences of the same point or point set; it
does not add electrical connectivity. A conductor or pole resolves its endpoint
mapping from the referenced source parts. Its path/pole list is essential for a
lateral using only A and N at a four-member single-line bus port. Never pair
bundle members by their visual positions.

A coil appears as a symbol with two contact ports and attachment leads, not as
a primary-to-secondary wire. Magnetic coupling is a separate decoration that
cannot participate in wire connectivity or electrical hit testing.

Use stable visual IDs derived from source identity, representation role and
occurrence. One source equipment may appear as several coil/load symbols or in
several diagram regions. An equipment selection selects all relevant occurrences;
a coil selection retains its narrower part reference. Do not resolve identity
through array indices, labels or global lowercasing in the browser.

### Projection policies

`projectSingleLine(view, request)` bundles terminals by equipment end, emits
separate objects for parallel circuits, and labels important membership and
connection distinctions. It preserves open switches and grounding targets.
Independent circuits sharing endpoint buses must not collapse into one line.

`projectMultiWire(view, request)` expands conductors and supported internal
connections, preserving the exact source endpoints. Unknown internals stay
opaque with a notice. A `ProjectionRequest` can focus a neighbourhood or override
selected equipment to compact/expanded detail. Mixed detail needs an explicit
breakout glyph and membership mapping at each transition.

`DetailNotice` records intentional collapse, excluded scope or unavailable
detail. Hiding an object for readability never deletes its source meaning.
Single-line projection does not convert an unbalanced model into a balanced one.

## Four wire feeder fixture

The feeder has four bus groups and six equipment records. H is an upstream
three-terminal bus. T1 has a delta primary and a wye secondary with its neutral
grounded at S. L1 carries A, B, C and N from S to F. L2 carries A and N from F to L.
Q3 is a three-element wye load at F; Q1 is an A–N load at L.

```text
U -- H -- T1 Δ/Yg -- S ==== L1 ABCN ==== F
                                          |-- Q3 Y, ABCN
                                          |
                                          +==== L2 AN ==== L -- Q1 A–N
```

These diagrams show topology, not an automatic placement result. Electrical
connections are specified fully by the fixture and the tables below.

| Object | Connection points or parts |
| --- | --- |
| H | H.A, H.B, H.C |
| S | S.A, S.B, S.C, S.N |
| F | F.A, F.B, F.C, F.N |
| L | L.A, L.N |
| earth | One model-only reference point in `fixture-earth-reference` |
| U | Pins A, B, C bound to H; internal connections outside fixture scope |
| T1 primary | p.A→H.A, p.B→H.B, p.C→H.C |
| T1 secondary | s.A→S.A, s.B→S.B, s.C→S.C, s.N→S.N |
| L1 | from.A/B/C/N at S; to.A/B/C/N at F; four separate conductor paths |
| L2 | from.A/N at F; to.A/N at L; two separate conductor paths |
| Q3 | Elements AN, BN and CN at F |
| Q1 | Element AN at L |
| GS | Ideal connection from S.N to earth; no other explicit ground bond |

The fixture contains 14 connection points: 13 bus terminals and one reference.
F.N and L.N connect back to the grounded source neutral through their respective
line conductors; they are not independently declared ideal ground points.

T1's explicit coil contacts are:

| Coil | Contact a | Contact b |
| --- | --- | --- |
| pAB | p.A → H.A | p.B → H.B |
| pBC | p.B → H.B | p.C → H.C |
| pCA | p.C → H.C | p.A → H.A |
| sAN | s.A → S.A | s.N → S.N |
| sBN | s.B → S.B | s.N → S.N |
| sCN | s.C → S.C | s.N → S.N |

Coil endpoint order does not assert a polarity. The fixture declares neither
polarity nor magnetic leg associations. A shared transformer annotation is valid;
pairwise primary/secondary magnetic links or phase-shift labels are not.

### Single line projection

The complete `singleLine` export in [feeder.ts](feeder.ts) contains nine symbols,
11 ports and eight connections. Bus symbols are H, S, F and L; equipment symbols
are U, T1, Q3 and Q1; GS is the ground symbol. Lines are connections rather than
extra symbol boxes.

| Port | Point membership | Equipment pin membership |
| --- | --- | --- |
| H.bundle | H.A, H.B, H.C | — |
| S.bundle | S.A, S.B, S.C, S.N | — |
| F.bundle | F.A, F.B, F.C, F.N | — |
| L.bundle | L.A, L.N | — |
| U.out | H.A, H.B, H.C | U A, B, C |
| T1.p | H.A, H.B, H.C | T1 p.A, p.B, p.C |
| T1.s | S.A, S.B, S.C, S.N | T1 s.A, s.B, s.C, s.N |
| Q3.in | F.A, F.B, F.C, F.N | Q3 A, B, C, N |
| Q1.in | L.A, L.N | Q1 A, N |
| S.neutral | S.N only | — |
| GS.ref | earth | — |

| Connection | Visual endpoints | Meaning |
| --- | --- | --- |
| U.lead | U.out → H.bundle | Attachment |
| T1.p.lead | H.bundle → T1.p | Attachment |
| T1.s.lead | T1.s → S.bundle | Attachment |
| L1 | S.bundle → F.bundle | L1 conductors A, B, C, N |
| L2 | F.bundle → L.bundle | L2 conductors A, N only |
| Q3.lead | F.bundle → Q3.in | Attachment |
| Q1.lead | L.bundle → Q1.in | Attachment |
| GS.bond | S.neutral → GS.ref | Grounding GS |

T1's single symbol represents both windings. There is no conducting edge between
T1.p and T1.s. Its Δ/Yg badge is derived from winding configuration and the
declared neutral bond. Q3 and Q1 show their connection/membership labels.

The dedicated S.neutral port matters: grounding the entire S.bundle would imply
an electrical relation the source does not contain. A glyph may place that port
beside the bundle with an N label; it must preserve its narrow membership.

For example, L2 is represented as:

```ts
{
  id: "L2",
  from: "F.bundle",
  to: "L.bundle",
  meaning: {
    kind: "conductor",
    equipment: { family: "line", key: "L2" },
    paths: ["A", "N"]
  },
  sources: [
    { kind: "part", equipment: { family: "line", key: "L2" },
      part_kind: "conductor", part: "A" },
    { kind: "part", equipment: { family: "line", key: "L2" },
      part_kind: "conductor", part: "N" }
  ]
}
```

This explicitly selects F.A and F.N through L2's path endpoints; F.B and F.C are
not part of the lateral despite being members of F.bundle.

### Multi wire projection

The complete `multiWire` export has 16 symbols, 37 ports, 30 wire connections,
four groups and one nonconducting magnetic decoration. Every port has exactly
one point. The four bus symbols use separate terminal rails.

The feeder conductors and loads expand as follows; `[Q]` denotes a two-terminal
load element, and each row states its own connectivity:

```text
S.A -- L1.A -- F.A -- L2.A -- L.A
S.B -- L1.B -- F.B
S.C -- L1.C -- F.C
S.N -- L1.N -- F.N -- L2.N -- L.N
 |
 GS -- earth

F.A -- [Q3.AN] -- F.N
F.B -- [Q3.BN] -- F.N
F.C -- [Q3.CN] -- F.N
L.A -- [Q1.AN] -- L.N
```

T1 expands into two winding groups, containing six separate coil symbols:

```text
primary delta                    secondary wye
H.A -- [T1.pAB] -- H.B            S.A -- [T1.sAN] -- S.N
H.B -- [T1.pBC] -- H.C            S.B -- [T1.sBN] -- S.N
H.C -- [T1.pCA] -- H.A            S.C -- [T1.sCN] -- S.N

       shared T1 magnetic annotation; no interwinding wire
```

Coil ports point to their actual contacts. For example, T1.pAB.a refers to H.A
and pin p.A; T1.pAB.b refers to H.B and pin p.B. T1.pCA.b also refers to H.A,
so both leads attach to the same H.A rail. This is a delta connection even if the
layout does not place the three coils in a literal triangle.

All 30 wire connections are enumerated by these families:

| Family | Expansion | Count |
| --- | --- | ---: |
| U leads | U.A/B/C to H.A/B/C | 3 |
| T1 primary leads | Two contacts for each of pAB, pBC, pCA | 6 |
| T1 secondary leads | Two contacts for each of sAN, sBN, sCN | 6 |
| L1 conductors | S.t to F.t, t ∈ A, B, C, N | 4 |
| L2 conductors | F.t to L.t, t ∈ A, N | 2 |
| Q3 leads | F.t to Q3.tN.a and F.N to Q3.tN.b, t ∈ A, B, C | 6 |
| Q1 leads | L.A to Q1.AN.a and L.N to Q1.AN.b | 2 |
| Ground bond | S.N to GS.ref at earth | 1 |

Coils and load elements themselves are symbols, not additional wire edges. The
source remains one three-port symbol with an explicit unavailable-detail notice.
The magnetic annotation belongs to T1 and connects visual coil occurrences only;
it contributes zero electrical connections.

### Selection across views

| User selects | Single-line view | Multi-wire view |
| --- | --- | --- |
| L1 equipment | L1 stroke | All four L1 conductor strokes |
| L1 conductor N | L1 stroke with N identified in the inspector | L1.N stroke |
| T1 equipment | T1 symbol | T1 winding groups and all six coil symbols |
| T1 coil pAB | T1 symbol with collapsed-detail indication | T1.pAB symbol |
| F.N terminal | F bundle membership | F.N rail and attached lead occurrences |

A narrower selection must not become a broader electrical edit just because the
current view is collapsed. Future editing commands must target source references,
never the visual object alone.

## Visual style and reference gallery

### Tellegen drawing language

The drawing treatment uses quiet working surfaces, a restrained dotted grid,
compact terminal panels, clear title and pin hierarchy, smoothly bent conductor
paths, explicit branch points and colour concentrated in wires. Both projections
preserve the relationship between complete equipment and individual connections.

Tellegen retains its own design language: Bricolage Grotesque for interface and
equipment labels, IBM Plex Mono for identifiers and terminal mappings, warm paper
surfaces and burnt amber interaction accents. The default gallery theme is paper;
a charcoal theme provides an alternative contrast without changing topology,
source identities or symbol meaning. The production application still owns its
theme; the gallery's dark palette is a proposal, not a new application default.

| Visual principle | Tellegen treatment |
| --- | --- |
| Quiet canvas | Paper or charcoal drawing surface, with a faint dotted alignment grid |
| Compact terminal hierarchy | Restrained group frames for equipment detail; labelled individual terminal rails |
| Smooth routed lanes | Orthogonal paths with small rounded bends; exact terminal membership remains authoritative |
| Colour concentrated in connections | Optional phase mode, with A/B/C/N labels and a persistent legend |
| Assembly and conductor detail | Shared source selection across the single-line and multi-wire views |

### Theme and geometry rules

The gallery implements these provisional tokens. Names express drawing roles;
production integration should map them onto the shared Svelte design tokens
rather than maintain a separate application palette.

| Role | Paper | Dark |
| --- | --- | --- |
| Page | `#ece9e2` | `#111418` |
| Drawing | `#fcfbf7` | `#171b20` |
| Equipment detail surface | `#f5f2eb` | `#1e2329` |
| Primary text | `#20242b` | `#eeeae1` |
| Neutral wire | `#454a50` | `#cbc8bf` |
| Interaction accent | `#b25e00` | `#e5aa54` |
| Phase A | `#b6453e` | `#ed8b81` |
| Phase B | `#347a49` | `#90c397` |
| Phase C | `#376ba6` | `#8fb7ea` |
| Neutral conductor | `#70675b` | `#c2b8a8` |

Phase colours are display conventions. The proposed shades retain the existing
rust/green/blue family with stronger contrast on paper and lighter variants on
charcoal. They do not claim a regional wiring standard. Unknown terminal roles
must use an explicit unknown label, not inherit the neutral colour by default.

Use approximately 1.8–1.9 drawing units for symbols and wires, 4 for busbars,
and 7-unit bend radii at this reference scale. Symbols have conventional
electrical geometry; rounded corners belong to routes and quiet group frames.
Selection uses an outer amber halo so the conductor's phase colour remains
visible. Hover and focus enlarge or outline the hit target without changing
electrical state. The drawing grid never establishes connectivity.

Labels use a consistent hierarchy: equipment ID, configuration or conductor
membership, then optional operating values. Text stays horizontal except where
a future explicit drawing convention requires otherwise. Reserve label space
before routing. Show no invented ratings, line lengths or numerical results in
a connectivity-only fixture.

Topology mode renders wires neutrally. Phase mode colours conductors and their
terminal marks. In a single-line bundle, membership labels carry the phase
information; one arbitrary phase colour must not stand for the whole bundle.
A future result mode must specify its quantity and units and use a separate
legend. Keep the user's interaction highlight distinct from all three modes.

### What the first gallery covers

Open [gallery/index.html](gallery/index.html) directly in a browser, or serve
the directory as described in the [gallery guide](gallery/README.md). All assets
are local. The gallery provides:

- Single-line and multi-wire views of the same feeder, using the existing
  `feeder.ts` projection exports and `feeder.connectivity.json` source fixture.
- Paper and dark themes, plus topology and phase colouring.
- Click and keyboard selection, retained when switching views, with source
  terminal mappings in an inspector.
- SVG export of the current drawing with resolved colours and styles.
- A narrow-screen canvas that scrolls instead of shrinking all terminal labels.

Geometry is manually authored for this fixture. The gallery demonstrates the
visual contract and exposes electrical detail; it is not a general diagram
renderer or evidence that automatic layout is solved. It does not modify network
data. The same projections still contain nine symbols and eight connections in
single-line mode, and 16 symbols and 30 connections in multi-wire mode.

The multi-wire transformer shows its three delta coil pairs and three wye coil
pairs without an interwinding conductor. No polarity dots or specific magnetic
leg pairings are invented. The sole declared ground bond is S.N to earth. Load
elements use generic two-terminal blocks, not resistor zigzags that would imply
a resistive load model. A masking stroke separates crossing routes without
creating a junction; dots identify actual branching connection points.

### Equipment and equivalent-circuit specimens

The [symbol specimen sheet](symbols/index.html), served over HTTP as described in
its [guide](symbols/README.md), is the next visual review fixture. It provides
Equipment, Equivalent circuit and side-by-side Compare views, paper and dark
themes, monochrome in either theme, and portable SVG export. The visual treatment
shares the feeder gallery's fonts, surfaces, neutral wire strokes and restrained
amber accents. Shape and text carry all electrical meaning without colour.

Two independent choices govern diagram representation:

1. **Projection:** single-line equipment/bundle structure or multi-wire terminal
   and conductor structure.
2. **Detail:** equipment identity or an explicitly declared equivalent model.

Selecting equivalent detail must not silently replace physical terminals, infer
conductor continuity or synthesize a study model. The specimen sheet chooses
bounded scalar examples deliberately; it does not yet implement a multi-wire
symbol registry or derive equivalents from BMOPF datasets.

| Specimen | Equipment view | Declared equivalent and boundary |
| --- | --- | --- |
| π line | Connection with π model badge | Series `Zs` between endpoints; `Yf` and `Yt` each terminate at explicit voltage reference `0`; end values follow PowerIO, including symmetric halves where specified |
| Generator | Circle with `G` | Internal voltage `E` behind `Zg`; chosen example, not a universal generator/inverter model |
| External supply | AC source circle | Thévenin `Vth` behind `Zth`; no implied slack assignment |
| Load | Demand arrow | Constant-power `S = P + jQ` block; does not imply a resistor or solved flow direction |
| Transformer | Two winding circles | Primary-referred leakage `Zℓ` plus ideal `a:1` transformer; magnetizing branch omitted by declaration; separate labelled references `0p` and `0s`, no interwinding conductor |
| Capacitive shunt | Capacitor plates | `Yc = jωC` between bus and external return terminal; no implicit fixed reference or earth connection |
| Switch, closed/open/unknown | Blade/contact geometry or explicit unknown marker | Ideal closed `ΔV = 0`, open `I = 0`, or unresolved state with no inferred conductive relation; position remains separate from service availability |

The vocabulary is IEC-oriented, pending engineer review, rather than a formal
standards compliance claim. [IEC's diagram-symbol guidance](https://tc3.iec.ch/tc-activity/graphical-symbols-for-diagrams/)
provides the standards context. [EPRI's line-model documentation](https://opendss.epri.com/Line.html)
illustrates why a π symbol alone cannot specify conductor-coupled matrix models.

**Reference-node convention and PowerIO alignment.** The π equivalent has no
horizontal lower rail: each end shunt terminates at an explicit triangle labelled
`0 — model reference`. The two markers denote the same voltage reference within
that specimen. They are not floating terminals. With endpoint currents entering
the line and voltages measured relative to `0`, the scalar equations are:

```text
If = (Vf − Vt) / Zs + Yf Vf
It = (Vt − Vf) / Zs + Yt Vt
```

A floating lower rail would instead introduce a series path through the two shunt
impedances, in parallel with `Zs`; that is not this model. Reference symbols are
voltage-reference declarations, not declarations of a physical earth bond. Their
identifiers are scoped to each specimen, never used to connect unrelated cards.

This was checked against PowerIO **0.11.3**, pinned in this worktree's Cargo.lock.
`DistLineCode` stores `r_series`/`x_series` and separate
`g_from`/`b_from`/`g_to`/`b_to` conductor matrices per unit length. Tellegen's
[multiconductor line stamp](../../../crates/tellegen/src/mc_pf/network.rs)
multiplies by length and adds end shunts to their respective diagonal blocks.
Transmission `Branch::calc_terminal_charging()` preserves explicit `BranchCharging`
values or derives equal `b/2` susceptance halves from legacy total `b`. The renderer
must use these resolved semantics, not invent either equal or unequal end values.
Multi-wire equivalents require the full conductor matrices and terminal mapping;
a scalar drawing cannot convey mutual coupling.

**Equivalent-circuit ambiguity review.** Generator, supply and constant-power load
now use labelled `0` triangles rather than hollow internal nodes. Transformer
windings use distinct `0p` and `0s` reference markers; no conductor joins them.
The capacitive shunt deliberately remains a two-terminal example: its hollow
`return (external)` port has no assigned reference or grounding connection.
Closed/open/unknown switch glyphs retain their distinct continuity, gap and
question-mark geometry. Unknown position has no inferred ideal-switch equation.
Small hollow circles denote external ports or switch contacts, not fixed reference nodes.
The convention and labels survive monochrome and standalone SVG export.

**Ownership.** PowerIO continues to own equipment identities, terminal membership,
conductor and winding relations, explicit switch-pole state, and available model
parameters with provenance. Missing or incompatible model detail remains explicit.
Tellegen owns symbol selection, primitive geometry, ports, bounds, labels, theme,
level of detail and interaction. Neither theme nor symbol choice changes electrical
semantics. Equivalent-model expansion needs its own source mapping for internal
model elements; visual reference nodes must not become physical network buses.
No canonical PowerIO schema changes are introduced by this specimen-only step.

**Implementation.** The small local vector registry in `symbols/symbols.mjs`
contains stable fixture IDs, shared external ports, separate primitive lists for
equipment and equivalent detail, and per-specimen assumptions. A common renderer
feeds both HTML SVGs and standalone SVG export. Symbols have fixed local bounds;
text remains horizontal. Transformer coupling is a non-conductive annotation.
Export resolves the palette and preserves source/port attributes and assumptions
in SVG descriptions. The review IDs are fixture identities, not imported network
references; this registry is not yet the production `DiagramScene` contract.

**Scale and next gate.** Keep equipment symbols compact in the network overview.
Equivalent-circuit detail belongs in a selected, bounded neighbourhood or inspector,
with space reserved before routing. Approved vector geometry should feed instanced
or batched deck.gl primitives, not one DOM element or GPU layer per equipment item.
Reuse stable geometry across theme changes and keep labels budgeted. This sheet
adds no work to the 100k-bus render loop and makes no new throughput claim. Before
integration, review the specimens with power engineers, add unavailable-detail and
multi-wire examples (including conductor coupling and explicit winding topology),
and measure glyph/label budgets on the combined SAPN collection.

### Visual review gate

Review both representations in both themes and in monochrome. A power engineer
should be able to trace L2.A and L2.N, locate the sole ideal ground bond, identify
the delta and wye connections, and distinguish shared terminals from crossings.
Changing theme, colour mode or selection must leave all source mappings intact.
Cross-view selection must preserve a conductor or coil selection even when the
compact view can show only its containing equipment.

Before production integration, extend the switch specimens to per-pole states and
add reference specimens for parallel circuits, a phase permutation, a centre tap, impedance grounding,
unavailable detail and quantitative overlays. Adopt approved specimens as visual
regression references after engineer review. Automatic layout must preserve the
same semantic and visual invariants.

## Scale requirement: 100,000 buses

The production single-line viewer must support a source network with **100,000
buses**, with every bus and equipment item searchable, inspectable and traceable
to source identity. This is a design requirement, not a demonstrated performance
guarantee. Network size, on-screen primitive count and readable detail are separate
budgets. Record equipment, terminal, branch, routed-segment and label counts in
addition to buses. Multi-wire expansion can multiply those counts substantially.

The [performance gallery](performance/index.html) establishes a full-geometry
baseline using deck.gl and precomputed 1k/10k/100k layouts. Its
[guide](performance/README.md) describes reproducible fixtures and measurements.
It complements the SVG electrical specimen; it does not replace the semantic
projection or prove automatic layout at this scale.

### BMOPF dataset experiment

The gallery now has a bounded, rendering-only BMOPF adapter and dataset selector.
It stages derived buffers and a source-reference sidecar from an external data
root; raw BMOPF files stay external. Both generated artifacts are ignored by Git.
The committed catalog/inventory contains relative paths, source qualifications,
input hashes and counts. See the [adapter contract](performance/README.md#adapter-contract)
for supported source shapes and explicit rejection rules.

The combined SAPN fixture retains **98,633 buses, 99,822 lines and 1,461 transformer
objects** across five independent source namespaces. Collection composition never
merges matching IDs or adds electrical connections. Display panels preserve scale
but deliberately translate each source separately; original geographic coordinates
and projection offsets remain available. Three French sizes and the 22,081-bus
Texas pilot provide small-case, meshed, transformer-leg and internal-bus coverage.

This POC keeps the future PowerIO/Tellegen boundary intact: canonical electrical
semantics belong in PowerIO; display coordinates, segment packing, indexes and
panel composition belong in Tellegen. The temporary adapter records source
terminal maps and grounding explicitly but does not elaborate transformer winding
internals or claim a general multi-wire conversion. Transformer edges are tagged
as equipment associations. A later multi-wire view must consume validated
connectivity/winding semantics, not expand those edges into assumed phase links.

Stable identity is `(source namespace, family, kind, original ID)`, independent of
buffer offsets and display grouping. Co-located internal buses are retained and
individually searchable. A display detour makes zero-length equipment associations
selectable without moving source buses or inventing electrical nodes. Attachments
remain traceable through the inspector. Loading validates hashes and reference
integrity before installing geometry; GPU layer identities include dataset IDs and
hashes, rather than only bus counts.

The initial combined fixture has 4.76 MiB of geometry and 49.25 MiB of reference
JSON. Treat reference parsing, identity indexes and peak heap as separate budgets
from owned typed arrays and GPU drawing. Follow-up experiments should measure
compact reference columns, lazy equipment details and worker preparation alongside
chunking and semantic level of detail. Geographic-coordinate density is now tested;
a first source-rooted schematic layout is now available alongside the coordinate
reference. Multi-wire expansion and operating overlays remain separate experiments.

### Structural schematic experiment

The diagram layer is a parallel structural view of the same electrical model,
not a replacement or improvement to Tellegen's existing geospatial visualizer.
The performance gallery now switches a BMOPF dataset between its source-coordinate
reference and a coordinate-independent schematic, preserving selection identity.
The first engineering specimen is the 540-bus French feeder; the combined SAPN
collection tests scaling to 98,633 buses.

Tellegen owns the schematic layout: choose a source-rooted spanning forest,
place buses by topological depth and subtree leaf order, route orthogonally,
and classify retained non-tree links explicitly. PowerIO continues to own source
semantics, conductor/terminal connectivity, grounding and winding relationships.
Layout parentage must not be interpreted as power-flow direction, energized state
or interwinding conductor continuity. Components without a declared source have
explicit fallback roots; multiple source declarations remain available.

The precomputed schematic scene shares its bus/branch indexing and source-reference
sidecar with the coordinate scene. Additional renderer arrays hold parent bus,
parent edge, depth, component and route role. There are no synthetic electrical
nodes, dropped mesh links or source-ID rewrites. Switching projections changes
geometry and camera state while retaining the inspected equipment. A root-path
trace is a structural navigation operation over the display tree.

The algorithm is deterministic, iterative and bounded by graph size, apart from
sorting root candidates. It lays out the inspected SAPN collection in roughly
47 ms in one warm local Node process, retaining 2,655 non-tree links and generating
264,100 routed segments. This is a POC observation, not a production target or an
optimized general graph layout. It does not address crossing minimization, semantic
symbols/ports, incremental editing, voltage-level grouping or multi-wire expansion.

The scale result identifies the next design question: a full-detail tree with
thousands of leaves becomes tall even when it computes and draws quickly. Develop
feeder/transformer-group aggregation and scoped drill-down as distinct structural
views. Validate comprehension, root-to-equipment paths and non-tree connectivity
with engineers; use frame rates as a separate constraint. The exact baseline must
remain available to validate that an aggregate never fabricates connectivity.
See the [layout contract and tests](performance/README.md#schematic-layout-mode).

The gallery now offers **Leaf order**, **Tidy tree**, and **Source coordinates**.
The tidy strategy changes only subtree placement on the same source-rooted BFS
forest, keeping all original bus/equipment identities, disabled links and non-tree
connections. Each connected component is packed independently before placing
source panels. A mesh therefore retains its extra connections while its spanning
tree supplies the reading structure; this is not electrical radiality analysis.

An independent iterative Buchheim-style implementation separates each finished
child subtree before starting its next sibling. This fixes the asymmetric-tree
spacing failure found during the study of Numair's diagram algorithm. Its layout
arrays and passes remain linear in the tree size. PowerIO supplies connectivity,
identity and state; strategy, spacing, root selection and geometry stay in Tellegen.
Neither the geospatial layer nor the electrical import schema changes.

On the 540-bus French feeder and 98,633-bus SAPN collection, tidy packing reduces
full drawing height by about 47% and 73%, respectively, at unchanged depth spacing.
The [reproducible comparison](performance/README.md#tidy-tree-comparison) records
layout timings, bus spacing, geometry size and overlap diagnostics. Parent paths,
endpoints, states and branch roles are equal across the two schematic strategies.
The remaining collinear route overlaps and dense SAPN overview make routing and
semantic aggregation the next experiments. Future multi-wire expansion needs
measured symbol/terminal footprints and conductor corridors; compact bus placement
must not be mistaken for completed multi-wire routing.


### Renderer and library boundary

Use a backend-independent scene/glyph contract. Prototype the interactive backend
with the existing deck.gl dependencies and `OrthographicView` in Cartesian
schematic coordinates. Geospatial context remains independent. Keep SVG as a
reference and vector-export backend for scoped drawings. Svelte owns controls,
accessible navigation and inspectors; do not mount a component or DOM element
for every bus, label or conductor in the full network.

Batch line geometry and repeated symbols into GPU buffers. Preserve source IDs
through picking and selection. Treat labels, selection and study overlays as
separate layers. Maintain an accessible search/list/inspector route to items that
are absent from the rendered DOM. The canvas alone is not a complete accessible
representation. Backend conformance includes glyphs, line widths, junctions,
selection and exports; performance does not permit fabricated connectivity.

Start with deck.gl reuse rather than a new graph-editor framework. Its
[orthographic view](https://deck.gl/docs/api-reference/core/orthographic-view)
supports a non-geospatial canvas; its
[performance guidance](https://deck.gl/docs/developer-guide/performance) explains
data invalidation and GPU-buffer costs. This is a prototype choice, not a promise
that generic layers will cover every electrical glyph efficiently. Benchmark
specialised instanced glyphs when the full symbol set is introduced.

### Bottlenecks and required controls

| Cost | Required control |
| --- | --- |
| Global placement, crossing reduction and obstacle routing | Hierarchical layout, bounded local problems, caching and cancellation |
| DOM/scene expansion and CPU–GPU traffic | Batched buffers, stable data identities, incremental dirty ranges |
| Off-screen geometry | Spatially indexed chunks with a cache and prefetch margin |
| Fully visible overview density | Semantic aggregation and simplified glyphs; viewport culling alone is insufficient |
| Text measurement and overlap | Cached metrics, spatial label candidates, priority and a screen-space label budget |
| Pointer hit testing and path selection | Spatial or GPU picking, indexed adjacency and small highlight overlays |
| Duplicate data and large GC pauses | Interned identities, compact indexed arrays, transferable buffers, bounded caches |
| Result/state refresh | Appearance-only updates with stable layout and routes |
| Very large coordinate extents | Chunk-local coordinates or camera-relative transforms to retain precision |
| Export | Explicit region/detail scope, tiled or paginated export for larger drawings |

GPU clipping is not CPU culling: invisible geometry can still incur upload and
vertex work. Conversely, a spatial index is not sufficient when the complete
network lies inside an overview viewport. Record submitted and visible counts
separately when culling and aggregation are added.

### Hierarchy, level of detail and electrical meaning

Support network overview, feeder/substation view and equipment/terminal detail.
Overview aggregates have stable visual IDs, source-member mappings, boundary
ports and a declared aggregation policy. An aggregate is a drawing object, never
a replacement electrical bus. Preserve parallel-circuit identity, boundary
connectivity, open/closed state and important exceptions. Do not hide an open
tie inside an apparently closed path or silently collapse neutral and earth.

Use existing grouping metadata when available; inferred hierarchy belongs to
Tellegen's display model and retains its provenance. Expose search and navigation
through collapsed groups. Selected objects and diagnostic locations should be
revealed or explicitly indicated. Label priorities are selected/focused items,
issues and important equipment before routine labels. A quantitative aggregate
must declare its reduction (for example maximum loading); colours must not
silently average away a violation or imply connectivity.

Expand multi-wire detail for a chosen region on demand while retaining the
surrounding single-line context. Independent view scopes and detail policies
must refer to the same underlying source model. Expanding a transformer must not
force every other transformer in the 100k-bus network to expand.

### Layout at scale

Separate cold layout generation from opening an existing drawing. Persist or
cache deterministic geometry under connectivity, constraint, glyph-metric and
layout-version fingerprints. Show a useful cached or coarse overview first and
refine bounded regions progressively. Keep the previous layout visible while a
replacement job runs. Jobs report progress, support cancellation, and discard
stale results by revision/job ID.

Do not depend on one global layered ELK run for 100k buses. Lay out the region
hierarchy and boundary corridors, then solve local equipment/port constraints.
Use a specialised tree/backbone placement for suitable radial feeders; preserve
and explicitly route ties and meshes. ELK remains a candidate for bounded regions.
Partition size and algorithm choice must be established by measurements, not a
fixed bus threshold assumed in the API. ELK.js supports worker execution, which
keeps the UI responsive but does not reduce the underlying layout complexity.
See the [ELK.js documentation](https://github.com/kieler/elkjs).

Stable anchors and region boundaries reduce movement during expansion and edits.
An operating-value update normally changes appearance only. Switch position may
change connectivity: a topology-derived spanning forest must be invalidated or
explicitly held as a reference layout. The MVLV scenario experiment below rebuilds
the forest offline and can move buses between states.
Topology edits invalidate affected projections/routes plus their dependent
boundary constraints; never assume every edit can be isolated to one region.

### PowerIO / Tellegen runtime responsibilities

PowerIO remains authoritative for equipment, terminals, identities and electrical
interpretation. Expose efficient indexed traversal and detail access without
requiring complete JSON object expansion on every request. The readable contracts
in this document describe meaning; they need not be the in-memory representation
of every rendering stage. A compact transport can intern IDs and carry typed
columns plus source-reference tables. Stable external IDs must not be replaced
by persistent array offsets.

Tellegen owns visual partitions, spatial indexes, detail policies, scene chunks,
label budgets, camera state and bounded GPU/layout caches. The integration layer
owns snapshot/revision consistency across native/WASM/worker boundaries. Prefer
transferable buffers to repeated structured-clone copies. Define ownership of
transferred buffers, and release superseded CPU/GPU resources. Shared memory is
an optional optimisation, not a browser deployment prerequisite.

Extend the proposed projection/layout API before production integration with:

- A scoped projection request and detail policy, not only a whole-model array.
- Stable visual aggregate IDs, source membership and explicit boundary ports.
- Layout job IDs, cancellation, progress and revision-tagged results.
- Scene chunks with bounds, source/picking indexes, geometry revision and level
  of detail; an incremental patch stream for appearance and geometry.
- Independent topology, geometry, appearance and interaction invalidation.

These are planned extensions; the accompanying `tellegen.ts` remains the small
semantic contract and does not yet implement chunk transport or worker jobs.
Camera changes must not rebuild connectivity or upload unchanged geometry.
Selection changes should touch only highlight state, not the base graph. Study
updates should update appearance attributes without re-running layout.

### Performance targets and evidence

Initial targets on a named reference laptop are approximately 60 Hz navigation
(a roughly 16.7 ms frame budget) and selection feedback within 100 ms. Treat these
as goals to validate, not existing guarantees. Record p50/p95/max frame intervals,
input-to-presentation where available, time to first useful drawing, cold layout,
local relayout, peak memory and CPU–GPU uploads separately. A draw callback is not
a GPU completion or presentation timestamp. Do not report retained typed-array
bytes as total or peak memory. Set cold-layout and total-memory acceptance budgets
after representative measurements, with an explicit hardware/browser profile.

Benchmark 1k, 10k and 100k source buses under the same viewport, pixel ratio,
labels, picking and overlay settings. Include radial, long-chain, meshed,
parallel-circuit, dense-substation and mixed-voltage fixtures; the current gallery
covers repeated radial regions with open ties only. Use three or more measured
runs after warm-up, preserve raw intervals and fixture hashes, and distinguish
first-visit shader/font costs from cached switching. Include repeated size changes,
selection/search, result refresh and local multi-wire expansion in later tests.

The initial gallery deliberately draws all base geometry in two layers: 100,000
buses, 100,999 branches (including 1,000 open ties), and 202,096 precomputed line
segments. It has bounded local labels and indexed selection, but no CPU geometry
culling, semantic aggregation, background preparation or automatic layout.
Measurements from it establish a baseline and cannot certify the full production
renderer. Add production optimisations as separately measurable experiments.

## Layout and rendering

Resolve glyphs and measure labels before requesting layout. Supply explicit
visual ports, bundle width, group bounds and ordering hints to the layout adapter.
ELK is the initial candidate; its IDs and option names do not enter PowerIO or
the stable projection contract.

For multi-wire layout, place equipment and allocate bundle corridors first, then
route conductor lanes with source mappings intact. At bends, branches and phase
permutations, simple polyline offsets are insufficient: lane order and clearances
must be resolved explicitly. Separate parallel circuits into separate corridors.

Junction dots are emitted only where an actual shared connection point branches.
Geometric crossings do not create nodes. Several diagram-local junctions may
represent the same source point; they remain visual occurrences, not new source
network buses.

`DiagramLayout` contains symbol boxes and rotations, port positions, routes and
label positions. `DiagramScene` contains paths, text and hit targets in drawing
coordinates. Scene primitives are derived and disposable. Symbol glyph expansion
and magnetic decoration geometry occur when constructing the scene.

Preserve common equipment anchors when switching view modes, but permit expanded
windings and conductor bundles to need more space. A manual pin is a constraint
on a visual occurrence. If a layout engine cannot satisfy it, report the conflict
and retain the previous layout rather than silently discarding the constraint.

The scene builder also receives resolved equipment state and study overlays by
source reference. Availability, open poles and unknown states affect styling
without deriving new electrical facts in the renderer. No colour or line style
alone changes connectivity.

## Persistence and invalidation

Keep portable drawing documents in PowerIO's display boundary. Extend the existing
drawing representation with source-addressed equipment parts, visual occurrence
IDs, symbol orientation, routes and label placements as needed. The exact drawing
serialization is a follow-on design; these connectivity types do not replace
`GeoLayer` or encode geometry in network buses.

Tellegen persists camera and interaction state separately. A portable drawing
may carry a declarative view mode and detail intent; ELK internals and routing
caches are disposable application data.

| Change | Required work |
| --- | --- |
| Result values or sensitivity preview | Update overlays; retain geometry |
| Open/closed or in-service state | Refresh semantic state and styling; retain placement by default |
| Terminal mapping or equipment structure | Rebuild affected projection, validate saved anchors and reroute |
| View mode or detail policy | Reproject; reuse compatible equipment anchors |
| Glyph dimensions, font metrics or labels | Re-measure affected geometry and reroute as needed |
| Pan or zoom | Update camera and visibility only |

A projection/layout fingerprint includes the relevant connectivity, detail
policy and glyph/measurement versions. Do not use the complete model revision as
the sole geometry cache key: operating-value edits would cause needless relayout.
Still require the current revision when binding results or applying edits.

Keep IDs stable across value and state changes. When an importer has no stable
part identifiers, assign documented persistent IDs during normalization or mark
those references revision-scoped. Never silently persist raw table offsets as
stable identity. Reimporting a renamed or structurally changed device requires
explicit reconciliation; saved geometry must not silently attach to another row.

## Validation and acceptance criteria

PowerIO projection validation must check identity uniqueness, reference
resolution, pin/end membership, conductor endpoint consistency, equipment-local
contacts, grounding references and compatibility of declared configurations with
explicit endpoints. Unknown or incomplete source detail produces a diagnostic
and an opaque/aggregate representation, not silently truncated terminal pairs.

Tellegen validation must check visual ID uniqueness, group references, port
ownership, valid source references, and that each connection's semantic endpoints
are contained in its visual port memberships. Attachment point sets must match.
Magnetic decorations must never enter the wire graph. A complete multi-wire
projection must account for every in-scope conductor, pole, coil, element and
grounding relation, either visibly or through an explicit detail notice.

Acceptance cases should include:

1. The supplied feeder: exactly four L1 and two L2 conductors; only S.N has an
   explicit ground bond; T1 has six coils and no interwinding wire.
2. A phase permutation and an A–C lateral: membership follows source mappings,
   not position or colour.
3. A centre-tapped transformer: both coil halves share the declared centre point
   without inventing a third phase.
4. An open tie and independent switch poles: state is visible while endpoints and
   placement remain stable.
5. Parallel circuits and a three-winding transformer: circuit IDs remain distinct;
   the transformer remains one source equipment, not a clique of pairwise devices.
6. An impedance-grounded neutral and a normalized implicit reference: neither is
   drawn as an undeclared ideal neutral bond or physical wire.
7. A balanced case and an opaque multiport device: unavailable multi-wire detail
   is visible and no explicit phases are fabricated.
8. Reordering source tables and editing load values: identity and compatible
   saved geometry remain stable.

Rendering checks then cover junction dots, nonconnecting crossings, readable
labels, isolated/offline equipment, selection round trips and deterministic
layout for a fixed input and layout configuration. A small layout prototype must
verify port constraints and incremental placement before those are promised as
product features.

## Routing experiment: bus fan-out and corridors

The gallery now separates placement (`schematic` / `tidy-tree`) from routing
(`current` / `corridor`). Tellegen owns the bus display footprint, branch display
ports, corridor allocation and geometry. PowerIO remains responsible for original
bus/equipment identity, connectivity, terminal mappings and state. A display port
on a busbar is not a new electrical terminal or node.

The corridor strategy reserves space for explicit bus-owned fan-out bars, uses
separate up/down tree-fan and mesh lane banks, and allocates each tree fan as a
group to preserve sibling ordering when lanes are reused. Congested corridors
widen; crowded busbar ranks can expand component height. Non-tree, parallel,
self-loop and disabled connections retain their endpoint identities. Exterior
tracks support links spanning components or nonadjacent depth columns.

All geometry is precomputed. Rendering adds one binary busbar layer to the existing
bus/wire layers, and picking a bar resolves to its original bus. Validation now
checks branch endpoint ports against their owning bars and enabled-route continuity.
The original routing remains available for comparison, and source-coordinate
visualization stays independent.

On the French feeder and combined SAPN collection, the experiment eliminates
measured exact collinear branch overlap. It does not eliminate mesh crossings:
SAPN retains 7,799 proper route crossings. Its wire segment count increases 18.2%
and geometry grows from 10.47 to 16.29 MiB. See the [routing comparison and limits](performance/README.md#fan-out-and-corridor-routing) for reproducible metrics.
Future multi-wire projection must replace these branch display ports with measured
terminal/conductor footprints derived from the semantic projection; it must not
interpret this fan-out as conductor continuity through transformer windings.

## Implementation sequence

1. Add PowerIO identity and semantic accessor support, including terminal-role
   provenance and connection elaboration. Reuse source-model conventions.
2. Add `ConnectivityView` and validation alongside `DistGraph`, with the feeder
   and focused edge-case fixtures. Preserve the canonical source module.
3. Export a versioned schema and generate the browser contract through
   `@tellegen/engine`; expose the projection through the WASM/native boundary.
4. Implement pure Tellegen single-line and multi-wire projection functions.
   First reproduce the hand-authored fixture outputs without automatic layout.
5. Establish the 1k/10k/100k precomputed-layout GPU baseline before broadening
   glyph coverage. Add measured glyphs, a backend-independent scene, GPU rendering
   and scoped SVG export. Benchmark layout independently, then introduce hierarchy,
   chunking and incremental updates. Reuse existing interaction and study integration.
6. Specify portable drawing extensions and geometry reconciliation, then broaden
   equipment coverage and layout quality.

The first review gate is semantic: both views must resolve to the same source
relationships. Layout quality is the next gate, after the projection can show
the feeder without fabricated wires, terminals or connections.

## Non-tree circuit inspection experiment

Structural readability also needs interaction: rerouting alone does not remove
all mesh crossings. On combined SAPN the corridor geometry has 7,799 proper route
crossings: zero tree/tree, 7,529 tree/non-tree and 270 non-tree/non-tree. The gallery
therefore adds a focused circuit view before attempting global crossing reduction.

Tellegen owns the inspection state, the chosen display spanning forest, the
endpoint-to-endpoint tree path and its overlays. PowerIO continues to own exact
source identities, terminal maps and equipment state. A non-tree role is never an
open/closed or energized/de-energized classification. Transformer associations in
a highlighted cycle do not assert conductor continuity between windings.

The interaction combines a bounded, searchable non-tree connection selector,
source-preserving inspector, A/B endpoint markers, a selected-route halo, optional
connecting tree-path and busbar highlights, context dimming and separate fits for
the connection or complete circuit. The path stops at the common ancestor and
does not highlight an unrelated shared route to the root. Source-coordinate views
have no display-tree inspection; the schematic modes share the same forest and
retain selected equipment when switching geometry.

All base geometry stays resident. The connection index is prepared once; route
segments come from the existing branch segment index. Path construction and cached
overlays cost O(path length), and dimming changes layer opacity. A constant number
of overlay layers avoids per-equipment layer growth. No layout or global crossing
computation runs on selection or camera movement. Full-path rendering still scales
with path length; aggregation of very long selected paths remains future work.

Combined SAPN has 2,655 eligible connections; the longest connecting path has 145
tree branches. All paths were verified against source endpoint indices and the
spanning forest. See [inspection implementation and measurements](performance/README.md#non-tree-circuit-inspection)
for reproducible tests, browser observations and limits. Crossing-aware local lane
ordering and selected/visible crossing decorations remain separate experiments.


## Crossing-aware corridor ordering experiment

The gallery now offers `crossing-aware` beside `current` and `corridor` routing.
Tellegen preserves the corridor router's bus positions, port identities, source
panels, lane counts and interval-colouring assignments, then permutes physical
lanes. Mesh lanes can move between tree lanes; tree lanes retain their relative
order. This reduces crossings without introducing junctions or changing electrical
connectivity, terminals, operating state or PowerIO types.

For eligible corridors, sorted endpoint rows provide the exact crossing cost for
each relative lane order. A deterministic insertion heuristic accepts only strict
improvements, for at most four alternating passes. A 32-lane cap bounds cost-matrix
work; disabled/gapped and exterior routes fall back to existing corridor ordering.
The renderer receives precomputed geometry in the same binary schema, with the
same segment and layer counts. Circuit inspection remains available in both modes.

Combined SAPN proper crossings fall from 7,799 to 5,139 (34.1%), including tree/mesh
7,529 to 4,967 and mesh/mesh 270 to 172. Tree/tree crossings, collinear overlaps and
wire/busbar interior crossings remain zero. Positions, ports, footprint and the
16.29 MiB geometry size are unchanged. Median placement/routing/packing time rises
from 127.5 to 163.2 ms in the local five-run comparison; the new work is offline.
The French radial geometry is byte-identical between these routing modes.

This is a local heuristic with fixed placement and port ordering, not global
crossing minimization. Remaining crossings still require circuit inspection and
potential later port-order or escape-track experiments. See the
[algorithm, fallbacks and measured evidence](performance/README.md#crossing-aware-lane-ordering).


## Switch-state and mixed MV/LV mesh experiment

The gallery now supports scalar BMOPF switches and the external MVLVmeshed case:
3,409 buses, 3,098 lines, 33 transformer objects and 317 switches. It distinguishes
imported position, effective scenario position and service status, retaining exact
source namespaces and ordered terminal maps. Open switches remain physical
connections in the projection, visible/selectable with a route gap, but never
contribute to the enabled spanning forest or highlighted tree path. This equipment
level experiment does not yet elaborate independent poles or conductor continuity.

The exporter currently emits all switches closed. Two explicit snapshots prevent
that importer behavior from silently defining the experiment: **Documented open
ties** applies 34 opening commands from the external DSS master (6 independent
cycles), while **All switches closed** is the intentional stress scenario (40
cycles). Both remain connected. Raw files, complete provenance and generated
reference/geometry files stay external or ignored; committed evidence has hashes
and aggregate measurements. No state is inferred from equipment names. The
[reproduction guide](performance/README.md#mvlvmeshed-switch-scenarios) records the
conversion procedure and limitations of the reconstructed source model.

This case has no coordinates. PowerIO should preserve that absence; Tellegen can
still produce a topology diagram. The adapter requires explicit opt-in for wholly
coordinate-free intake, preserves null coordinates, and exposes only schematic
layouts. It rejects mixed/incomplete coverage and never synthesizes geographic
positions. Both scenarios retain the same source namespace, even though their
reference buffers and offline layout snapshots differ.

Corridor routing removes thousands of collinear overlaps. Crossing-aware ordering
reduces all-closed proper crossings from 837 to 759 at about 6.2 ms total offline
layout/routing/packing. In the documented state, it reduces 2,089 to 2,077 at about
6.8 ms: open connections trigger conservative ordering fallbacks in 41 corridors.
These totals include inactive routes. Both corridor geometries remain below
0.57 MiB. The result suggests a separate inactive-tie presentation policy, evaluated
without losing discoverability or source identity; it does not justify dropping
open equipment from the model. Circuit inspection remains useful for individual
connections. See [aggregate evidence](performance/evidence/switch-routing-comparison.json).

A state change can alter a BFS forest and move many buses. Production should
separate operating state from layout reference state, offer stable anchors when
comparing scenarios, and invalidate connectivity/path indexes explicitly. PowerIO
owns authoritative pole state and connectivity semantics; Tellegen owns scenario
presentation, drawing policy, cached route geometry and selection reconciliation.
The gallery's scalar open/closed contract is deliberately narrower than the
proposed PowerIO per-pole types above.


## Inactive-tie presentation experiment

The performance gallery now separates open-switch visibility from full-route
visibility. Its default **Endpoint rings** mode replaces long open routes with
hollow amber endpoint annotations, grouped by actual bus identity. A bounded,
keyboard-accessible switch browser retains every equipment identity. Selecting a
tie restores its complete route, existing gap and A / B labels. Selecting an
endpoint annotation scopes the list to that bus. Full routes remains available at
the same camera, positions, ports and topology. Open switches never become enabled
forest edges or circuit paths through this interaction.

This belongs to Tellegen's presentation policy, not PowerIO's electrical model.
PowerIO still supplies state, endpoints and stable identities. Tellegen derives
annotation groups, cached draw buffers, pick remapping and selection overlays.
The gallery operates at equipment-level scalar switch state; a later multi-wire
view must apply equivalent presentation rules to authoritative per-pole states.
The policy targets explicitly open switches, retaining closed-but-out-of-service
and disabled non-switch routes separately.

On MVLVmeshed's documented state, 34 open switches become 68 endpoint annotations.
With crossing-aware routes, visible route crossings fall from 2,077 to 23 before
selection; the selected tie restores at most six segments. This removes clutter
by changing what is drawn, not by improving topology or route ordering. All-closed
is unchanged. The compact cached arrays add 0.31 MiB; data preparation is measured
outside interaction, and visibility toggles reuse both resident wire layers.
No-open scenarios, including combined SAPN, have a zero-copy path. A 100k-bus test
checks partitioning, bounded storage and original equipment picking indices.

See the [implementation guide](performance/README.md#compact-presentation-of-open-switches)
and [aggregate comparison](performance/evidence/open-ties-comparison.json).
Remaining limitations are explicit: open-route corridor space is still reserved,
endpoint annotations may overlap at distant zoom, and changing electrical scenarios
can still move buses. Those require independent layout-stability and screen-space
annotation policies, with the source topology and switch browser kept authoritative.


## Source-owned equipment symbols in schematic networks

The performance gallery now integrates equipment symbols through a separate
**Equipment + clearance** routing option, available for both schematic placements.
It combines crossing-aware corridor ordering with explicit device envelopes and
bus attachment slots. Current, corridor and crossing-aware routing remain useful
baselines. Source-coordinate visualization is unchanged.

**Ownership boundary.** PowerIO remains authoritative for equipment identities,
terminal maps, winding associations, switch state and service status. Tellegen
owns compact visual templates, glyph bounds, attachment order, route gaps,
binary draw buffers and picking. The POC changes no PowerIO schema and introduces
no synthetic electrical buses. Its numeric glyph owner is a branch index or
`branchCount + attachmentIndex` into the unchanged BMOPF reference sidecar. Public
selection retains the full source identity, including transformer kind. The future
production projection should use the proposed stable source references above;
these packed indices are transport/rendering details scoped to one fixture.

**Visual semantics.** Transformers use separated winding circles; switches use
contacts with an open/closed blade; loads use an equipment arrow; supplies use a
circle and wave. General shunts use a Y block. In particular, MVLV neutral
conductance shunts must not become capacitors just because the specimen sheet has
a capacitive example. Attachments connect to their owning busbar at equipment
level; their exact terminal maps remain inspectable. They do not establish an
implicit neutral or earth. No generator or detailed equivalent is invented from
an unsupported source table. The π-line specimen remains an inspection model,
not the default line symbol. A multi-wire projection will require authoritative
per-terminal connections and coupled models, independently of these glyphs.

**Geometry contract.** Stage 32 × 32-unit device envelopes before allocating
corridors. Inline transformers and switches reserve 68-unit leads; their wire
segments stop at actual symbol terminals. Attachments reserve bus-owned vertical
slots and stems. A monotone local expansion opens only constrained row gaps while
preserving row order/alignment across depths, then packs component/source panels
with equipment extents. This avoids the pathological global scaling caused by a
single very small tidy-tree gap. Geometry is deterministic and preprocessing stays
O((V + E + A) log(V + E + A)), with bounded glyph tessellation and linear storage;
A is attachment count. Bus centres move, while electrical endpoints and the
display forest stay unchanged. Layout should therefore be treated as a separate
cached variant, not an overlay that can be applied to arbitrary old coordinates.

**Interaction and rendering.** Binary deck.gl layers batch strokes, attachment
junctions and pick targets. A small selection overlay and exact source-ID search
cover both inline and attached devices. Compact open-tie mode suppresses each
open body with its route, retaining endpoint annotations and the switch browser;
selection restores both. Closed-but-out-of-service devices remain distinguishable.
No-open datasets reuse full geometry. There is no per-device DOM, per-frame layout
or global glyph scan for selection labels. Bounds are geometric, not masks that
hide unrelated routes.

**Evidence and limits.** Both MVLV switch scenarios (2,894 symbols each) and the
98,633-bus combined SAPN collection (80,123 symbols) pass unchanged source-reference,
endpoint/state/forest checks and an offline reserved-box clearance audit. The
measured SAPN geometry increases from 16.3 to 38.6 MiB; median layout/packing is
about 266 ms versus 216 ms in this local five-sample run. Symbol clearance expands
SAPN height about 4.9× and MVLV about 4.3×. This is a readable local-detail baseline,
not the final overview density policy. Symbols are all submitted at every zoom;
GPU copies, JS object overhead, label collisions and screen-space crowding remain
outside the packed geometry metric. Future work should evaluate adaptive port
spacing, glyph/label LOD and bounded equivalent/multi-wire inspection. See the
[implementation and measurements](performance/README.md#equipment-symbols-and-reserved-routing-space)
and [reproducible aggregate audit](performance/evidence/equipment-comparison.json).


The visible browser comparison measured SAPN p95 camera intervals of 10.6 ms with
equipment versus 10.4 ms without, and MVLV documented 10.2 versus 10.1 ms. MVLV
all-closed equipment measured 10.3 ms. These are single six-second samples with
automatic labels and wire picking off, at DPR 2; equipment picking stays enabled.
SAPN owned CPU arrays total 44.18 MiB; observed load-to-first-draw was 1.17 s.
Browser preparation, identity indexes and source parsing remain separate costs
from offline routing. See [browser evidence](performance/evidence/equipment-browser-comparison.json).

## Results overlays

The result overlay uses Tellegen's existing deck.gl layers and saved Tellegen
multiconductor power-flow results. Its design follows these requirements:

- Separate electrical model identity, layout revision and result revision. A
  geometry-only change must retain a valid solved snapshot. Electrical changes
  must invalidate its association, and stale results must never silently remain
  coloured as current. Hashes of source inputs and result payloads enforce this
  in the POC; editing and solver sessions remain outside this gallery.
- Read P and Q independently, including their signs at **both** branch ends.
  Shape and lane position supplement colour, and static markers remain useful
  when motion is disabled. Animation is an optional direction and magnitude cue, not physical
  propagation. The engineering-workflow section below specifies its fixed scales.
- Keep normal operation quiet and make limits explicit. Compare full-precision
  values before formatting, and retain exact status, units, reference terminals
  and rating basis in the inspector. A solver's convergence/validity band is not
  a substitute for the model's declared operating limits.
- Keep selection and diagram results keyed to the same source identity. Future
  tables and exports should consume the same validated result projection.
- Use one scene contract for screen and export, and score label candidates
  against equipment, routes and other labels. These remain follow-ups: this
  experiment adds bounded annotations, not a general label-placement optimizer
  or a full-scene export implementation.

### Electrical boundary and result projection

PowerIO owns typed equipment identities, terminals, parameter/rating semantics
and portable solutions. Tellegen's solver produces physical terminal voltages,
port currents and powers. Tellegen's diagram projection binds those values to
source identities and chooses display aggregation, units, limits, legends and
annotation budgets. Layout roots and tree directions never determine flow signs.
No PowerIO schema change or new electrical solve is required for this experiment.
The [proposed TypeScript contract](tellegen.ts) now includes a separate result
snapshot; it does not put display colours or arrows in PowerIO.

The French fixture uses the external saved `McPfResult` for
`75_MVFeeder0100.bmopf.json` (540 buses, 539 branches). Staging verifies the saved
validation summary's input SHA-256 against both the raw model and staged source
identity, requires a passed/converged result, and records solver revision,
options, binary/result hashes, timestamp, iterations and residual. The recorded
solver build had local changes; its revision alone is not a reproducible build
identity. Raw models and full solver dumps remain external. Derived result
sidecars remain ignored local data, just like geometry. Browser binding verifies
sidecar hashes and source keys; rejected/missing results leave topology usable
and show an explicit status instead of falling back to invented values.

**P/Q.** Port powers are summed across every terminal at each end, including
neutral power, and converted from W/var to kW/kvar. Positive means power **into
the equipment at that end**. A positive `from` value follows source-from →
source-to route order; a positive `to` value opposes that order. P and Q can point
in different directions, and end totals need not be negatives because of losses
and shunts. Zero/absent values and inactive branches have no direction marker.
The POC uses a filled triangle for P and an open chevron for Q in offset lanes.
There is no inference of current direction or conductor connectivity from these
aggregate powers; a multi-wire view must retain per-terminal quantities.

**Voltage.** At explicit-neutral French buses, compute `|V_phase − V_neutral|`
and compare with the declared `vpn_min/max`. At the three-phase MV buses, compare
complex phase-to-phase differences against their uniform `vpp_min/max`. A missing
neutral is not silently replaced by earth. No nominal base is guessed from
setpoints or limit midpoints. Nonuniform phase-pair bounds require an explicit
pair-order mapping and are rejected by this narrow fixture adapter. Values are
shown in V with the terminal pair and bounds. This saved solve converged in 16
iterations, but one bus violates its declared input voltage limit despite the
solver's wider validity band passing. The UI can focus that violation directly.

**Loading.** For lines, use the maximum conductor `|I| / i_max` over both ends,
including the neutral when rated. Missing/invalid ratings produce unavailable
loading. For transformers, show the maximum end `|sum S| / s_rating`, explicitly
labelled **aggregate MVA, not worst winding/coil**. This is not a complete
unbalanced thermal check; per-winding current/coil limits are future work.
Amber begins at 90%; red means strictly above 100%. Those visual warning choices
are separate from the input constraints. French maximum line loading is 94.23642%.

### Rendering and scale experiment

The optional controls select topology, voltage-limit or loading colours, plus
P/Q off, static or moving. Colour arrays are prepared once per result/layout.
They reuse resident geometry and do not rebuild routing or mutate equipment.
Result state is independent of paper/dark theme. Selected end values remain
available in the inspector; local selected-branch labels accompany the arrows.
Voltage/loading violations also receive bounded `!` annotations at detail zoom.

Flow annotations use a spatial index of source-ordered first/last route segments.
They are local end indicators, not particles traversing an entire polyline or
crossing a transformer body. A camera query visits at most 4,096 candidates,
retains at most 256 end anchors and emits at most 512 P/Q glyphs. Overview zoom
suppresses them, as do segments shorter than 28 screen pixels. Moving glyphs
update only this bounded set. Static colours stay resident; animation pauses in
a hidden tab or when no local markers are visible, and initially respects reduced
motion. The normal topology, wire picking and source-owned equipment are retained.
Dense/screen-space label collisions and omitted end markers remain limitations;
the inspector is authoritative, and overview does not claim complete flow coverage.

There is no saved SAPN solution in the supplied external results. Its scaling
fixture therefore uses **deliberately artificial values on the real 98,633-bus,
101,283-branch topology**, with prominent status/watermark/inspector labels. It
exercises mixed signs and warning/violation paths; it makes no claim about SAPN
operation, conservation or electrical feasibility. This is a rendering benchmark.
The French view contains real saved solved values; stress data is never substituted
for a missing French result.

The [offline measurements](performance/evidence/results-comparison.json) separate
JSON parsing, identity binding, colour/index preparation, camera queries and
bounded glyph generation. The [browser comparison](performance/evidence/results-browser-comparison.json)
records the same camera path with overlays off, static and animated. Resident
results are retained even for the overlays-off sample: it measures incremental
drawing overhead, not a memory or cold-start baseline without results.

A production implementation should pack result scalars/indices in typed arrays,
parse and validate in a worker, fetch snapshots on demand, and query selected
terminal detail lazily. The SAPN POC sidecar is 34.61 MiB of uncompressed JSON;
its JS objects and spatial index are a larger concern than its 1.54 MiB of added
colour buffers. Snapshots are immutable and layout-independent, but this POC
reparses them on layout reload. Keep binary result streaming, worker preparation,
cache reuse, screen-space collision scoring and multi-wire result inspection as
explicit next experiments before application integration.

The matched-viewport browser samples (692 × 500, DPR 2, dark, equipment routing)
measured French p95 frame intervals of 9.1/9.3/9.1 ms for overlays off, loading with
static P/Q, and loading with moving P/Q. Combined SAPN measured 10.1/10.4/12.8 ms;
voltage with static P/Q measured 9.7 ms. These are single local samples, not GPU
measurements or universal budgets. An early unmatched-viewport baseline is retained
but excluded from the comparison. Sixty-three POC tests pass. See the performance
guide for settings, allocation exclusions, selected-source screenshots and
remaining collision/aggregation limitations.

## Engineering results workflow

The gallery now prioritizes a practical sequence: **follow power → inspect an
end value → locate a limit violation → trace its feeder profile**. Results presets
sit immediately above the diagram. Layout/routing experiments and performance
measurements remain available below the engineering workbench. This supersedes
the initial direction-only animation experiment described above.

### Discoverable controls and inspection

**Power flow** enables P/Q motion and focuses the largest absolute-P branch.
**Voltage** focuses a declared voltage-limit violation when one exists;
**Loading** focuses the highest available loading. **Topology** removes result
colours and markers. P and Q have independent toggles, an explicit pause/resume
button, and ½×/1×/2× playback. Display settings expose manual combinations and
explain conventions. The status retains solver provenance or the artificial-data
warning. Missing snapshots disable result actions; they never manufacture values.

Wire hover/picking is enabled by default. Tooltips show the source namespace and
ID, both signed P/Q end totals, loading and its rating basis, or actual bus
terminal-pair voltages, bounds and violation status. An attachment reports its
connected bus voltage explicitly; it does not claim that bus values are device
power. Clicking pins these details and connects the selection to its voltage
profile. Tooltip placement flips at the viewport edges. Source identity remains
authoritative through layout changes and chart navigation.

### Magnitude-dependent P/Q motion

P triangles and Q chevrons move in independent offset lanes. The sign at each
source-ordered end determines direction; the display spanning tree never does.
Speed now represents magnitude: `max(2, 72 × sqrt(abs(value)/snapshotMaximum))`
CSS pixels per second at 1×, capped at 72. P and Q each use their own maximum
absolute end value over the **whole immutable snapshot**, computed once when
preparing results. The legend shows numeric quarter-maximum and maximum examples
(36 and 72 px/s). P and Q speeds are not directly comparable across their separate
scales. The scale does not renormalize with the viewport. The 2 px/s floor keeps
small nonzero values visible; zero, missing and inactive flows have no marker.

The logical animation clock freezes on pause or in a hidden tab. Playback changes
alter the clock rate, preserving phase. Speed is invariant to zoom and route
segment length; markers wrap within their reserved end windows. This is an
engineering magnitude cue, **not physical propagation or time-varying simulation**.
Reduced-motion preference initially selects static markers. The user can explicitly
start motion. The existing 256-anchor / 512-glyph budget remains; no full-network
per-frame scan, relayout or colour-buffer regeneration is introduced.

### Linked voltage profiles

Select a bus, attachment, or branch to obtain a root-to-bus path in the display
spanning forest. For a branch, use its deeper tree endpoint (source-to endpoint
if depths tie). This is a structural inspection path, **not a claim of unique
power supply through a mesh**. The profile is unavailable in source-coordinate
mode without a display tree. It never joins independent SAPN source panels.

Plot actual volts against branch steps, with one trace and declared lower/upper
bounds per explicit terminal pair. Break into separate plots at transformers and
whenever the terminal-reference set changes. Do not connect MV phase-to-phase and
LV phase-to-neutral samples, infer a nominal per-unit base, replace neutral with
earth, or interpolate through unavailable reference data. Full precision determines
violations; labels show rounded values. The saved French violation is visible on
phase 3–neutral at `75_LVBus0623137`: 207.7769 V against a 207.8461 V minimum.

Hovering/focusing a profile point highlights its bus in the diagram and shows exact
values. Clicking or pressing Enter pins and centers that bus while retaining the
original complete profile. Left/right arrows, a path-bus selector and Previous/Next
provide keyboard navigation. Diagram hover links back to matching profile points.
The path overlay is optional; **Fit path** frames the path. Curve points are source
bus indices bound to validated reference records, not inferred from labels.

Path construction is bounded to 4,096 buses; longer paths receive an explicit
unavailable notice. Charts show at most the last 256 buses, with an omission notice
and the full bounded path still highlighted. No downsampling silently removes a
violation. Small independent SVG charts are built only on selection; deck.gl keeps
the large scene resident. No chart library or additional dependency is introduced. PowerIO and solver schemas are unchanged: these are Tellegen display
and interaction policies over the existing validated result snapshot.

### Evidence and remaining scope

The [workflow CPU guardrail](performance/evidence/engineering-workflow-cpu.json)
samples 1,000 paths/cameras per dataset. On combined SAPN, path preparation p95 was
0.051 ms and SVG-string generation p95 0.247 ms (excluding DOM insertion). The
largest sampled path had 216 buses; local glyph generation p95 was 0.005 ms.
The browser guardrail and reproducible walkthrough are in the
[performance guide](performance/README.md#engineering-workflow).

The French walkthrough uses saved solved values. SAPN remains explicitly labelled
**artificial stress, not a power-flow solution**. These are local guardrails, not
universal performance budgets. Seventy-three tests cover the new speed mappings,
sign/zoom behavior, profile boundaries, missing references, bounded paths, source
identity and safe SVG labels, alongside the existing POC tests.

Remaining engineering work includes per-conductor result inspection in the
multi-wire view, winding/coil thermal limits, coherent scenario/time comparisons,
full-path flow animation, chart/scene export and general label collision handling.
The current animated snapshot does not solve or simulate edits. Application
integration and broader user testing remain separate from this gallery experiment.
