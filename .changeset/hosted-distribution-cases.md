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
