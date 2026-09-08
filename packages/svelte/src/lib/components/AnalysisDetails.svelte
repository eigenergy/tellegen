<script lang="ts">
  import { getAppState, getController } from "../context.svelte.js";
  import { fmt, formulationLabel, signed, solveMetaLabel } from "../format.js";
  import BindingLines from "./BindingLines.svelte";
  import FormulationSelector from "./FormulationSelector.svelte";

  const app = getAppState();
  const ctrl = getController();
</script>

{#if ctrl.activeSolvable}
  {@const c = ctrl.activeSolvable}
  {@const stats = ctrl.networkStats}
  <div class="analysis-state mono">
    <span class="state" class:running={c.solving}
      >{c.solving ? "solving" : c.solution ? "solved" : "waiting"}</span
    >
    <span>{formulationLabel(c.formulation)}</span>
    {#if c.solving || c.solveMs !== null}<span>{solveMetaLabel(c)}</span>{/if}
  </div>

  <FormulationSelector />

  {#if stats}
    <dl class="mono">
      <div>
        <dt>objective</dt>
        <dd>{stats.objective === null ? "…" : fmt.format(stats.objective)}</dd>
      </div>
      {#if ctrl.isPerturbed(c) && stats.deltaObjective !== null}
        <div class="delta">
          <dt>vs base</dt>
          <dd>{signed(stats.deltaObjective)}</dd>
        </div>
      {/if}
      <div>
        <dt>binding lines</dt>
        <dd>{stats.binding ?? "…"}</dd>
      </div>
    </dl>
  {/if}

  <BindingLines />
{:else if app.activeMulti}
  <p class="empty mono">
    No live analysis. Multiconductor distribution cases are currently viewing
    only.
  </p>
{:else}
  <p class="empty mono">No analysable network is active.</p>
{/if}

<style>
  .analysis-state {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    margin-bottom: 8px;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .state {
    padding: 1px 6px;
    border: 1px solid color-mix(in srgb, var(--data-neg) 55%, var(--line));
    border-radius: 2px;
    color: var(--data-neg);
  }

  .state.running {
    border-color: var(--accent);
    color: var(--text-accent);
  }

  dl {
    margin: 10px 0 0;
    font-size: 12px;
  }

  dl div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 3px 0;
  }

  dt,
  .empty {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    text-align: right;
  }

  .delta dd {
    color: var(--text-accent);
  }

  .empty {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
  }
</style>
