# Changelog

## 0.1.2 — 2026-07-28

`@tellegen/svelte` only. The compact layout renders the control panel as a
bottom sheet over the map; this fixes what that layout got wrong on phones
(#59). `@tellegen/engine` stays at 0.1.1 and the `tellegen` crate is unchanged.

- A resolved selection leads the sheet body, above the case stats,
  formulation, bus lookup, and binding lines that used to push it off screen.
  The body scroll resets on selection, and `BusPicker` blurs its input on
  coarse pointers so the on-screen keyboard stops covering the readout (#59).
- Basemap attribution rides above the sheet, parks before it would reach the
  header, and fades once the sheet covers what it annotates; attribution is a
  licensing requirement for the CARTO and OSM tiles (#59).
- The map zoom buttons are hidden under the compact breakpoint, where the
  solve card covers wherever they land. Pinch and double tap still zoom. They
  stay for coarse pointers above the breakpoint, at 44px (#59).
- Under a short viewport the case tabs stay on the brand's row, halving the
  header, and the `half` snap gives up room there, so a landscape phone keeps
  a usable band of map. The hardcoded header offsets in the map and the sheet
  are replaced by a measured `app.headerInset`, and both the sheet and the
  map's chrome read one `app.viewportHeight` published by the shell (#59).
- Touch targets are 44px on coarse pointers, keyed on the pointer rather than
  the breakpoint. The grab bar rendered 25px, the `full` snap overlapped the
  header, the solve card sat at an offset positioned for the old layout, the
  formulation select ran off a 320px screen, attribution wrapped into a wide
  block over the network, and "esc clear" named a key a phone does not have
  (#59).

## 0.1.1 — 2026-07-21

Multiconductor viewing polish from IEEE 123 feedback: edge selection, a tidy
feeder tree for synthetic layouts, and IEC transformer symbols (#58).

- Multiconductor edges select like buses: the panel expands the edge's kind,
  endpoints, phase count, and per-conductor terminal pairing; bus and edge
  selection are mutually exclusive and Escape clears both (#58).
- Synthetic layouts detect near-tree graphs and draw them as a tidy tree
  rooted at the source bus, so radial feeders read as a trunk with laterals;
  normalization keeps the drawing's aspect ratio for synthetic and
  planar-coordinate cases alike (#58).
- Transformer edges carry the IEC two-circle symbol at their midpoints,
  angled along the edge and tinted by selection (#58).
- The multiconductor panel legend no longer collapses under the global color
  ramp rule, and the powerio parsing footnote is removed (#58).
- `TellegenMap` gains an optional `onmultiedgeclick` prop (#58).

## 0.1.0 — 2026-07-21

First tagged release: the `tellegen` engine crate, the `tellegen-wasm`,
`tellegen-server`, and `tellegen-cli` adapters, and the `@tellegen/engine` and
`@tellegen/svelte` npm packages, all at 0.1.0, on powerio 0.7.1.

- Case interpretation moves to powerio-prob problem instances (#49).
  `DcNetwork`/`AcNetwork` build `DcOpfInstance`/`AcOpfInstance` from the parsed
  network as the single owner of case reading; tellegen keeps formulations and
  solver policy on top (piecewise cost fitting, missing cost policy, angle
  bound normalization, fallback rating synthesis, shed policy, per bus
  aggregation) and its own branch susceptance convention. PGLib snapshot
  objectives are unchanged.
- Studies are powerio packages (#41, #42). `Study::to_package`/`from_package`
  round trip the base network, the edit log (one `StudyCommit` per commit),
  and the formulation and solve options under `study.app["tellegen"]` through
  `.pio.json`; loads fail closed on unknown edit kinds, unrecognized app
  payloads, and unresolved keys. The web app saves a study, restores it from a
  drop (content sniffed package envelope), and exports the committed state
  through the powerio format writers with fidelity warnings surfaced.
- Multiconductor case viewing (#39). OpenDSS `.dss`, BMOPF JSON, PMD JSON, and
  multiconductor `.pio.json` packages parse in the browser through
  powerio-dist and render as a bus level graph with terminal detail: phase and
  neutral badges, ground markers, per conductor strands, attachment badges.
  Viewing only, no solve.
- Geographic sidecars ride powerio's GeoLayer (#43). Parsing moves upstream
  (the tolerant reader runs in wasm); applied coordinates land on the network
  itself (`Bus.location`, `Branch.route`), so saved packages and exports carry
  the placement on screen; branch routes render as polylines; layouts export
  as `.geo.json` with `synthetic`/`manual` provenance; a dropped PowerWorld
  `.pwd` fills a coordinate-less sibling case through the `SubNum` join.
