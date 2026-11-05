<script lang="ts">
	import { tick } from 'svelte';
	import type { NetworkBranch, NetworkBus, SolutionFlow } from '@tellegen/engine';
	import type { operatingExceptions } from '../../diagram/transmission-inspection.js';
	import {
		assessVoltage,
		hasStatedRating,
		loadingStateText,
		type LoadingState
	} from '../../diagram/transmission-results.js';

	let {
		exceptions,
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
		onclear
	}: {
		exceptions: ReturnType<typeof operatingExceptions>;
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
	} = $props();

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
	const rated = $derived(
		hasStatedRating(info) &&
			loading !== undefined &&
			['normal', 'warning', 'at-limit', 'violation'].includes(loading)
	);
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
	const number = new Intl.NumberFormat('en', { maximumFractionDigits: 6, useGrouping: false });
	function quantity(value: number | null | undefined, unit: string, signed = false) {
		if (value == null || !Number.isFinite(value)) return 'Unavailable';
		return `${signed && value > 0 ? '+' : ''}${number.format(Object.is(value, -0) ? 0 : value)}${unit === '%' ? '' : ' '}${unit}`;
	}
	async function inspect(kind: 'bus' | 'branch', id: number) {
		open = false;
		await oninspect(kind, id);
		await tick();
		heading?.focus();
		onframe();
	}
	async function clear() {
		onclear();
		await tick();
		summary?.focus();
	}
</script>

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
		{#if bus}
			<dl>
				<div>
					<dt>Voltage</dt>
					<dd>
						{voltageSolved
							? quantity(deenergized ? undefined : voltage, 'pu')
							: 'Unavailable in DC'}
					</dd>
				</div>
				<div>
					<dt>Minimum</dt>
					<dd>{quantity(bus.vmin_pu, 'pu')}</dd>
				</div>
				<div>
					<dt>Maximum</dt>
					<dd>{quantity(bus.vmax_pu, 'pu')}</dd>
				</div>
				{#if voltageSolved}<div>
						<dt>Voltage status</dt>
						<dd>{voltageText[voltageStatus]}</dd>
					</div>{/if}
				<div>
					<dt>Solved generation</dt>
					<dd>{quantity(generation, 'MW')}</dd>
				</div>
				<div>
					<dt>Model demand</dt>
					<dd>{quantity(demand, 'MW')}</dd>
				</div>
				<div>
					<dt>Installed capacity</dt>
					<dd>{quantity(bus.gen_mw, 'MW')}</dd>
				</div>
			</dl>
		{:else if branch}
			<p>Positive flow enters the branch from the named bus.</p>
			<table>
				<thead
					><tr><th scope="col">End</th><th scope="col">Active</th><th scope="col">Reactive</th></tr
					></thead
				>
				<tbody>
					<tr
						><th scope="row">From · bus {branch.from}</th><td>{quantity(flow?.mw, 'MW', true)}</td
						><td>{quantity(flow?.qf, 'MVAr', true)}</td></tr
					>
					<tr
						><th scope="row">To · bus {branch.to}</th><td>{quantity(flow?.pt, 'MW', true)}</td><td
							>{quantity(flow?.qt, 'MVAr', true)}</td
						></tr
					>
				</tbody>
			</table>
			<dl>
				<div>
					<dt>Utilization</dt>
					<dd>{rated && flow ? quantity(flow.loading * 100, '%') : 'Unavailable'}</dd>
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
					<dd>{quantity(rated ? info?.rating : undefined, info?.basis.toUpperCase() ?? '')}</dd>
				</div>
				<div>
					<dt>Headroom</dt>
					<dd>
						{quantity(
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

<details bind:open>
	<summary bind:this={summary}>Exceptions · {total}</summary>
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
				{needle ? 'No matching exceptions.' : 'No reported operating exceptions.'}
			</p>{/if}
		{#if buses.length}
			<h3>Voltage · {buses.length}</h3>
			<ol>
				{#each buses.slice(0, limit) as b (b.id)}
					<li>
						<button onclick={() => inspect('bus', b.id)}
							><strong>Bus {b.id}{b.name ? ` · ${b.name}` : ''}</strong><span
								>{quantity(b.value, 'pu')} · {quantity(b.exceedance, 'pu')}
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
						<button onclick={() => inspect('branch', b.id)}
							><strong>Branch {b.id} · {b.from} → {b.to}</strong><span
								>{quantity(b.loading * 100, '%')} · {loadingStateText[b.state]}</span
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
