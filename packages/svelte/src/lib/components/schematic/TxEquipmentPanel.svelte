<script lang="ts">
	import TxDcTopology from './TxDcTopology.svelte';
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
		<h3>Equipment models</h3>
		<button onclick={onclose} aria-label="Close equipment models">Close</button>
	</header>
	<p class="intro">Source settings and their treatment in this calculation.</p>
	{#if !network.equipment}
		<p>
			Equipment metadata is unavailable in this saved view. Reimport the source to identify shunts,
			controls and HVDC.
		</p>
	{:else}
		{#if dc && (dc.nodes || dc.lines || dc.converters)}
			<p class="notice">
				Detailed DC data retained: {dc.nodes} nodes, {dc.lines} lines, {dc.converters} converters.
				{network.equipment.hvdc.length
					? 'The main diagram shows reduced links.'
					: 'Detailed DC connections are not drawn as AC branches.'}
				Expand the detailed network below.
			</p>
		{/if}
		{#if network.equipment.import_notes?.length}
			<details>
				<summary>Import coverage · {network.equipment.import_notes.length} notes</summary>
				<ul>
					{#each network.equipment.import_notes as note}<li>{note}</li>{/each}
				</ul>
			</details>
		{/if}
		<TxDcTopology equipment={network.equipment} {moduleJson} />
		<label
			>Find equipment <input
				type="search"
				bind:value={query}
				placeholder="PST, capacitor, bus…"
			/></label
		>
		<p class="count">{filtered.length} of {items.length} devices</p>
		{#if detail}
			<article aria-label={detail.title}>
				<h4>{detail.title}</h4>
				<p><strong>{detail.state}</strong></p>
				<p class="notice">
					{formulation.toUpperCase()}: {equipmentTreatment(detail.kind, formulation)}
				</p>
				<ul>
					{#each detail.facts as fact}<li>{fact}</li>{/each}
				</ul>
				<p>{detail.explanation}</p>
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
		{#if !items.length}<p>
				No transformer settings, shunts or reduced HVDC links are recorded in this payload.
			</p>{/if}
	{/if}
</section>

<style>
	.equipment-panel {
		color: var(--ink, #37352f);
		background: var(--paper, #fbfaf6);
		padding: 12px;
		border: 1px solid #d2cec4;
		border-radius: 8px;

		box-sizing: border-box;
		font-size: 12px;
		line-height: 1.45;
	}
	header {
		position: sticky;
		top: 0;
		z-index: 1;
		background: var(--paper, #fbfaf6);
		padding-block: 4px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}
	h3,
	h4 {
		margin: 0;
		font-size: 14px;
	}
	p {
		margin: 8px 0;
	}
	label {
		display: grid;
		gap: 4px;
	}
	input {
		width: 100%;
		box-sizing: border-box;
		min-height: 36px;
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
		gap: 8px;
		width: 100%;
		text-align: left;
		padding: 6px;
		border: 1px solid transparent;
		background: transparent;
		border-radius: 4px;
	}
	.devices button[aria-pressed='true'] {
		border-color: #566b95;
		background: #e9edf5;
	}
	small {
		display: block;
		color: #67635b;
	}
	svg {
		flex-shrink: 0;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.4;
	}
	article {
		border-block: 1px solid #d2cec4;
		padding: 10px 0;
		margin-bottom: 8px;
		overflow-wrap: anywhere;
	}
	article ul {
		padding-left: 18px;
	}
	.notice {
		border-left: 3px solid #956b2a;
		padding-left: 8px;
	}
	.count,
	.intro {
		color: #67635b;
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
