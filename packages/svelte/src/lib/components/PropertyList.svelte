<script lang="ts">
  import {
    inspectorLabel,
    inspectorValue,
    type InspectorRecord,
  } from "../element-inspector.js";

  interface Props {
    record: InspectorRecord;
  }

  let { record }: Props = $props();
  const entries = $derived(Object.entries(record));
</script>

<dl class="properties mono">
  {#each entries as [key, value] (key)}
    {@const formatted = inspectorValue(value)}
    <div class:complex={formatted === null}>
      <dt>{inspectorLabel(key)}</dt>
      {#if formatted !== null}
        <dd>{formatted}</dd>
      {:else}
        <dd>
          <details>
            <summary>show</summary>
            <pre>{JSON.stringify(value, null, 2)}</pre>
          </details>
        </dd>
      {/if}
    </div>
  {/each}
</dl>

<style>
  .properties {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
  }

  .properties > div {
    display: grid;
    grid-template-columns: minmax(92px, 0.9fr) minmax(0, 1.1fr);
    gap: 8px;
    padding: 4px 0;
    border-top: 1px solid color-mix(in srgb, var(--line) 62%, transparent);
  }

  .properties > div:first-child {
    border-top: 0;
  }

  dt {
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }

  dd {
    min-width: 0;
    margin: 0;
    text-align: right;
    overflow-wrap: anywhere;
  }

  details summary {
    color: var(--text-accent);
    cursor: pointer;
  }

  pre {
    max-width: 100%;
    overflow: auto;
    margin: 5px 0 0;
    padding: 7px;
    background: var(--surface-control);
    border: 1px solid var(--line);
    border-radius: 2px;
    font: inherit;
    font-size: 9.5px;
    line-height: 1.45;
    text-align: left;
    white-space: pre;
  }
</style>
