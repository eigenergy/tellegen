---
"@tellegen/svelte": patch
---

The multiconductor map hands its transformer symbol to deck.gl as a prepacked
icon atlas, so nothing is fetched. It was a `data:` URL that deck.gl fetched,
and a Content Security Policy whose `connect-src` leaves out `data:` blocked it,
so the map drew without symbols.
