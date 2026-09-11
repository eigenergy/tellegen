<script lang="ts">
  import { getAppState, getController } from "../context.svelte.js";
  import {
    distributionBusDetails,
    distributionEdgeDetails,
    transmissionBranchDetails,
    transmissionBusDetails,
    type ElementModelDetails,
    type InspectorRecord,
  } from "../element-inspector.js";
  import DemandSlider from "./DemandSlider.svelte";
  import PropertyList from "./PropertyList.svelte";
  import RatingSlider from "./RatingSlider.svelte";
  import SensitivityReadout from "./SensitivityReadout.svelte";
  import TopMovers from "./TopMovers.svelte";

  const app = getAppState();
  const ctrl = getController();

  const details = $derived.by((): ElementModelDetails | null => {
    const mc = app.activeMulti;
    if (mc?.networkDetails && mc.selectedBusId !== null) {
      return distributionBusDetails(mc.networkDetails, mc.selectedBusId);
    }
    if (mc?.networkDetails && mc.selectedEdge) {
      return distributionEdgeDetails(
        mc.networkDetails,
        mc.selectedEdge.kind,
        mc.selectedEdge.id,
      );
    }
    const active = ctrl.activeSolvable;
    if (active && ctrl.selectedBusData) {
      return transmissionBusDetails(
        active.studyInputJson,
        ctrl.selectedBusData,
      );
    }
    if (active && ctrl.selectedBranchData) {
      return transmissionBranchDetails(
        active.studyInputJson,
        ctrl.selectedBranchData,
      );
    }
    return null;
  });

  const selected = $derived(
    app.selectedBus !== null ||
      app.selectedBranch !== null ||
      app.activeMulti?.selectedBusId != null ||
      app.activeMulti?.selectedEdgeId != null,
  );

  const liveOutput = $derived.by((): InspectorRecord | null => {
    const c = ctrl.activeSolvable;
    if (!c?.solution) return null;
    if (ctrl.selectedBusData) {
      const bus = ctrl.selectedBusData;
      const lmp = c.solution.prices.find(
        (entry) => entry.bus === bus.id,
      )?.value;
      const angle = c.solution.va.find((entry) => entry.bus === bus.id)?.value;
      const w = c.solution.w.find((entry) => entry.bus === bus.id)?.value;
      return {
        analysis: c.formulation.toUpperCase(),
        status: c.solving ? "updating" : "current",
        demand_mw: bus.demand_mw + ctrl.committedDelta,
        generation_mw: bus.gen_mw,
        ...(lmp === undefined ? {} : { lmp: lmp }),
        ...(angle === undefined ? {} : { voltage_angle: angle }),
        ...(w === undefined
          ? {}
          : { voltage_magnitude_pu: Math.sqrt(Math.max(w, 0)) }),
      };
    }
    if (ctrl.selectedBranchData) {
      const branch = ctrl.selectedBranchData;
      const flow = c.solution.flows.find((entry) => entry.branch === branch.id);
      return {
        analysis: c.formulation.toUpperCase(),
        status: c.solving ? "updating" : "current",
        flow_mw: flow?.mw ?? null,
        loading_percent: flow ? flow.loading * 100 : null,
        binding: flow ? flow.loading >= 0.999 : null,
      };
    }
    return null;
  });

  const scenarioInput = $derived.by((): InspectorRecord | null => {
    if (ctrl.selectedBusData) {
      return {
        base_demand_mw: ctrl.selectedBusData.demand_mw,
        demand_change_mw: ctrl.committedDelta,
        effective_demand_mw:
          ctrl.selectedBusData.demand_mw + ctrl.committedDelta,
      };
    }
    if (ctrl.selectedBranchData) {
      return {
        base_rating_mw: ctrl.selectedBranchData.rate_mw,
        rating_change_mw: ctrl.committedRating,
        effective_rating_mw:
          ctrl.selectedBranchData.rate_mw + ctrl.committedRating,
      };
    }
    return null;
  });

  const hasSensitivity = $derived(
    (app.selectedBus !== null || app.selectedBranch !== null) &&
      (ctrl.selectedSensitivity !== null || app.sensitivityLoading),
  );
</script>

{#snippet scenarioControls()}
  {#if hasSensitivity}
    <div class="scenario-block">
      <p class="section-label">
        Scenario controls <span class="provenance modified">modified</span>
      </p>
      <SensitivityReadout showClear={false} />
      {#if app.selectedBus !== null}<DemandSlider />{:else}<RatingSlider />{/if}
      {#if ctrl.showMoverSlot}<TopMovers />{/if}
    </div>
  {/if}
{/snippet}

{#if !selected}
  <p class="empty mono">
    Select a bus or edge on the map to inspect its model properties and live
    results.
  </p>
{:else}
  {#if app.compactLayout}{@render scenarioControls()}{/if}

  {#if details}
    <div class="element-heading">
      <span class="kind mono">{details.typeLabel}</span>
      <strong>{details.id}</strong>
      <button type="button" class="clear mono" onclick={ctrl.clearSelection}
        ><span class="key-hint">esc&nbsp;</span>clear</button
      >
    </div>

    <details class="detail-group" open>
      <summary>Model properties <span class="provenance">source</span></summary>
      {#if details.record}
        <PropertyList record={details.record} />
      {:else}
        <p class="unavailable mono">
          Full source properties are loading or unavailable; map properties
          remain visible.
        </p>
      {/if}
    </details>

    {#each details.related as group (group.label)}
      <details class="detail-group">
        <summary
          >{group.label}
          <span class="count mono">{group.records.length}</span></summary
        >
        {#each group.records as record, index (index)}
          {#if index > 0}<hr />{/if}
          <PropertyList {record} />
        {/each}
      </details>
    {/each}
  {/if}

  {#if liveOutput}
    <details class="detail-group result" open>
      <summary
        >Current solution <span class="provenance result-label"
          >live output</span
        ></summary
      >
      <PropertyList record={liveOutput} />
    </details>
  {/if}

  {#if scenarioInput}
    <details class="detail-group">
      <summary
        >Effective inputs <span class="provenance modified">scenario</span
        ></summary
      >
      <PropertyList record={scenarioInput} />
    </details>
  {/if}

  {#if !app.compactLayout}{@render scenarioControls()}{/if}
{/if}

<style>
  .empty,
  .unavailable {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.5;
  }

  .element-heading {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 8px;
  }

  .element-heading strong {
    min-width: 0;
    overflow: hidden;
    font-size: 15px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    color: var(--text-accent);
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .clear {
    margin-left: auto;
    padding: 2px 0;
    border: 0;
    background: transparent;
    color: var(--text-secondary);
    font-size: 9.5px;
    cursor: pointer;
  }

  .clear:hover {
    color: var(--text-accent);
  }

  .detail-group {
    border-top: 1px solid var(--line);
  }

  .detail-group > summary {
    padding: 9px 0;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .provenance,
  .count {
    float: right;
    padding: 1px 5px;
    border: 1px solid var(--line);
    border-radius: 2px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 8.5px;
    font-weight: 400;
    text-transform: uppercase;
  }

  .result {
    border-top-color: color-mix(in srgb, var(--data-neg) 35%, var(--line));
  }
  .result-label {
    border-color: color-mix(in srgb, var(--data-neg) 45%, var(--line));
    color: var(--data-neg);
  }
  .modified {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--line));
    color: var(--text-accent);
  }

  .detail-group :global(.properties) {
    margin-bottom: 8px;
  }

  .detail-group hr {
    margin: 7px 0;
    border: 0;
    border-top: 1px dashed var(--line);
  }

  .scenario-block {
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--accent) 40%, var(--line));
  }

  .section-label {
    margin: 0 0 8px;
    font-size: 11.5px;
    font-weight: 600;
  }

  @media (hover: none), (pointer: coarse) {
    .key-hint {
      display: none;
    }
    .clear {
      min-height: 44px;
      padding-inline: 8px;
    }
  }

  @media (max-width: 760px) {
    .scenario-block {
      padding-top: 0;
      border-top: 0;
    }
    .element-heading {
      margin-top: 10px;
    }
  }
</style>
