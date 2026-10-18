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
  current narrow viewport without cropping, showing the open blade, diagram title,
  legend and selected equipment ID. The drawing continues off screen.

Screenshots illustrate local readability, not an overview-density or performance
claim. See the adjacent JSON evidence and the [performance guide](../README.md)
for reproducible measurements and limitations. The original
[four-wire reference screenshots](../../gallery/reference-images/) illustrate
both projections with manually authored geometry, independently of BMOPF routing.

## Engineering workflow

- [French Results toolbar and P/Q detail](french-engineering-workflow.png): dark theme, saved solved results, selected line `75_LVBranch0306208`, fixed snapshot speed scales and pause/playback controls. The screenshot frames the Results toolbar and complete canvas; the route continues beyond the local view.
- [Linked MV/LV profile](french-linked-voltage-profile.png): dark theme, 30-bus path to `75_LVBus0623137`; separate MV phase-pair and LV phase-neutral plots, declared limits, and the phase-3 undervoltage. The path selector and exact values remain visible.

These are verified screenshots of the independently implemented local gallery. Still images show the controls and glyph positions, not the animation speed. Raw models and result sidecars remain external/ignored.

## Engineering workbench

Captured at 1440 × 900 (DPR 1) in headless Chromium with software WebGL, dark theme, saved French solution.

- [Worst issue](workbench-worst-issue.png): <kbd>N</kbd> from a fresh load. The KPI strip, ranked issue list, inspector pair bars and linked profile all point at `75_LVBus0623137` (3–n 207.7769 V below its 207.8461 V limit). The profile shades the min–max of all buses at the same depth and reference.
- [Conductor currents](workbench-conductors.png): line `75_LVBranch0306208` restored from a shared URL (`sel=branch:line:french-medium/75_LVBranch0306208`). Per-conductor |I| at both ends on one scale: 91 / 113 / 176 A on the phases and 72 A (41 %) on the neutral.
- [Branch table](workbench-branch-table.png): continuous loading colour, Branches table sorted by loading and linked to the diagram selection.
- [Paper theme, voltage within band](workbench-paper-voltage-band.png): continuous voltage colouring over each bus's declared band (green toward the minimum), with the selected undervoltage bus, its pair bars and the profile envelope.
- [Phone width](workbench-phone.png): 390 × 844 after <kbd>N</kbd>; KPIs wrap to a grid and the panels stack below the diagram, without horizontal scrolling.
