---
"@tellegen/engine": patch
---

Grow the engine's wasm memory in larger steps. The allocator used to grow
linear memory one 64 KiB page at a time, and on a 57,757-bus distribution case
those tens of thousands of grows took most of the ingest. Ingesting that case
in the worker is now about three times faster. Linear memory, which never
shrinks, can end a few percent higher than before after a large ingest.
