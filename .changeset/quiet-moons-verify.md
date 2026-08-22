---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

Retire the wasm instance after a trap instead of serving the next request from
it, bound the string entry points at the same 128 MiB limit as their byte
counterparts, frame a single-point selection instead of clamping to maximum
zoom, and keep the parsing indicator up while dropped JSON cases materialize.
