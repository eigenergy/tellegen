---
"@tellegen/svelte": patch
---

Show the case file button on every device. It was hidden whenever the primary
input could not hover, which included touchscreen laptops with a mouse and all
phones, so local files could not be opened there at all. Drag and drop is now
gated on `any-hover`/`any-pointer`, and the button no longer depends on it.
Diagnosed by @MohamedNumair in #110.
