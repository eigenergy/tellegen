---
"@tellegen/svelte": minor
---

The terminal results find a bus or a piece of equipment with the same search as the overview, instead of a `<select>` holding every bus (100,000 options on a large feeder) and another holding every piece of equipment. `McResults` takes the case's index as `searchIndex` and builds one from `graph` when it is left out. The two loss figures now have distinct names, "Passive loss" in the results and "Branch losses" in the overview, and the results state the sign convention once: source power positive into the network, equipment P and Q positive into the equipment.
