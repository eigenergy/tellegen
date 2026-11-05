---
"@tellegen/engine": minor
"@tellegen/svelte": patch
---

Balanced network payloads carry what a transmission diagram needs. Each bus has
its nominal voltage (`base_kv`) and a `substation` index into the new
`Network.substations` list; each branch says whether it is a `transformer`.
The same fields appear on ingested local cases (`IngestedCase.view` and
`Topology`) and survive synthetic placement.

The engine groups buses into substations from the strongest evidence the case
offers: substations declared by the source (PowerIO voltage levels, or a
PowerWorld `SubNum`), buses that shared one coordinate before display spreading,
then the two ends of a transformer when they are not far apart. Each substation
records which of these grouped it (`source`), its distinct voltages (`kv`) and a
stable `key`. ACTIVSg200 from its aux file gives its 111 declared substations;
grouping 70,000 buses takes about 13 ms natively. A branch counts as a
transformer when PowerIO says so or when its ends have different nominal
voltages. All new fields are optional in the types, so older payloads still
parse.
