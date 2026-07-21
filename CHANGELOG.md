# Changelog

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
