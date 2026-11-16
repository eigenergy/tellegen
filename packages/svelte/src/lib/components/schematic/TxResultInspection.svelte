<script lang="ts">
	import { tick } from 'svelte';
	import {
		transferDirectionText,
		transferReading,
		type TransferReading
	} from '../../diagram/transmission-transfer.js';
	import { lineName, readable } from '../../format.js';
	import { previewHandlers, type PreviewItem } from './preview.js';
	import type { NetworkBranch, NetworkBus, SolutionFlow } from '@tellegen/engine';
	import type { operatingExceptions } from '../../diagram/transmission-inspection.js';
	import {
		assessVoltage,
		hasStatedRating,
		isRated,
		loadingStateText,
		type LoadingState
	} from '../../diagram/transmission-results.js';

	let {
		exceptions,
		showExceptions = true,
		transfer,
		bus,
		branch,
		voltage,
		voltageSolved,
		deenergized,
		generation,
		demand,
		flow,
		loading,
		stale,
		oninspect,
		onframe,
		onclear,
		onpreview,
		brief = false
	}: {
		exceptions: ReturnType<typeof operatingExceptions>;
		showExceptions?: boolean;
		transfer?: { dc: boolean; reading: TransferReading | undefined };
		bus: NetworkBus | undefined;
		branch: NetworkBranch | undefined;
		voltage: number | undefined;
		voltageSolved: boolean;
		deenergized: boolean;
		generation: number | undefined;
		demand: number | undefined;
		flow: SolutionFlow | undefined;
		loading: LoadingState | undefined;
		stale: boolean;
		oninspect: (kind: 'bus' | 'branch', id: number) => Promise<void>;
		onframe: () => void;
		onclear: () => void;
		/** Equipment under the pointer or keyboard focus in a list, to highlight on the diagram. */
		onpreview?: (item: PreviewItem | null) => void;
		/** Collapsed: the selection's heading and flow only, no exception list. */
		brief?: boolean;
	} = $props();
	const preview = (kind: PreviewItem['kind'], id: number) =>
		previewHandlers(onpreview, { kind, id });

	const batch = 12;
	let limit = $state(batch);
	let query = $state('');
	let open = $state(false);
	let heading = $state<HTMLElement>();
	let summary = $state<HTMLElement>();
	const needle = $derived(query.trim().toLowerCase());
	const buses = $derived(
		exceptions.buses.filter(
			(b) =>
				!needle || `bus ${b.id} ${b.name ?? ''} ${b.station ?? ''}`.toLowerCase().includes(needle)
		)
	);
	const branches = $derived(
		exceptions.branches.filter(
			(b) =>
				!needle ||
				`line ${b.id} branch ${b.id} bus ${b.from} bus ${b.to} ${b.from}–${b.to}`.includes(needle)
		)
	);
	const total = $derived(exceptions.buses.length + exceptions.branches.length);
	const info = $derived(flow?.loading_info);
	const rated = $derived(hasStatedRating(info) && isRated(loading));
	const voltageStatus = $derived(
		bus && voltageSolved ? assessVoltage(voltage, bus, deenergized).state : 'missing'
	);
	const voltageText = {
		deenergized: 'De-energized',
		missing: '—',
		unbounded: 'No limits',
		'invalid-limits': 'Invalid limits',
		normal: 'Within limits',
		low: 'Low',
		high: 'High'
	};
	/** The flow along the branch in words, when one end supplies the other. */
	const flowSentence = $derived.by(() => {
		if (!branch) return null;
		const reading = transfer ? transfer.reading : transferReading(flow, !voltageSolved);
		if (!reading?.magnitude || !reading.direction) return null;
		const [a, b] = reading.direction > 0 ? [branch.from, branch.to] : [branch.to, branch.from];
		return { mw: readable(reading.magnitude, 'MW').text, from: a, to: b };
	});
	export function focusSelection() {
		heading?.focus();
	}
	async function inspect(kind: 'bus' | 'branch', id: number) {
		open = false;
		await oninspect(kind, id);
		await tick();
		focusSelection();
		onframe();
	}
	async function clear() {
		onclear();
		await tick();
		summary?.focus();
	}
</script>

<!-- A rounded quantity; the exact value is kept for hover and for checks. -->
{#snippet q(value: number | null | undefined, unit: string, signed = false)}
	{@const r = readable(value, unit, signed)}
	<span data-exact={r.exact} title={r.exact === r.text ? undefined : `Exact: ${r.exact}`}
		>{r.text}</span
	>
{/snippet}

{#if bus || branch}
	<section class="inspection" aria-label="Selected equipment">
		<header>
			<h3 tabindex="-1" bind:this={heading}>
				{#if bus}Bus {bus.id}{bus.name ? ` (${bus.name})` : ''}{:else if branch}Line {branch.id}{/if}
			</h3>
			<button onclick={onframe}>Frame</button>
			<button onclick={clear} aria-label="Clear selection">Clear</button>
		</header>
		{#if stale}<span class="chip">Stale</span>{/if}
		{#if brief}
			{#if flowSentence}<p class="flow-sentence">
					<strong>{flowSentence.mw}</strong> from bus {flowSentence.from} to bus {flowSentence.to}
				</p>{/if}
		{:else if bus}
			<dl>
				{#if !transfer}
					<div>
						<dt>Voltage</dt>
						<dd>
							{#if voltageSolved}{@render q(deenergized ? undefined : voltage, 'pu')}{:else}—{/if}
						</dd>
					</div>
					<div>
						<dt>Min</dt>
						<dd>{@render q(bus.vmin_pu, 'pu')}</dd>
					</div>
					<div>
						<dt>Max</dt>
						<dd>{@render q(bus.vmax_pu, 'pu')}</dd>
					</div>
					{#if voltageSolved}<div>
							<dt>Status</dt>
							<dd>{voltageText[voltageStatus]}</dd>
						</div>{/if}
				{/if}
				<div>
					<dt>Generation</dt>
					<dd>{@render q(generation, 'MW')}</dd>
				</div>
				<div>
					<dt>Demand</dt>
					<dd>{@render q(demand, 'MW')}</dd>
				</div>
				<div>
					<dt>Capacity</dt>
					<dd>{@render q(bus.gen_mw, 'MW')}</dd>
				</div>
			</dl>
		{:else if branch}
			{#if flowSentence}<p class="flow-sentence">
					<strong>{flowSentence.mw}</strong> from bus {flowSentence.from} to bus {flowSentence.to}
				</p>{/if}
			<table>
				<thead>
					<tr>
						<th scope="col">End</th><th scope="col">Active</th>
						{#if !transfer?.dc}<th scope="col">Reactive</th>{/if}
					</tr>
				</thead>
				<tbody>
					<tr>
						<th scope="row">From bus {branch.from}</th>
						<td>{@render q(transfer ? transfer.reading?.from : flow?.mw, 'MW', true)}</td>
						{#if !transfer?.dc}<td>{@render q(flow?.qf, 'MVAr', true)}</td>{/if}
					</tr>
					<tr>
						<th scope="row">To bus {branch.to}</th>
						<td>{@render q(transfer ? transfer.reading?.to : flow?.pt, 'MW', true)}</td>
						{#if !transfer?.dc}<td>{@render q(flow?.qt, 'MVAr', true)}</td>{/if}
					</tr>
				</tbody>
			</table>
			<dl>
				{#if transfer}
					<div>
						<dt>Flow</dt>
						<dd>{@render q(transfer.reading?.magnitude, 'MW')}</dd>
					</div>
					<div>
						<dt>Direction</dt>
						<dd>
							{transfer.reading ? transferDirectionText(transfer.reading, branch) : '—'}
						</dd>
					</div>
				{/if}
				<div>
					<dt>Loading</dt>
					<dd>{@render q(rated && flow ? flow.loading * 100 : undefined, '%')}</dd>
				</div>
				<div>
					<dt>Status</dt>
					<dd>{loading ? loadingStateText[loading] : '—'}</dd>
				</div>
				<div>
					<dt>Rating</dt>
					<dd>{@render q(rated ? info?.rating : undefined, info?.basis.toUpperCase() ?? '')}</dd>
				</div>
				<div>
					<dt>Headroom</dt>
					<dd>
						{@render q(
							rated && flow && info?.rating ? info.rating * (1 - flow.loading) : undefined,
							info?.basis.toUpperCase() ?? '',
							true
						)}
					</dd>
				</div>
				{#if info?.limiting_end}<div>
						<dt>Limiting end</dt>
						<dd>{info.limiting_end}</dd>
					</div>{/if}
			</dl>
		{/if}
	</section>
{/if}

{#if showExceptions && !brief && total > 0}
	<details bind:open>
		<summary bind:this={summary}>{transfer ? 'Overloads' : 'Flagged'} ({total})</summary>
		{#if open}
			<label>
				Search
				<input
					type="search"
					bind:value={query}
					oninput={() => (limit = batch)}
					placeholder="Bus, station or line"
				/>
			</label>
			{#if !buses.length && !branches.length}<p>
					{needle ? 'No matches' : 'None'}
				</p>{/if}
			{#if buses.length}
				<h3>Buses ({buses.length})</h3>
				<ol>
					{#each buses.slice(0, limit) as b (b.id)}
						<li>
							<button onclick={() => inspect('bus', b.id)} {...preview('bus', b.id)}
								><strong>Bus {b.id}{b.name ? ` (${b.name})` : ''}</strong><span class="over"
									>{@render q(b.value, 'pu')} {b.state === 'low' ? 'Low' : 'High'}</span
								></button
							>
						</li>
					{/each}
				</ol>
			{/if}
			{#if branches.length}
				<h3>Lines ({branches.length})</h3>
				<ol>
					{#each branches.slice(0, limit) as b (b.id)}
						<li>
							<button onclick={() => inspect('branch', b.id)} {...preview('branch', b.id)}
								><strong>{lineName(b)}</strong><span class={{ over: b.state !== 'warning' }}
									>{@render q(b.loading * 100, '%')}</span
								></button
							>
						</li>
					{/each}
				</ol>
			{/if}
			{#if Math.max(buses.length, branches.length) > limit}
				<button onclick={() => (limit += batch)}>Show more</button>
			{/if}
		{/if}
	</details>
{/if}

<style>
	.inspection,
	details {
		width: 100%;
		min-width: 0;
		border-top: 1px solid var(--line);
		font-family: var(--font-display);
		font-size: var(--fs-label);
		line-height: var(--lh-snug);
		padding: var(--space-2xs) 0;
	}
	header,
	dl div {
		display: flex;
		align-items: baseline;
		gap: var(--space-xs);
	}
	header h3 {
		flex: 1;
		overflow-wrap: anywhere;
	}
	h3 {
		font: inherit;
		font-weight: 600;
		color: var(--ink);
		margin: var(--space-2xs) 0;
	}
	p {
		margin: var(--space-2xs) 0;
	}
	.flow-sentence {
		color: var(--ink);
		font-size: var(--fs-data);
	}
	.chip {
		padding: 1px 8px;
		border-radius: var(--radius-full);
		background: var(--accent-soft);
		color: var(--text-accent);
	}
	dt,
	th {
		color: var(--text-secondary);
	}
	dl {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(175px, 100%), 1fr));
		gap: var(--space-2xs) var(--space-lg);
		margin: var(--space-2xs) 0;
	}
	dl div {
		justify-content: space-between;
	}
	dd,
	td,
	li button > span {
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
		text-align: right;
	}
	dd {
		margin: 0;
		color: var(--ink);
	}
	table {
		border-collapse: collapse;
		width: 100%;
	}
	th,
	td {
		padding: var(--space-3xs);
		border-bottom: 1px solid var(--line);
	}
	th {
		font-weight: 400;
		text-align: left;
	}
	thead th:not(:first-child) {
		text-align: right;
	}
	td {
		color: var(--ink);
	}
	summary {
		cursor: pointer;
		color: var(--ink);
		padding: var(--space-2xs) 0;
	}
	label {
		display: grid;
		gap: var(--space-2xs);
		margin: var(--space-2xs) 0;
	}
	ol {
		padding-left: var(--space-3xl);
		margin: var(--space-2xs) 0;
	}
	li {
		padding: var(--space-3xs) 0;
	}
	li button {
		width: 100%;
		text-align: left;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-lg);
		justify-content: space-between;
		overflow-wrap: anywhere;
	}
	button,
	input {
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		padding: var(--space-2xs) var(--space-xs);
	}
	button {
		cursor: pointer;
	}
	/* At or over a limit: red, as on the drawing. */
	li button > span.over {
		color: var(--red);
	}
	@media (pointer: coarse) {
		button,
		input,
		summary {
			min-height: 44px;
		}
		summary {
			align-content: center;
		}
		input {
			font-size: 16px;
		}
	}
</style>
