<script lang="ts">
  import { getAppState, getController } from "../context.svelte.js";
  import DisplayControls from "./DisplayControls.svelte";
  import MulticonductorLegend from "./MulticonductorLegend.svelte";
  import SizeLegend from "./SizeLegend.svelte";

  const app = getAppState();
  const ctrl = getController();
</script>

{#if ctrl.activeSolvable}
  <DisplayControls />
  <div class="size-section"><SizeLegend /></div>
{:else if app.activeMulti}
  <MulticonductorLegend />
  <div class="size-section"><SizeLegend unit="kW" /></div>
{:else}
  <p class="empty mono">No map display is active.</p>
{/if}

<style>
  .size-section {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .empty {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>
