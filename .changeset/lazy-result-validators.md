---
'@tellegen/engine': minor
'@tellegen/svelte': minor
---

The package root no longer loads the generated result validators, which cuts its static import graph from about 505 kB to 62 kB gzipped. The validators are split by result family, and each family loads the first time one of its results is checked.

- `parseApiResult`, `validateApiResult` and `unpackRun` return promises. Add `await` where you call them.
- `preloadValidators(names)` loads the validators for those results ahead of their first use.
- `@tellegen/engine/validators` exports `parseApiResultSync` and `validateApiResultSync` for a host that cannot await validation. That entry imports every validator.

`@tellegen/svelte` builds on this engine. Its HTTP client still handles solve stream events in the order they arrive and closes the stream as soon as it ends.
