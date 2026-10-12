# Equipment screenshots for design discussion

These screenshots show the local performance gallery on the documented MVLVmeshed
scenario, using tidy-tree placement, equipment-clearance routing and compact open
switch presentation. They contain rendered views only; raw BMOPF files stay external.

- [Load and general shunt](mvlv-equipment-detail.png): paper theme, selected
  `ld1000_load_a` attached to bus `b1085`. Its shunt uses a general Y block; no
  capacitive subtype or earth bond is inferred. Captured at a 1280-pixel viewport.
- [Selected open switch](mvlv-open-switch.png): paper theme, selected
  `switch_1059_open`. Compact mode restores this switch's route and blade gap;
  other open routes remain represented by endpoint annotations. Captured at the
  current narrow viewport and cropped to the diagram section including its title
  and legend. The drawing continues off screen.

Screenshots illustrate local readability, not an overview-density or performance
claim. See the adjacent JSON evidence and the [performance guide](../README.md)
for reproducible measurements and limitations. The original
[four-wire reference screenshots](../../gallery/reference-images/) illustrate
both projections with manually authored geometry, independently of BMOPF routing.
