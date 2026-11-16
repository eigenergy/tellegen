<script lang="ts">
	import StatBoxes from '../StatBoxes.svelte';
	import TxDcTopology from './TxDcTopology.svelte';
	import TxControlResults from './TxControlResults.svelte';
	import { solveRegulated, type RegulatedSolution } from '@tellegen/engine';
	import { onDestroy } from 'svelte';
	import type { Network } from '@tellegen/engine';
	import {
		equipmentGlyph,
		equipmentItems,
		equipmentTreatment
	} from '../../diagram/transmission-equipment.js';
	import { glyphPath } from '../../diagram/schematic-equipment.js';
	let {
		network,
		selected = '',
		onselect,
		onclose,
		formulation,
		moduleJson
	}: {
		network: Network;
		formulation: string;
		moduleJson?: string;
		selected?: string;
		onselect: (key: string) => void;
		onclose: () => void;
	} = $props();
	let query = $state('');
	let regulated = $state<RegulatedSolution | null>(null);
	let controlError = $state('');
	let controlBusy = $state(false);
	let controlGeneration = 0;
	$effect(() => {
		void moduleJson;
		controlGeneration++;
		regulated = null;
		controlError = '';
		controlBusy = false;
	});
	onDestroy(() => controlGeneration++);
	async function runRegulators() {
		if (!moduleJson) return;
		const request = ++controlGeneration;
		controlBusy = true;
		controlError = '';
		regulated = null;
		try {
			const next = await solveRegulated(moduleJson);
			if (request === controlGeneration) regulated = next;
		} catch (error) {
			if (request === controlGeneration)
				controlError = error instanceof Error ? error.message : String(error);
		} finally {
			if (request === controlGeneration) controlBusy = false;
		}
	}
	const items = $derived(equipmentItems(network));
	const filtered = $derived(
		items.filter((i) =>
			`${i.title} ${i.facts.join(' ')}`.toLowerCase().includes(query.toLowerCase())
		)
	);
	const detail = $derived(items.find((i) => i.key === selected));
	const dc = $derived(network.equipment?.detailed_dc);
</script>

<section aria-label="Equipment models" class="equipment-panel">
	<header>
		<h3>Equipment</h3>
		<button class="close" onclick={onclose} aria-label="Close equipment models" title="Close"
			><svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"
				><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" /></svg
			></button
		>
	</header>
	{#if !network.equipment || (!items.length && !dc?.converters && !dc?.nodes && !dc?.lines && !network.equipment.import_notes?.length)}
		<p class="empty">No equipment</p>
	{:else}
		{#if dc && (dc.nodes || dc.lines || dc.converters)}
			<StatBoxes
				label="DC network"
				stats={[
					{ label: 'DC nodes', value: dc.nodes.toLocaleString() },
					{ label: 'DC lines', value: dc.lines.toLocaleString() },
					{ label: 'Converters', value: dc.converters.toLocaleString() }
				]}
			/>
		{/if}
		{#if network.equipment.import_notes?.length}
			<details>
				<summary>Import notes ({network.equipment.import_notes.length})</summary>
				<ul>
					{#each network.equipment.import_notes as note, index (index)}<li>{note}</li>{/each}
				</ul>
			</details>
		{/if}
		<TxDcTopology equipment={network.equipment} {moduleJson} />
		{#if moduleJson && !dc?.converters && items.length}
			<details>
				<summary>Regulators</summary>
				<button onclick={runRegulators} disabled={controlBusy}
					>{controlBusy ? 'Solving…' : 'Solve with regulators'}</button
				>
				{#if controlError}<p role="alert">{controlError}</p>{/if}
				{#if regulated}
					<StatBoxes
						label="Regulator result"
						stats={[
							{ label: 'Passes', value: String(regulated.passes) },
							{ label: 'Controls', value: String(regulated.controls.length) },
							{
								label: 'Violations',
								value: String(regulated.violations.length),
								tone: regulated.violations.length ? 'warning' : 'ok'
							}
						]}
					/>
					<TxControlResults controls={regulated.controls} />
					{#each regulated.violations as violation, index (index)}<p class="notice">
							{violation}
						</p>{/each}
				{/if}
			</details>
		{/if}
		{#if items.length}
			<label
				>Find equipment <input type="search" bind:value={query} placeholder="Name or bus" /></label
			>
			{#if query}<p class="count">{filtered.length} of {items.length}</p>{/if}
		{/if}
		{#if detail}
			<article aria-label={detail.title}>
				<h4>{detail.title}</h4>
				<p><strong>{detail.state}</strong></p>
				<ul>
					<li>Model: {equipmentTreatment(detail.kind, formulation)}</li>
					{#each detail.facts as fact, index (index)}<li>{fact}</li>{/each}
				</ul>
			</article>
		{/if}
		<ul class="devices" aria-label="Equipment inventory">
			{#each filtered as item (item.key)}
				<li>
					<button aria-pressed={item.key === selected} onclick={() => onselect(item.key)}>
						<svg viewBox="-14 -12 42 24" width="42" height="24" aria-hidden="true"
							><path
								d={glyphPath(equipmentGlyph(item.kind, item.state.startsWith('Closed')), 8)}
							/></svg
						>
						<span>{item.title}<small>{item.state}</small></span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.equipment-panel {
		color: var(--ink);
		background: var(--paper);
		padding: var(--space-lg);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		box-sizing: border-box;
		font-family: var(--font-display);
		font-size: var(--fs-label);
		line-height: var(--lh-normal);
	}
	header {
		position: sticky;
		top: 0;
		z-index: 1;
		background: var(--paper);
		padding-block: var(--space-2xs);
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
	}
	h3,
	h4 {
		margin: 0;
		font-size: var(--fs-heading);
		font-weight: 600;
	}
	p {
		margin: var(--space-sm) 0;
	}
	label {
		display: grid;
		gap: var(--space-2xs);
	}
	input {
		width: 100%;
		box-sizing: border-box;
		min-height: 36px;
		padding: 0 10px;
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
	}
	button {
		font: inherit;
		color: inherit;
		cursor: pointer;
		min-height: 36px;
	}
	.devices {
		list-style: none;
		padding: 0;
		margin: 0;
	}
	.devices button {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		width: 100%;
		text-align: left;
		padding: var(--space-xs);
		border: 1px solid transparent;
		background: transparent;
		border-radius: var(--radius-sm);
	}
	.devices button[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	small {
		display: block;
		color: var(--text-secondary);
	}
	svg {
		flex-shrink: 0;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.4;
	}
	article {
		border-block: 1px solid var(--line);
		padding: var(--space-md) 0;
		margin-bottom: var(--space-sm);
		overflow-wrap: anywhere;
	}
	article ul {
		padding-left: var(--space-xl);
	}
	.count,
	.empty {
		color: var(--text-secondary);
	}
	.empty {
		margin: 0;
	}
	.close {
		display: grid;
		place-items: center;
		width: 28px;
		min-height: 28px;
		padding: 0;
		border: 0;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--text-secondary);
	}
	.close:hover {
		background: var(--surface-hover);
		color: var(--ink);
	}
	.close svg {
		stroke-width: 1.6;
	}
	@media (max-width: 600px) {
		button {
			min-height: 44px;
		}
		input {
			min-height: 44px;
		}
	}
</style>
