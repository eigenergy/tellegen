---
'@tellegen/engine': patch
---

The AC power flow of an OpenStreetMap grid no longer stops with `BUILD.INSTANCE.VOLTAGE_CONTROL_CONFLICT` when a plant sits at the receiving converter of an HVDC link. The fixed transfer that stands in for the link in validation states the 1.02 pu set point of every generated unit instead of 1.0.
