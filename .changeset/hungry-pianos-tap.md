---
"@tellegen/svelte": patch
---

Recognize a `.pio.json` package by either envelope spelling, the `schema` URL or
`schema_version`, so a study saved by a newer powerio still restores.
`PIO_PACKAGE_SCHEMA_PREFIX` stays exported.
