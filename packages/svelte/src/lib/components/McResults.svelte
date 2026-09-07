<script lang="ts">
	import type { McPfResult, McComplex, DistGraph } from '@tellegen/engine';
	import { neutralTerminal } from '../multiconductor.js';
	let {
		result,
		elapsedMs,
		selectedBus,
		selectedEdge,
		graph
	}: {
		result: McPfResult;
		elapsedMs: number | null;
		selectedBus: string | null;
		selectedEdge: string | null;
		graph: DistGraph | null;
	} = $props();
	let busChoice = $state('');
	let edgeChoice = $state('');
	const busId = $derived(
		selectedBus ??
			(graph?.buses.some((b) => b.id === busChoice) ? busChoice : graph?.buses[0]?.id) ??
			''
	);
	const bus = $derived(graph?.buses.find((b) => b.id === busId));
	const neutralName = $derived(bus ? neutralTerminal(bus.terminals, bus.neutral_terminal) : null);
	const neutral = $derived(
		result.terminals.find((t) => t.bus === busId && t.terminal === neutralName)?.voltage
	);
	const edgeId = $derived(selectedEdge ?? edgeChoice);
	const sourcePower = $derived(
		result.source_reactions.reduce(
			(sum, port) => ({
				re: sum.re + port.power_into_network.re,
				im: sum.im + port.power_into_network.im
			}),
			{ re: 0, im: 0 }
		)
	);
	const passiveLoss = $derived(
		result.element_ports
			.filter((port) => !['load', 'generator', 'ibr'].includes(port.kind))
			.reduce((sum, port) => sum + port.power_into_element.re, 0)
	);
	let search = $state('');
	let page = $state(0);
	const size = 20;
	const terminals = $derived(
		result.terminals.filter(
			(row) =>
				(!busId || row.bus === busId) &&
				`${row.bus} ${row.terminal}`.toLowerCase().includes(search.toLowerCase())
		)
	);
	const pageCount = $derived(Math.max(1, Math.ceil(terminals.length / size)));
	const currentPage = $derived(Math.min(page, pageCount - 1));
	const visible = $derived(terminals.slice(currentPage * size, (currentPage + 1) * size));
	const ports = $derived(result.element_ports.filter((row) => !edgeId || row.element === edgeId));
	let portPage = $state(0);
	const portPages = $derived(Math.max(1, Math.ceil(ports.length / size)));
	const currentPortPage = $derived(Math.min(portPage, portPages - 1));
	const visiblePorts = $derived(ports.slice(currentPortPage * size, (currentPortPage + 1) * size));
	const magnitude = (value: McComplex) => Math.hypot(value.re, value.im);
	const fixed = (value: number) => (Number.isFinite(value) ? value.toFixed(3) : '-');
	const angle = (value: McComplex) =>
		magnitude(value) > 1e-12 ? ((Math.atan2(value.im, value.re) * 180) / Math.PI).toFixed(2) : '-';
</script>

<section aria-label="AC power flow results" class="mc-results">
	<div class="result-heading">
		<strong>AC power flow</strong><span
			>{result.iterations} iterations{elapsedMs === null
				? ''
				: `, ${Math.round(elapsedMs)} ms`}</span
		>
	</div>
	<dl class="totals">
		<div>
			<dt>Status</dt>
			<dd>Converged</dd>
		</div>
		<div>
			<dt>Source P / Q</dt>
			<dd>{fixed(sourcePower.re / 1000)} / {fixed(sourcePower.im / 1000)} kW / kvar</dd>
		</div>
		<div>
			<dt>Passive loss</dt>
			<dd>{fixed(passiveLoss / 1000)} kW</dd>
		</div>
	</dl>
	<label class="search"
		>Bus result<select
			value={busId}
			onchange={(event) => {
				busChoice = event.currentTarget.value;
				selectedBus = null;
				page = 0;
			}}
			>{#each graph?.buses ?? [] as bus (bus.id)}<option value={bus.id}>{bus.id}</option
				>{/each}</select
		></label
	>
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
					><th>Terminal</th><th>To ground<br />V</th><th>Angle<br />deg</th>{#if neutral}<th
							>To neutral<br />V</th
						><th>Angle<br />deg</th>{/if}<th>Net current<br />A</th></tr
				></thead
			>
			<tbody
				>{#each visible as row (JSON.stringify([row.bus, row.terminal]))}
					{@const relative = neutral
						? { re: row.voltage.re - neutral.re, im: row.voltage.im - neutral.im }
						: null}
					<tr
						><td>{row.terminal}</td><td>{fixed(magnitude(row.voltage))}</td><td
							>{angle(row.voltage)}</td
						>{#if relative}<td>{fixed(magnitude(relative))}</td><td>{angle(relative)}</td>{/if}<td
							>{fixed(magnitude(row.current_into_network))}</td
						></tr
					>
				{/each}</tbody
			>
		</table>
	</div>
	<div class="pagination">
		<span>{terminals.length} terminals{busId ? ` at bus ${busId}` : ''}</span
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
		<summary>Equipment currents and power</summary>
		<label class="search"
			>Branch result<select
				value={edgeId}
				onchange={(event) => {
					edgeChoice = event.currentTarget.value;
					selectedEdge = null;
					portPage = 0;
				}}
				><option value="">All equipment</option>{#each graph?.edges ?? [] as edge (edge.id)}<option
						value={edge.id}>{edge.from} - {edge.to}, {edge.id}</option
					>{/each}</select
			></label
		>
		<p>Current enters the named equipment at each bus terminal.</p>
		<div class="scroll">
			<table aria-label="Equipment currents">
				<thead
					><tr
						><th>Equipment</th><th>Bus</th><th>Terminal</th><th>Current<br />A</th><th>P<br />kW</th
						><th>Q<br />kvar</th></tr
					></thead
				><tbody
					>{#each visiblePorts as row (JSON.stringify( [row.kind, row.element, row.branch, row.bus, row.terminal] ))}<tr
							><td>{row.kind} {row.element}</td><td>{row.bus}</td><td>{row.terminal}</td><td
								>{fixed(magnitude(row.current_into_element))}</td
							><td>{fixed(row.power_into_element.re / 1000)}</td><td
								>{fixed(row.power_into_element.im / 1000)}</td
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
	select,
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
	input,
	select {
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
