<script lang="ts">
  import type { DistAttachmentKind } from "@tellegen/engine";
  import type { RGBA } from "../colors.js";
  import { rgbaCss } from "../format.js";
  import {
    attachmentColor,
    attachmentGlyph,
    phaseColor,
  } from "../multiconductor.js";

  const ATTACHMENTS: DistAttachmentKind[] = [
    "source",
    "generator",
    "ibr",
    "load",
    "shunt",
  ];
  const PHASES: [string, RGBA][] = [
    ["a", phaseColor("1")],
    ["b", phaseColor("2")],
    ["c", phaseColor("3")],
    ["n", [120, 114, 102, 255]],
  ];
</script>

<div class="legend mono">
  <div>
    <span>conductors</span>
    {#each PHASES as [label, color] (label)}
      <i style={`--swatch:${rgbaCss(color)}`}></i>{label}
    {/each}
  </div>
  <div>
    <span>attachments</span>
    {#each ATTACHMENTS as kind (kind)}
      <i style={`--swatch:${rgbaCss([...attachmentColor(kind)])}`}
      ></i>{attachmentGlyph(kind)}
    {/each}
  </div>
</div>

<style>
  .legend {
    display: flex;
    flex-direction: column;
    gap: 7px;
    color: var(--text-secondary);
    font-size: 9.5px;
  }
  .legend div {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
  }
  .legend span {
    flex: 0 0 78px;
  }
  i {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-left: 4px;
    border-radius: 2px;
    background: var(--swatch);
  }
</style>
