<script lang="ts">
	import { tick } from 'svelte';
	import {
		transferDirectionText,
		transferReading,
		type TransferReading
	} from '../../diagram/transmission-transfer.js';
	import { readable } from '../../format.js';
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
			(b) => !needle || `branch ${b.id} · bus ${b.from} to bus ${b.to}`.includes(needle)
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
		missing: 'Voltage unavailable',
		unbounded: 'Bounds unavailable',
		'invalid-limits': 'Invalid bounds',
		normal: 'Within available bounds',
		low: 'Below minimum',
		high: 'Above maximum'
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
				{#if bus}Bus {bus.id}{bus.name ? ` · ${bus.name}` : ''}{:else if branch}Branch {branch.id}{/if}
			</h3>
			<button onclick={onframe}>Frame</button>
			<button onclick={clear} aria-label="Clear selection">Clear</button>
		</header>
		{#if stale}<p>Showing previous or incomplete results</p>{/if}
		{#if brief}
			{#if flowSentence}<p class="flow-sentence">
					<strong>{flowSentence.mw}</strong> flows from bus {flowSentence.from} to bus {flowSentence.to}
				</p>{/if}
		{:else if bus}
			<dl>
				{#if !transfer}
					<div>
						<dt>Voltage</dt>
						<dd>
							{#if voltageSolved}{@render q(
									deenergized ? undefined : voltage,
									'pu'
								)}{:else}Unavailable in DC{/if}
						</dd>
					</div>
					<div>
						<dt>Minimum</dt>
						<dd>{@render q(bus.vmin_pu, 'pu')}</dd>
					</div>
					<div>
						<dt>Maximum</dt>
						<dd>{@render q(bus.vmax_pu, 'pu')}</dd>
					</div>
					{#if voltageSolved}<div>
							<dt>Voltage status</dt>
							<dd>{voltageText[voltageStatus]}</dd>
						</div>{/if}
				{/if}
				<div>
					<dt>Solved generation</dt>
					<dd>{@render q(generation, 'MW')}</dd>
				</div>
				<div>
					<dt>Model demand</dt>
					<dd>{@render q(demand, 'MW')}</dd>
				</div>
				<div>
					<dt>Installed capacity</dt>
					<dd>{@render q(bus.gen_mw, 'MW')}</dd>
				</div>
			</dl>
		{:else if branch}
			{#if flowSentence}<p class="flow-sentence">
					<strong>{flowSentence.mw}</strong> flows from bus {flowSentence.from} to bus {flowSentence.to}
				</p>{/if}
			<p class="convention">Signs at each end: positive where power enters the branch.</p>
			<table>
				<thead>
					<tr>
						<th scope="col">End</th><th scope="col">Active</th>
						{#if !transfer?.dc}<th scope="col">Reactive</th>{/if}
					</tr>
				</thead>
				<tbody>
					<tr>
						<th scope="row">From · bus {branch.from}</th>
						<td>{@render q(transfer ? transfer.reading?.from : flow?.mw, 'MW', true)}</td>
						{#if !transfer?.dc}<td>{@render q(flow?.qf, 'MVAr', true)}</td>{/if}
					</tr>
					<tr>
						<th scope="row">To · bus {branch.to}</th>
						<td>{@render q(transfer ? transfer.reading?.to : flow?.pt, 'MW', true)}</td>
						{#if !transfer?.dc}<td>{@render q(flow?.qt, 'MVAr', true)}</td>{/if}
					</tr>
				</tbody>
			</table>
			<dl>
				{#if transfer}
					<div>
						<dt>Active magnitude</dt>
						<dd>{@render q(transfer.reading?.magnitude, 'MW')}</dd>
					</div>
					<div>
						<dt>Direction</dt>
						<dd>
							{transfer.reading ? transferDirectionText(transfer.reading, branch) : 'Unavailable'}
						</dd>
					</div>
				{/if}
				<div>
					<dt>Utilization</dt>
					<dd>{@render q(rated && flow ? flow.loading * 100 : undefined, '%')}</dd>
				</div>
				<div>
					<dt>Loading status</dt>
					<dd>{loading ? loadingStateText[loading] : 'Unavailable'}</dd>
				</div>
				<div>
					<dt>Rating basis</dt>
					<dd>{info?.basis.toUpperCase() ?? 'Unavailable'}</dd>
				</div>
				<div>
					<dt>Effective rating</dt>
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
				<div>
					<dt>Limiting end</dt>
					<dd>{info?.limiting_end ?? (info?.basis === 'mw' ? 'DC magnitude' : 'Unavailable')}</dd>
				</div>
			</dl>
		{/if}
	</section>
{/if}

{#if showExceptions && !brief}
	<details bind:open>
		<summary bind:this={summary}>{transfer ? 'Overloads' : 'Exceptions'} · {total}</summary>
		{#if open}
			<label>
				Search exceptions
				<input
					type="search"
					bind:value={query}
					oninput={() => (limit = batch)}
					placeholder="Bus, station or branch ID"
				/>
			</label>
			{#if !buses.length && !branches.length}<p>
					{needle
						? 'No matching exceptions.'
						: transfer
							? 'No reported overloads.'
							: 'No reported operating exceptions.'}
				</p>{/if}
			{#if buses.length}
				<h3>Voltage · {buses.length}</h3>
				<ol>
					{#each buses.slice(0, limit) as b (b.id)}
						<li>
							<button onclick={() => inspect('bus', b.id)} {...preview('bus', b.id)}
								><strong>Bus {b.id}{b.name ? ` · ${b.name}` : ''}</strong><span
									>{@render q(b.value, 'pu')} · {@render q(b.exceedance, 'pu')}
									{b.state === 'low' ? 'below minimum' : 'above maximum'}</span
								></button
							>
						</li>
					{/each}
				</ol>
			{/if}
			{#if branches.length}
				<h3>Branch loading · {branches.length}</h3>
				<ol>
					{#each branches.slice(0, limit) as b (b.id)}
						<li>
							<button onclick={() => inspect('branch', b.id)} {...preview('branch', b.id)}
								><strong>Branch {b.id} · {b.from} → {b.to}</strong><span
									>{@render q(b.loading * 100, '%')} · {loadingStateText[b.state]}</span
								></button
							>
						</li>
					{/each}
				</ol>
			{/if}
			{#if Math.max(buses.length, branches.length) > limit}
				<button onclick={() => (limit += batch)}>Show more exceptions</button>
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
		font: 11px var(--font-mono);
		padding: 4px 0;
	}
	header,
	dl div {
		display: flex;
		align-items: baseline;
		gap: 6px;
	}
	header h3 {
		flex: 1;
		overflow-wrap: anywhere;
	}
	h3 {
		font: inherit;
		font-weight: 700;
		color: var(--ink);
		margin: 4px 0;
	}
	p {
		margin: 4px 0;
	}
	.flow-sentence {
		color: var(--ink);
		font-size: 12px;
	}
	.convention {
		color: var(--text-secondary);
	}
	dl {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(175px, 100%), 1fr));
		gap: 4px 12px;
		margin: 5px 0;
	}
	dl div {
		justify-content: space-between;
	}
	dd {
		margin: 0;
		color: var(--ink);
		text-align: right;
	}
	table {
		border-collapse: collapse;
		width: 100%;
	}
	th,
	td {
		padding: 3px;
		text-align: left;
		border-bottom: 1px solid var(--line);
	}
	td {
		color: var(--ink);
	}
	summary {
		cursor: pointer;
		color: var(--ink);
		padding: 4px 0;
	}
	label {
		display: grid;
		gap: 4px;
		margin: 4px 0;
	}
	ol {
		padding-left: 23px;
		margin: 4px 0;
	}
	li {
		padding: 2px 0;
	}
	li button {
		width: 100%;
		text-align: left;
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		justify-content: space-between;
		overflow-wrap: anywhere;
	}
	button,
	input {
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 5px 6px;
	}
	button {
		cursor: pointer;
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
