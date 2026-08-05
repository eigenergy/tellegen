---
"@tellegen/svelte": patch
---

Recognize a `.pio.json` package by either envelope spelling. The classifier keyed
on the `schema` URL field alone, which is one of four version identifiers powerio
is collapsing into a single `schema_version`; a saved study written after that
change would have fallen through to the distribution reader and failed with a
parse error. Both spellings now classify, and `PIO_PACKAGE_SCHEMA_PREFIX` stays
exported.
