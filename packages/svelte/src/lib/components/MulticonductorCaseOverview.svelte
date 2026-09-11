<script lang="ts">
  import { formatPowerIoDiagnostic } from "@tellegen/engine";
  import { getAppState, getController } from "../context.svelte.js";
  import { fmt } from "../format.js";

  const app = getAppState();
  const ctrl = getController();
</script>

{#if app.activeMulti}
  {@const mc = app.activeMulti}
  {@const s = mc.summary}
  <h2>{mc.label} <span class="region mono">via {mc.fileName}</span></h2>
  {#if s}
    <p class="tag mono">multiconductor &#8901; viewing only</p>
    <dl class="mono">
      <div>
        <dt>buses</dt>
        <dd>{s.n_bus}</dd>
      </div>
      <div>
        <dt>lines / switches / xfmrs</dt>
        <dd>{s.n_line} / {s.n_switch} / {s.n_transformer}</dd>
      </div>
      <div>
        <dt>load</dt>
        <dd>{fmt.format(s.load_kw)} kW</dd>
      </div>
      <div>
        <dt>gen capacity</dt>
        <dd>{fmt.format(s.gen_kw)} kW</dd>
      </div>
      <div>
        <dt>sources / loads / gens</dt>
        <dd>{s.n_source} / {s.n_load} / {s.n_generator}</dd>
      </div>
      {#if s.n_ibr > 0 || s.n_shunt > 0 || (s.n_capacitor ?? 0) > 0}
        <div>
          <dt>IBRs / shunts / caps</dt>
          <dd>{s.n_ibr} / {s.n_shunt} / {s.n_capacitor ?? 0}</dd>
        </div>
      {/if}
    </dl>
    {#if s.diagnostics.length > 0}
      <ul class="warnings mono">
        {#each s.diagnostics.slice(0, 4) as diagnostic, i (i)}
          <li>{formatPowerIoDiagnostic(diagnostic)}</li>
        {/each}
        {#if s.diagnostics.length > 4}<li>
            +{s.diagnostics.length - 4} more
          </li>{/if}
      </ul>
    {/if}
    <p class="footnote mono">
      {#if !mc.placed}
        {s.coords_kind === "planar"
          ? "diagram coordinates; place the layout on the map"
          : "no coordinates; place the layout on the map"}
      {:else if mc.coordsKind === "geographic"}
        coordinates: geographic, from the case file
      {:else if mc.coordsKind === "planar"}
        coordinates: diagram layout fit where you placed it
      {:else}
        coordinates: synthetic topology layout centered where you placed it
      {/if}
    </p>
  {/if}

  <div class="actions">
    <button class="reset mono" onclick={() => ctrl.moveMultiCase(mc)}
      >{mc.placed ? "move layout" : "place on map"}</button
    >
    <button class="reset mono" onclick={() => ctrl.removeMultiCase(mc)}
      >remove</button
    >
  </div>
{/if}

<style>
  h2 {
    margin: 0 0 8px;
    font-size: 16px;
    font-weight: 600;
  }
  .region {
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 400;
  }
  .tag {
    margin: 0 0 10px;
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: uppercase;
  }
  dl {
    margin: 0;
    font-size: 12px;
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 3px 0;
  }
  dt {
    color: var(--text-secondary);
  }
  dd {
    margin: 0;
    text-align: right;
    white-space: nowrap;
  }
  .warnings {
    margin: 8px 0 0;
    padding: 0;
    color: var(--text-accent);
    font-size: 10.5px;
    line-height: 1.5;
    list-style: none;
  }
  .footnote {
    margin: 8px 0 0;
    color: var(--text-tertiary);
    font-size: 10px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }
</style>
