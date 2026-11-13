---
"@tellegen/svelte": minor
---

Show voltage angles in degrees in the balanced views too: the power flow inspector labels its angle `Voltage angle (°)` and the diagram's bus tooltip reads `-2.26°` instead of radians, both through the shared formatter. The overview's phase bars show per-unit beside volts (`221.7 V (0.9606 pu)`) when the bus's galvanic zone declares a winding or source rating, with phase–neutral and phase–ground pairs on the phase base; a value far from 1 pu, where the rating is not the pair's base, shows volts alone.
