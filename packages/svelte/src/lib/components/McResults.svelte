<script lang="ts">
	import type { McPfResult, McComplex } from '@tellegen/engine';
	let {
		result,
		elapsedMs,
		selectedBus,
		selectedEdge
	}: {
		result: McPfResult;
		elapsedMs: number | null;
		selectedBus: string | null;
		selectedEdge: string | null;
	} = $props();
	let search = $state('');
	let page = $state(0);
	const size = 20;
	const terminals = $derived(
		result.terminals.filter(
			(row) =>
				(!selectedBus || row.bus === selectedBus) &&
				`${row.bus} ${row.terminal}`.toLowerCase().includes(search.toLowerCase())
		)
	);
	const pageCount = $derived(Math.max(1, Math.ceil(terminals.length / size)));
	const currentPage = $derived(Math.min(page, pageCount - 1));
	const visible = $derived(terminals.slice(currentPage * size, (currentPage + 1) * size));
	const ports = $derived(
		result.element_ports.filter(
			(row) =>
				(!selectedBus || row.bus === selectedBus) && (!selectedEdge || row.element === selectedEdge)
		)
	);
	let portPage = $state(0);
	const portPages = $derived(Math.max(1, Math.ceil(ports.length / size)));
	const currentPortPage = $derived(Math.min(portPage, portPages - 1));
	const visiblePorts = $derived(ports.slice(currentPortPage * size, (currentPortPage + 1) * size));
	const magnitude = (value: McComplex) => Math.hypot(value.re, value.im);
	const fixed = (value: number) => (Number.isFinite(value) ? value.toFixed(3) : '-');
	const angle = (value: McComplex) =>
		magnitude(value) > 1e-12 ? fixed((Math.atan2(value.im, value.re) * 180) / Math.PI) : '-';
</script>

<section aria-label="AC power flow results" class="mc-results">
	<div class="result-heading">
		<strong>AC power flow</strong><span
			>{result.iterations} iterations{elapsedMs === null
				? ''
				: `, ${Math.round(elapsedMs)} ms`}</span
		>
	</div>
	<label class="search"
		>Terminals<input
			aria-label="Find terminal"
			type="search"
			placeholder="Bus or terminal"
			bind:value={search}
			oninput={() => (page = 0)}
		/></label
	>
	<div class="scroll">
		<table aria-label="Terminal results">
			<thead
				><tr
					><th>Bus</th><th>Terminal</th><th>Voltage<br /> (V)</th><th>Angle<br /> (deg)</th><th
						>Net current (A)</th
					></tr
				></thead
			>
			<tbody
				>{#each visible as row (JSON.stringify([row.bus, row.terminal]))}<tr
						><td>{row.bus}</td><td>{row.terminal}</td><td>{fixed(magnitude(row.voltage))}</td><td
							>{angle(row.voltage)}</td
						><td>{fixed(magnitude(row.current_into_network))}</td></tr
					>{/each}</tbody
			>
		</table>
	</div>
	<div class="pagination">
		<span>{terminals.length} terminals{selectedBus ? ` at bus ${selectedBus}` : ''}</span
		>{#if pageCount > 1}<button
				disabled={currentPage === 0}
				onclick={() => (page = currentPage - 1)}
				aria-label="Previous terminals">Previous</button
			><span>{currentPage + 1} / {pageCount}</span><button
				disabled={currentPage + 1 >= pageCount}
				onclick={() => (page = currentPage + 1)}
				aria-label="Next terminals">Next</button
			>{/if}
	</div>
	<details>
		<summary>Equipment currents</summary>
		<p>Current enters the named equipment at each bus terminal.</p>
		<div class="scroll">
			<table aria-label="Equipment currents">
				<thead><tr><th>Equipment</th><th>Bus</th><th>Terminal</th><th>Current (A)</th></tr></thead
				><tbody
					>{#each visiblePorts as row (JSON.stringify( [row.kind, row.element, row.branch, row.bus, row.terminal] ))}<tr
							><td>{row.kind} {row.element}</td><td>{row.bus}</td><td>{row.terminal}</td><td
								>{fixed(magnitude(row.current_into_element))}</td
							></tr
						>{/each}</tbody
				>
			</table>
		</div>
		<div class="pagination">
			<span>{ports.length} connections</span>{#if portPages > 1}<button
					disabled={currentPortPage === 0}
					onclick={() => (portPage = currentPortPage - 1)}
					aria-label="Previous currents">Previous</button
				><span>{currentPortPage + 1} / {portPages}</span><button
					disabled={currentPortPage + 1 >= portPages}
					onclick={() => (portPage = currentPortPage + 1)}
					aria-label="Next currents">Next</button
				>{/if}
		</div>
	</details>
	<details>
		<summary>Calculation details</summary>
		<dl>
			<div>
				<dt>Maximum KCL residual</dt>
				<dd>{result.physical_kcl_residual.toExponential(3)} A</dd>
			</div>
			<div>
				<dt>Scaled KCL residual</dt>
				<dd>{result.scaled_kcl_residual.toExponential(3)}</dd>
			</div>
			<div>
				<dt>Voltage change</dt>
				<dd>{result.voltage_change.toExponential(3)} V</dd>
			</div>
			<div>
				<dt>Matrix size</dt>
				<dd>{result.matrix_dimension}</dd>
			</div>
			<div>
				<dt>Nonzero entries</dt>
				<dd>{result.matrix_nonzeros}</dd>
			</div>
			<div>
				<dt>Factorizations</dt>
				<dd>{result.factorization_count}</dd>
			</div>
		</dl>
		<p>
			Voltage is measured to ground. Net current is the injection into the network. The calculation
			checks current balance at unconstrained terminals.
		</p>
	</details>
</section>

<style>
	.mc-results {
		margin: 14px 0;
		padding-top: 12px;
		border-top: 1px solid var(--line);
		font-size: 11px;
	}
	.result-heading,
	.pagination,
	dl div {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}
	.result-heading span,
	.pagination,
	dt,
	p {
		color: var(--text-secondary);
	}
	.search {
		display: flex;
		flex-direction: column;
		gap: 5px;
		margin: 12px 0 8px;
	}
	input,
	button {
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 5px 7px;
	}
	button {
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.4;
		cursor: default;
	}
	input {
		width: 100%;
		box-sizing: border-box;
	}
	.scroll {
		overflow: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font: 10px var(--font-mono);
	}
	th,
	td {
		padding: 6px 5px;
		text-align: right;
		white-space: nowrap;
		border-bottom: 1px solid var(--line);
	}
	th:first-child,
	td:first-child {
		text-align: left;
	}
	th {
		color: var(--text-secondary);
		font-weight: 500;
	}
	.pagination {
		margin: 8px 0;
		justify-content: flex-end;
	}
	.pagination > span:first-child {
		margin-right: auto;
	}
	details {
		margin-top: 12px;
	}
	summary {
		cursor: pointer;
		font-weight: 500;
	}
	p {
		line-height: 1.5;
		margin: 8px 0;
	}
	dl {
		font: 10px var(--font-mono);
	}
	dl div {
		padding: 4px 0;
	}
	dd {
		margin: 0;
	}
</style>
