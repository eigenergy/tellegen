<script lang="ts">
	import {
		mcPfResultDetail,
		summarizeMcPfResult,
		type DistGraph,
		type McComplex,
		type McPfDetail,
		type McPfDetailQuery,
		type McPfResult,
		type McPfSummary
	} from '@tellegen/engine';
	import { neutralTerminal } from '../multiconductor.js';
	import type { McEditTiming } from '../state.svelte.js';
	let {
		result,
		detail,
		timing = null,
		elapsedMs,
		selectedBus,
		selectedEdge,
		graph
	}: {
		/** A live session's summary, or a stored full result. */
		result: McPfSummary | McPfResult;
		/** Fetches one bus's terminals and a page of equipment ports. A full
		 * `result` serves its own pages when this is omitted. */
		detail?: (query: McPfDetailQuery) => Promise<McPfDetail | null>;
		timing?: McEditTiming | null;
		elapsedMs: number | null;
		selectedBus: string | null;
		selectedEdge: string | null;
		graph: DistGraph | null;
	} = $props();
	const summary = $derived('terminals' in result ? summarizeMcPfResult(result) : result);
	const load = $derived.by(() => {
		if (detail) return detail;
		if (!('terminals' in result)) return null;
		const full = result;
		return (query: McPfDetailQuery) => Promise.resolve(mcPfResultDetail(full, query));
	});
	let busChoice = $state('');
	let edgeChoice = $state('');
	const busId = $derived(
		selectedBus ??
			(graph?.buses.some((b) => b.id === busChoice) ? busChoice : graph?.buses[0]?.id) ??
			''
	);
	const bus = $derived(graph?.buses.find((b) => b.id === busId));
	const neutralName = $derived(bus ? neutralTerminal(bus.terminals, bus.neutral_terminal) : null);
	const edgeId = $derived(selectedEdge ?? edgeChoice);
	let search = $state('');
	let page = $state(0);
	const size = 20;
	let portPage = $state(0);
	// Only the selected bus and one equipment page are fetched; the previous
	// rows stay visible until the page for a new operating point arrives.
	let current = $state.raw<McPfDetail | null>(null);
	let requested = 0;
	$effect(() => {
		const fetchDetail = load;
		const query: McPfDetailQuery = {
			bus: busId || null,
			element: edgeId || null,
			port_offset: portPage * size,
			port_limit: size
		};
		void result;
		if (!fetchDetail) {
			current = null;
			return;
		}
		const seq = ++requested;
		fetchDetail(query).then(
			(next) => {
				if (seq !== requested || !next) return;
				if (next.element_port_total > 0 && query.port_offset! >= next.element_port_total) {
					portPage = 0;
					return;
				}
				current = next;
			},
			() => {}
		);
	});
	const neutral = $derived(
		current?.terminals.find((t) => t.bus === busId && t.terminal === neutralName)?.voltage
	);
	const sourcePower = $derived(summary.source_power_into_network);
	const passiveLoss = $derived(summary.passive_loss.re);
	const terminals = $derived(
		(current?.terminals ?? []).filter(
			(row) =>
				(!busId || row.bus === busId) &&
				`${row.bus} ${row.terminal}`.toLowerCase().includes(search.toLowerCase())
		)
	);
	const pageCount = $derived(Math.max(1, Math.ceil(terminals.length / size)));
	const currentPage = $derived(Math.min(page, pageCount - 1));
	const visible = $derived(terminals.slice(currentPage * size, (currentPage + 1) * size));
	const portTotal = $derived(current?.element_port_total ?? 0);
	const portPages = $derived(Math.max(1, Math.ceil(portTotal / size)));
	const currentPortPage = $derived(Math.min(portPage, portPages - 1));
	const visiblePorts = $derived(current?.element_ports ?? []);
	const magnitude = (value: McComplex) => Math.hypot(value.re, value.im);
	const fixed = (value: number) => (Number.isFinite(value) ? value.toFixed(3) : '-');
	const ms = (value: number | null) => (value === null ? '-' : `${value.toFixed(1)} ms`);
	const angle = (value: McComplex) =>
		magnitude(value) > 1e-12 ? ((Math.atan2(value.im, value.re) * 180) / Math.PI).toFixed(2) : '-';
</script>

<section aria-label="AC power flow results" class="mc-results">
	<div class="result-heading">
		<strong>AC power flow</strong><span
			>{summary.iterations} iterations{elapsedMs === null
				? ''
				: `, ${Math.round(elapsedMs)} ms`}</span
		>
	</div>
	<dl class="totals">
		<div>
			<dt>Status</dt>
			<dd>{summary.converged ? 'Converged' : 'Not converged'}</dd>
		</div>
		<div>
			<dt>Voltage band</dt>
			<dd>{summary.voltage_valid ? 'Valid' : `${summary.voltage_violation_count} violations`}</dd>
		</div>
		<div>
			<dt>Load voltage range</dt>
			<dd>{summary.min_voltage_pu == null || summary.max_voltage_pu == null ? 'n/a' : `${fixed(summary.min_voltage_pu)}–${fixed(summary.max_voltage_pu)} pu`}</dd>
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
	<!-- Scrollable result tables accept keyboard navigation. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div class="scroll" role="region" aria-label="Terminal result columns" tabindex="0">
		<table aria-label="Terminal results">
			<thead
				><tr
					><th>Terminal</th><th>To ground<br />V</th><th>Angle<br />deg</th><th
						>Net current<br />A</th
					></tr
				></thead
			>
			<tbody
				>{#each visible as row (JSON.stringify([row.bus, row.terminal]))}
					<tr
						><td>{row.terminal}</td><td>{fixed(magnitude(row.voltage))}</td><td
							>{angle(row.voltage)}</td
						><td>{fixed(magnitude(row.current_into_network))}</td></tr
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
	{#if neutral}
		<details>
			<summary>Voltages to neutral</summary>
			<!-- Scrollable result tables accept keyboard navigation. -->
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<div class="scroll" role="region" aria-label="Neutral voltage columns" tabindex="0">
				<table aria-label="Neutral voltage results">
					<thead><tr><th>Terminal</th><th>To neutral<br />V</th><th>Angle<br />deg</th></tr></thead>
					<tbody
						>{#each visible as row (JSON.stringify([row.bus, row.terminal]))}
							{@const relative = {
								re: row.voltage.re - neutral.re,
								im: row.voltage.im - neutral.im
							}}
							<tr
								><td>{row.terminal}</td><td>{fixed(magnitude(relative))}</td><td
									>{angle(relative)}</td
								></tr
							>
						{/each}</tbody
					>
				</table>
			</div>
		</details>
	{/if}
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
		<!-- Scrollable result tables accept keyboard navigation. -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<div class="scroll" role="region" aria-label="Equipment current columns" tabindex="0">
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
			<span>{portTotal} connections</span>{#if portPages > 1}<button
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
				<dd>{summary.physical_kcl_residual.toExponential(3)} A</dd>
			</div>
			<div>
				<dt>Scaled KCL residual</dt>
				<dd>{summary.scaled_kcl_residual.toExponential(3)}</dd>
			</div>
			<div>
				<dt>Voltage change</dt>
				<dd>{summary.voltage_change.toExponential(3)} V</dd>
			</div>
			<div>
				<dt>Matrix size</dt>
				<dd>{summary.matrix_dimension}</dd>
			</div>
			<div>
				<dt>Nonzero entries</dt>
				<dd>{summary.matrix_nonzeros}</dd>
			</div>
			<div>
				<dt>Factorizations</dt>
				<dd>{summary.factorization_count}</dd>
			</div>
			{#if timing}
				<div>
					<dt>Engine solve</dt>
					<dd>{ms(timing.engine_ms)}</dd>
				</div>
				<div>
					<dt>Transfer and parse</dt>
					<dd>
						{ms(
							timing.engine_ms === null
								? null
								: timing.round_trip_ms - timing.engine_ms + timing.parse_ms
						)}
					</dd>
				</div>
				<div>
					<dt>Total update</dt>
					<dd>{ms(timing.total_ms)}</dd>
				</div>
			{/if}
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
		max-width: 100%;
	}
	.scroll:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
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
