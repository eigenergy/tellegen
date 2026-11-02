---
"@tellegen/engine": minor
"@tellegen/svelte": minor
"@tellegen/webmcp": minor
---

Recognize multiconductor entries in the hosted case catalogue and load their
portable PowerIO modules into the distribution viewer and browser AC power-flow
workflow, with lazy loading, browser-local removal, and restore support.

Hosted cases can supply source/association metadata and PF defaults. Preserve
those defaults across UI/WebMCP solves and saved-study reopen; compact
multiconductor exports keep large studies within the replay input limit.

Cancel hosted requests on controller disposal, discard late ingest completions,
and report missing backends without starting a request.

Keep hosted inputs parsed in the engine and initialize the shared solver settings
from catalogue defaults, including saved-study replay. Free retained inputs when
hosted requests are superseded or the catalogue is reloaded.
