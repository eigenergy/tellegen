<script lang="ts">
	import type { DistGraph, McPfResult } from '@tellegen/engine';
	import MultiWire from './MultiWire.svelte';
	import OmniSearch from './OmniSearch.svelte';
	import ResultsTable from './ResultsTable.svelte';
	import {
		buildSearchIndex,
		busState,
		conductorBars,
		graphInputs,
		LOADING_WARNING,
		loadingState,
		magnitudeUnbalance,
		multiWireBranch,
		pairBar,
		queryTable,
		rankIssues,
		snapshotFromMcPf,
		snapshotSummary,
		tableCsv,
		tableModel,
		type Issue,
		type SearchMatch,
		type TableKind
	} from '../diagram/index.js';

	let {
		result,
		graph,
		loading = false,
		selectedBus,
		selectedEdge,
		onselectbus,
		onselectedge,
		label = 'Multiconductor power flow',
		onanalyse,
		schematic = false,
		onschematic,
		hovered = null,
		onhover,
		onreveal
	}: {
		/** The full operating point; `null` while it is being fetched. */
		result: McPfResult | null;
		graph: DistGraph;
		loading?: boolean;
		selectedBus: string | null;
		selectedEdge: string | null;
		onselectbus: (id: string) => void;
		onselectedge: (id: string) => void;
		/** Case name, used in exported file names and provenance. */
		label?: string;
		/** When set, the overview is computed on request rather than after every solve. */
		onanalyse?: () => void;
		/** Whether the case is shown as a schematic, and how to toggle it. */
		schematic?: boolean;
		onschematic?: (show: boolean) => void;
		/** Bus or branch under the pointer in any linked view. */
		hovered?: { kind: 'bus' | 'branch'; id: string } | null;
		onhover?: (h: { kind: 'bus' | 'branch'; id: string } | null) => void;
		/** Bring the current selection into view again, for an issue clicked while selected. */
		onreveal?: () => void;
	} = $props();

	const ISSUE_LIMIT = 50;
	const inputs = $derived(graphInputs(graph));
	const snapshot = $derived(
		result ? snapshotFromMcPf(result, inputs.buses, inputs.branches) : null
	);
	const issues = $derived(snapshot ? rankIssues(snapshot) : []);
	const summary = $derived(snapshot ? snapshotSummary(snapshot, issues) : null);
	const limitsDeclared = $derived(
		inputs.buses.some((b) => b.limits) || inputs.branches.some((b) => b.rating)
	);
	const busIndex = $derived(new Map(graph.buses.map((b, i) => [b.id, i])));
	const edgeIndex = $derived(new Map(graph.edges.map((e, i) => [e.id, i])));
	// Network-only structures are rebuilt when the case changes, not after every solve.
	const searchIndex = $derived(buildSearchIndex(inputs.entities));
	let tableKind = $state<TableKind>('bus');
	const tables = $derived(
		snapshot
			? {
					bus: tableModel(snapshot, inputs.names, 'bus'),
					branch: tableModel(snapshot, inputs.names, 'branch')
				}
			: null
	);
	const tableSelected = $derived(
		tableKind === 'bus'
			? selectedBus !== null
				? (busIndex.get(selectedBus) ?? -1)
				: -1
			: selectedEdge !== null
				? (edgeIndex.get(selectedEdge) ?? -1)
				: -1
	);
	const tableHovered = $derived(
		hovered?.kind === tableKind
			? ((tableKind === 'bus' ? busIndex : edgeIndex).get(hovered.id) ?? -1)
			: -1
	);
	/** Search results show the bus voltage nearest a limit, or the branch loading. */
	function searchHint(m: SearchMatch) {
		if (!snapshot) return null;
		if (m.kind === 'branch') {
			const loading = snapshot.branches[m.index].loading;
			return loading === null ? null : { text: `${n(loading, 1)} %`, state: loadingState(loading) };
		}
		const bus = snapshot.buses[m.index];
		if (!bus.pairs.length) return null;
		const lowest = bus.pairs.reduce((a, b) => (b.volts < a.volts ? b : a));
		return { text: `${n(lowest.volts)} V`, state: busState(bus) };
	}
	/** Selecting what is already selected would toggle it off in the controller. */
	function selectOnce(kind: 'bus' | 'branch', index: number) {
		const id = kind === 'bus' ? graph.buses[index].id : graph.edges[index].id;
		if (id !== (kind === 'bus' ? selectedBus : selectedEdge)) select(kind, index);
	}

	const n = (v: number, digits = 2) =>
		v.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
	/** Compact range: kV above 1 kV, four significant figures; exact values are in the tables. */
	function range(a: number, b: number): string {
		const k = Math.max(Math.abs(a), Math.abs(b)) >= 1000;
		const f = (v: number) => Number((k ? v / 1000 : v).toPrecision(4)).toLocaleString();
		return `${f(a)}–${f(b)} ${k ? 'kV' : 'V'}`;
	}
	const zoneLabel = (zone: string) =>
		zone === 'phase–neutral'
			? 'V phase–neutral'
			: zone === 'phase–phase'
				? 'V phase–phase'
				: `V ${zone}`;

	/** An issue click selects it; clicking the selected issue again shows it rather than clearing it. */
	function revealOrSelect(kind: 'bus' | 'branch', index: number) {
		const id = kind === 'bus' ? graph.buses[index].id : graph.edges[index].id;
		if (id === (kind === 'bus' ? selectedBus : selectedEdge)) onreveal?.();
		else select(kind, index);
	}
	function select(kind: 'bus' | 'branch', index: number) {
		if (kind === 'bus') onselectbus(graph.buses[index].id);
		else onselectedge(graph.edges[index].id);
	}
	function issueText(issue: Issue) {
		if (issue.kind === 'voltage') {
			const id = graph.buses[issue.target.index].id;
			return {
				id,
				value: `${n(issue.value)} V`,
				detail: `${issue.pair?.join('–')} ${issue.low ? 'below' : 'above'} ${n(issue.limit)} V by ${n(issue.score)} %`
			};
		}
		const edge = graph.edges[issue.target.index];
		return {
			id: edge.id,
			value: `${n(issue.value, 1)} %`,
			detail: `${edge.kind} · ${issue.severity === 'violation' ? 'over rating' : `≥ ${LOADING_WARNING} % of rating`}`
		};
	}
	const isSelected = (issue: Issue) =>
		issue.target.kind === 'bus'
			? graph.buses[issue.target.index]?.id === selectedBus
			: graph.edges[issue.target.index]?.id === selectedEdge;

	const busSelection = $derived(
		snapshot && selectedBus !== null && busIndex.has(selectedBus)
			? snapshot.buses[busIndex.get(selectedBus) as number]
			: null
	);
	// Phase–neutral magnitudes also move with neutral shift, which VUF excludes; show both.
	const busSpread = $derived(
		busSelection?.unbalance ? magnitudeUnbalance(busSelection.pairs) : null
	);
	const multiWire = $derived(
		selectedEdge !== null && edgeIndex.has(selectedEdge)
			? multiWireBranch(graph, edgeIndex.get(selectedEdge) as number, snapshot)
			: null
	);
	const edgeSelection = $derived(
		snapshot && selectedEdge !== null && edgeIndex.has(selectedEdge)
			? conductorBars(snapshot.branches[edgeIndex.get(selectedEdge) as number].ends)
			: null
	);

	function exportCsv(kind: TableKind) {
		if (!snapshot) return;
		const model = tableModel(snapshot, inputs.names, kind);
		const csv = tableCsv(model, queryTable(model), [
			`Tellegen export ${new Date().toISOString()}`,
			`Case: ${label}`,
			`${snapshot.label}; ${snapshot.iterations ?? '?'} iterations`,
			'P/Q are positive into the equipment at each end; voltages are terminal-pair magnitudes'
		]);
		const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
		const a = document.createElement('a');
		a.href = url;
		a.download = `${label.replace(/[^\w.-]+/g, '_')}-${kind === 'bus' ? 'buses' : 'branches'}.csv`;
		a.click();
		setTimeout(() => URL.revokeObjectURL(url), 1000);
	}
</script>

<section class="workbench" aria-label="Operating point overview">
	<div class="heading">
		<h4>Overview</h4>
		{#if loading}<span role="status">Reading the full operating point…</span>{/if}
		{#if onschematic}<button
				class="schematic-toggle"
				aria-pressed={schematic}
				onclick={() => onschematic(!schematic)}>{schematic ? 'Show map' : 'Show schematic'}</button
			>{/if}
	</div>
	<OmniSearch
		index={searchIndex}
		hint={searchHint}
		onselect={(m) => selectOnce(m.kind === 'bus' ? 'bus' : 'branch', m.index)}
	/>
	{#if summary}
		{@const { voltage, overload, warning } = summary.counts}
		<div class="kpis">
			<button
				class="kpi"
				class:violation={voltage + overload > 0}
				class:warning={voltage + overload === 0 && warning > 0}
				disabled={!issues.length}
				onclick={() => issues[0] && select(issues[0].target.kind, issues[0].target.index)}
				title="Select the worst issue"
			>
				<span class="label">Violations</span>
				<span class="value"
					>{voltage + overload ? (voltage + overload).toLocaleString() : 'None'}</span
				>
				<span class="sub"
					>{warning
						? `+ ${warning} warning${warning === 1 ? '' : 's'}`
						: limitsDeclared
							? 'within declared limits'
							: 'no limits declared'}</span
				>
			</button>
			<div class="kpi" title="Sum of both into-equipment end powers over active branches">
				<span class="label">Losses</span>
				<span class="value">{n(summary.loss.p)} kW</span>
				<span class="sub">{n(summary.loss.q)} kvar</span>
			</div>
			{#each summary.zones as zone (zone.zone)}
				{@const bad =
					zone.min.state === 'violation'
						? zone.min
						: zone.max.state === 'violation'
							? zone.max
							: null}
				<button
					class="kpi"
					class:violation={!!bad}
					onclick={() => select('bus', (bad ?? zone.min).bus)}
					title={`Minimum at ${graph.buses[zone.min.bus].id}, maximum at ${graph.buses[zone.max.bus].id}`}
				>
					<span class="label">{zoneLabel(zone.zone)}</span>
					<span class="value">{range(zone.min.volts, zone.max.volts)}</span>
					<span class="sub">{bad ? 'limit violated' : `${zone.buses.toLocaleString()} buses`}</span>
				</button>
			{/each}
			{#if summary.loading}
				{@const top = summary.loading}
				<button
					class="kpi"
					class:violation={top.value > 100}
					class:warning={top.value >= 90 && top.value <= 100}
					onclick={() => select('branch', top.branch)}
					title={snapshot?.branches[top.branch].basis ?? ''}
				>
					<span class="label">Max loading</span>
					<span class="value">{n(top.value, 1)} %</span>
					<span class="sub">{graph.edges[top.branch].id}</span>
				</button>
			{/if}
			{#if summary.unbalance}
				{@const u = summary.unbalance}
				<button
					class="kpi"
					onclick={() => select('bus', u.bus)}
					title={u.definition === 'VUF'
						? 'Voltage unbalance factor |V2| / |V1| from terminal phasors'
						: 'Magnitude-only unbalance: max deviation from the mean / mean'}
				>
					<span class="label">Max unbalance</span>
					<span class="value">{n(u.percent)} %</span>
					<span class="sub">{u.definition} · {graph.buses[u.bus].id}</span>
				</button>
			{/if}
		</div>

		<h4>Issues <span class="count">{issues.length.toLocaleString()}</span></h4>
		{#if !limitsDeclared}
			<p class="hint">
				This case declares no voltage limits or ratings, so nothing is ranked against them.
			</p>
		{:else if !issues.length}
			<p class="hint ok">No limit violations or loading warnings.</p>
		{:else}
			<ol class="issues">
				{#each issues.slice(0, ISSUE_LIMIT) as issue (`${issue.target.kind}:${issue.target.index}`)}
					{@const t = issueText(issue)}
					<li class={issue.severity}>
						<button
							aria-current={isSelected(issue)}
							onclick={() => revealOrSelect(issue.target.kind, issue.target.index)}
							title={`${t.id} · ${t.detail}`}
						>
							<span class="sev" aria-label={issue.severity}
								>{issue.severity === 'violation' ? '!' : '▲'}</span
							>
							<span class="id">{t.id}</span>
							<span class="num">{t.value}</span>
							<span class="detail">{t.detail}</span>
						</button>
					</li>
				{/each}
			</ol>
			{#if issues.length > ISSUE_LIMIT}<p class="hint">
					Showing the worst {ISSUE_LIMIT} of {issues.length.toLocaleString()}; the CSV has all rows.
				</p>{/if}
		{/if}

		{#if busSelection}
			<h4>Phases at {selectedBus}</h4>
			{#if busSelection.pairs.length}
				<div class="bars" role="list" aria-label="Pair voltages against declared limits">
					{#each busSelection.pairs.map(pairBar) as bar, k (k)}
						<div class="bar {bar.state}" role="listitem">
							<span class="mono">{bar.pair.join('–')}</span>
							<span class="track">
								{#if bar.band}<i
										class="band"
										style:left="{bar.band.left}%"
										style:width="{bar.band.width}%"
									></i>{/if}
								<i class="mark" style:left="{bar.marker}%"></i>
							</span>
							<span class="num">{n(bar.volts)} V{bar.state === 'violation' ? ' !' : ''}</span>
						</div>
					{/each}
				</div>
				<p class="hint">
					Track: the declared band ± 25 %{busSelection.unbalance
						? ` · unbalance ${n(busSelection.unbalance.percent)} % (${busSelection.unbalance.definition})`
						: ''}{busSpread
						? ` · magnitude spread ${n(busSpread.percent)} % (${busSpread.definition})`
						: ''}
				</p>
				{#if busSelection.unbalance && busSpread && busSpread.percent > 2 * busSelection.unbalance.percent}
					<p class="hint">
						The spread is mostly neutral shift (zero sequence), which VUF excludes.
					</p>
				{/if}
			{:else}<p class="hint">No terminal-pair voltages at this bus.</p>{/if}
		{:else if edgeSelection}
			<h4>Conductors on {selectedEdge}</h4>
			{#each edgeSelection.ends as end, side (side)}
				{#if end}
					<p class="end">{side ? 'To' : 'From'} end · {n(end.p)} kW · {n(end.q)} kvar</p>
					<div
						class="bars"
						role="list"
						aria-label={`${side ? 'To' : 'From'} end conductor currents`}
					>
						{#each end.bars as bar, k (k)}
							<div class="bar" class:neutral={bar.neutral} role="listitem">
								<span class="mono">{bar.terminal}</span>
								<span class="track"><i class="fill" style:width="{bar.fill}%"></i></span>
								<span class="num">{n(bar.amps, 1)} A</span>
							</div>
						{/each}
					</div>
				{/if}
			{/each}
			<p class="hint">
				One scale for both ends (max {n(edgeSelection.scaleAmps, 1)} A){edgeSelection.neutralShare !==
				null
					? ` · neutral carries ${n(edgeSelection.neutralShare, 0)} % of the largest phase current`
					: ''}
			</p>
			{#if multiWire}
				<h4>Multi-wire view</h4>
				<MultiWire view={multiWire} />
			{/if}
		{/if}

		{#if tables}
			<details class="tables">
				<summary>Results table</summary>
				<div class="tabs" role="tablist" aria-label="Results table">
					{#each ['bus', 'branch'] as const as kind (kind)}
						<button role="tab" aria-selected={tableKind === kind} onclick={() => (tableKind = kind)}
							>{kind === 'bus' ? 'Buses' : 'Branches'}</button
						>
					{/each}
				</div>
				{#key tableKind}
					<ResultsTable
						model={tables[tableKind]}
						selected={tableSelected}
						onselect={(i) => selectOnce(tableKind, i)}
						hovered={tableHovered}
						onhover={(i) =>
							onhover?.(
								i < 0
									? null
									: {
											kind: tableKind,
											id: tableKind === 'bus' ? graph.buses[i].id : graph.edges[i].id
										}
							)}
					/>
				{/key}
			</details>
		{/if}

		<div class="actions">
			<button onclick={() => exportCsv('bus')}>Bus CSV</button>
			<button onclick={() => exportCsv('branch')}>Branch CSV</button>
		</div>
	{:else if !loading && onanalyse}
		<p class="hint">
			This case is large, so the overview reads the full operating point on request.
		</p>
		<div class="actions"><button onclick={onanalyse}>Analyse operating point</button></div>
	{:else if !loading}
		<p class="hint">Run the power flow to see the overview.</p>
	{/if}
</section>

<style>
	.workbench {
		margin: 14px 0;
		padding-top: 12px;
		border-top: 1px solid var(--line);
		font-size: 11px;
	}
	.heading {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 8px;
	}
	.heading span,
	.hint,
	.count,
	.end,
	.kpi .label,
	.kpi .sub,
	.detail {
		color: var(--text-secondary);
	}
	h4 {
		margin: 12px 0 6px;
		font-size: 11px;
		font-weight: 600;
	}
	.kpis {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
		gap: 1px;
		background: var(--line);
		border: 1px solid var(--line);
		border-radius: 3px;
		overflow: hidden;
	}
	.kpi {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		padding: 6px 8px;
		text-align: left;
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 0;
		border-radius: 0;
	}
	button.kpi {
		cursor: pointer;
	}
	button.kpi:disabled {
		cursor: default;
	}
	.kpi .label {
		font: 9px var(--font-mono);
		letter-spacing: 0.5px;
		text-transform: uppercase;
	}
	.kpi .value {
		font: 12px var(--font-mono);
	}
	.kpi .label,
	.kpi .value,
	.kpi .sub {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.kpi.violation .value,
	.violation .sev,
	.violation .num {
		color: var(--text-danger);
	}
	.kpi.warning .value,
	.warning .sev,
	.warning .num {
		color: var(--text-accent);
	}
	.ok {
		color: var(--pos);
	}
	.issues {
		list-style: none;
		margin: 0;
		padding: 0;
		max-height: 240px;
		overflow: auto;
	}
	.issues button {
		display: grid;
		grid-template-columns: 12px 1fr auto;
		gap: 1px 6px;
		width: 100%;
		padding: 5px 6px;
		text-align: left;
		font: inherit;
		color: var(--ink);
		background: transparent;
		border: 0;
		border-radius: 3px;
		cursor: pointer;
	}
	.issues button:hover,
	.issues button[aria-current='true'] {
		background: var(--accent-soft);
	}
	.sev {
		font: 700 11px var(--font-mono);
	}
	.id,
	.num,
	.mono {
		font-family: var(--font-mono);
		white-space: nowrap;
	}
	.id {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.detail {
		grid-column: 2 / 4;
	}
	.bars {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.bar {
		display: grid;
		grid-template-columns: 30px 1fr 84px;
		gap: 6px;
		align-items: center;
	}
	.bar .num {
		text-align: right;
	}
	.track {
		position: relative;
		height: 9px;
		background: var(--surface-control);
		border-radius: 2px;
		overflow: hidden;
	}
	.band {
		position: absolute;
		inset: 0 auto;
		background: color-mix(in srgb, var(--pos) 25%, transparent);
		border-inline: 1px solid var(--text-secondary);
	}
	.mark {
		position: absolute;
		inset: -1px auto;
		width: 3px;
		margin-left: -1.5px;
		background: var(--ink);
	}
	.violation .mark {
		background: var(--text-danger);
	}
	.fill {
		position: absolute;
		inset: 0 auto 0 0;
		background: var(--text-secondary);
	}
	.neutral .fill {
		background: var(--accent);
	}
	.end {
		margin: 8px 0 3px;
	}
	.schematic-toggle {
		margin-left: auto;
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 3px 8px;
		cursor: pointer;
	}
	.schematic-toggle[aria-pressed='true'] {
		color: var(--accent);
		background: var(--accent-soft);
	}
	.workbench :global(.omni) {
		margin: 6px 0 10px;
	}
	.tables {
		margin-top: 12px;
	}
	.tables summary {
		cursor: pointer;
		font-weight: 600;
	}
	.tabs {
		display: flex;
		gap: 12px;
		margin: 8px 0 6px;
	}
	.tabs button {
		font: 10px var(--font-mono);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		color: var(--text-secondary);
		background: transparent;
		border: 0;
		border-bottom: 2px solid transparent;
		padding: 2px 0;
		cursor: pointer;
	}
	.tabs button[aria-selected='true'] {
		color: var(--ink);
		border-bottom-color: var(--accent);
	}
	.actions {
		display: flex;
		gap: 6px;
		margin-top: 12px;
	}
	.actions button {
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 5px 7px;
		cursor: pointer;
	}
</style>
