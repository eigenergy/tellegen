# Changelog

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
