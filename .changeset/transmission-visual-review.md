---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

Keep transmission stress layouts finite when the case has one substation. Normalize routed
branches to endpoint order so flow arrows and transformer winding colours stay correct even
where a route doubles back, and expose `route_iterations` in `TxLayoutOptions`.

Transmission conductors and busbars now use dark centre strokes with colour halos for voltage
or loading. Loads use larger filled demand arrows from the shared multiconductor symbol set;
loads and generators can be selected directly. The loading key matches the branch thresholds.
Touch controls have larger targets, explicit zoom buttons, and a wrapping toolbar whose results
key remains clear of its controls.
