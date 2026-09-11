<script lang="ts">
  import type { ModelDetails as ModelDetailsRecord } from "@tellegen/engine";

  let { details }: { details?: ModelDetailsRecord | null } = $props();
  let open = $state(false);
  let page = $state(0);
  let search = $state("");
  const pageSize = 20;
  const method = $derived(
    details?.method === "convex_quadratic_fit"
      ? "Convex quadratic fit"
      : details?.method === "exact"
        ? "Source model"
        : String(details?.method ?? "").replaceAll("_", " "),
  );
  const rows = $derived(
    open
      ? (details?.approximations.filter((row) =>
          `${row.generator} ${row.bus}`.includes(search.trim()),
        ) ?? [])
      : [],
  );
  const pageCount = $derived(Math.max(1, Math.ceil(rows.length / pageSize)));
  const currentPage = $derived(Math.min(page, pageCount - 1));
  const number = (value: number) =>
    value.toLocaleString(undefined, { maximumSignificantDigits: 4 });
</script>

{#if details}
  <details class="model-details" bind:open>
    <summary>Model details</summary>
    {#if open}
      <dl>
        <div>
          <dt>Method</dt>
          <dd>{method}</dd>
        </div>
        <div>
          <dt>Quantity</dt>
          <dd>{details.quantity}</dd>
        </div>
        <div>
          <dt>Approximated curves</dt>
          <dd>{details.approximations.length.toLocaleString()}</dd>
        </div>
      </dl>
      {#if details.approximations.length}
        <label
          >Find generator or bus<input
            type="search"
            value={search}
            oninput={(event) => {
              search = event.currentTarget.value;
              page = 0;
            }}
          /></label
        >
        <div class="table-scroll">
          <table>
            <caption>Errors at source breakpoints ({details.units})</caption>
            <thead
              ><tr
                ><th>Generator</th><th>Bus</th><th
                  ><abbr title="Root mean square error">RMS</abbr></th
                ><th>Maximum</th></tr
              ></thead
            >
            <tbody
              >{#each rows.slice(currentPage * pageSize, (currentPage + 1) * pageSize) as row (row.generator)}<tr
                  ><th>{row.generator}</th><td>{row.bus}</td><td
                    >{number(row.rms_breakpoint_error)}</td
                  ><td>{number(row.max_breakpoint_error)}</td></tr
                >{/each}</tbody
            >
          </table>
        </div>
        {#if !rows.length}<p>No matching generators.</p>{/if}
        {#if pageCount > 1}<div class="pagination">
            <button
              disabled={currentPage === 0}
              onclick={() => (page = currentPage - 1)}>Previous</button
            ><span
              >{currentPage * pageSize + 1}-{Math.min(
                (currentPage + 1) * pageSize,
                rows.length,
              )} of {rows.length}</span
            ><button
              disabled={currentPage + 1 >= pageCount}
              onclick={() => (page = currentPage + 1)}>Next</button
            >
          </div>{/if}
      {/if}
      <details class="references">
        <summary>Source and model IDs</summary>
        <dl>
          <div>
            <dt>Source</dt>
            <dd><code>{details.source}</code></dd>
          </div>
          <div>
            <dt>Model</dt>
            <dd><code>{details.model}</code></dd>
          </div>
        </dl>
      </details>
    {/if}
  </details>
{/if}

<style>
  .model-details {
    border-top: 1px solid var(--line);
    padding: 12px 0;
    font: 12px/1.45 var(--font-display);
    color: var(--ink);
  }
  summary {
    cursor: pointer;
    font-weight: 400;
  }
  dl {
    display: grid;
    gap: 7px;
    margin: 14px 0;
  }
  dl > div {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 10px;
  }
  dt {
    color: var(--text-secondary);
  }
  dd {
    margin: 0;
  }
  label {
    display: grid;
    gap: 6px;
    margin: 12px 0;
    font-size: 11px;
  }
  input {
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
    background: var(--paper);
    border: 1px solid var(--line);
    padding: 6px 8px;
    border-radius: 4px;
    color: inherit;
    font: inherit;
  }
  .table-scroll {
    max-height: 250px;
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 10px;
  }
  caption {
    text-align: left;
    color: var(--text-secondary);
    padding-bottom: 8px;
  }
  th,
  td {
    padding: 7px 4px;
    text-align: right;
    border-bottom: 1px solid var(--line);
    font-variant-numeric: tabular-nums;
  }
  th:first-child {
    text-align: left;
  }
  tbody th {
    font-weight: 400;
  }
  thead th {
    position: sticky;
    top: 0;
    background: var(--paper);
    font-weight: 500;
  }
  abbr {
    text-decoration-style: dotted;
    text-underline-offset: 3px;
  }
  .pagination {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 10px 0;
    font-size: 10px;
    color: var(--text-secondary);
  }
  button {
    padding: 4px 6px;
    font: inherit;
    border: 1px solid var(--line);
    background: transparent;
    border-radius: 4px;
    cursor: pointer;
    color: inherit;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  button:focus-visible,
  input:focus-visible,
  summary:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 2px;
  }
  .references {
    margin-top: 12px;
    font-size: 10px;
  }
  .references dl > div {
    grid-template-columns: 42px minmax(0, 1fr);
  }
  code {
    overflow-wrap: anywhere;
    font: 10px/1.5 var(--font-mono);
  }
</style>
