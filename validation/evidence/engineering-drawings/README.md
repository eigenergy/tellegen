# Engineering drawing checks

Measured on 2026-11-16 in headless Chromium on the development Apple Silicon host. These are local observations, not cross-machine performance guarantees. Each fixture has 1,000 buses; the radial case has 999 branches and the meshed case adds 968 cross-links. Initial rendering includes page navigation. Other timings cover the individual browser action through its visible outcome.

Run `TELLEGEN_PREVIEW_PORT=4373 npm --workspace tellegen-frontend run test:browser -- tests/engineering-drawing.spec.ts --workers=1` after building the engine, Svelte package and web app. The tests attach separate rendering, selection, editing, layout, result display and SVG export measurements.

The substation test imports the licensed handwritten CGMES node-breaker fixture in `tests/data/node-breaker`, checks retained disconnectors, edits a busbar, exercises undo/redo, saves, reloads and checks SVG output. Native tests check detailed equipment projection and HTTP serialization; public Rust API compatibility was checked with cargo-semver-checks against main: 229 checks passed, 31 not applicable.

CGMES Diagram Layout records currently remain unsupported metadata in PowerIO. Node-breaker geometry is generated from retained equipment and connectivity. Source bus coordinates and routes supported by the reader are preserved as the starting layout. The editor does not claim to reproduce original CGMES Diagram Layout geometry.

Desktop (1280 by 900) and touch-enabled phone (390 by 844) screenshots cover the detailed substation and small radial/meshed cases. The phone page declares a device-width viewport and its controls collapse without document overflow. Browser tests also reject invalid numeric edits, preserve live switch-state changes without discarding manual positions, and check that full SVG exports contain rotated frames, offset labels and route bends independently of interactive zoom.
