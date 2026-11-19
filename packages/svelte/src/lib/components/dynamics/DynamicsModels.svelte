<script lang="ts">
	/** The Models tab (experimental): every generating unit of the active
	 * case with its dynamic models, and the editor of the one picked (also
	 * opened from a unit's glyph on the map).
	 * @block Models tab */
	import { untrack } from 'svelte';
	import { getAppState, getController } from '../../context.svelte.js';
	import { unitKey, unitSummary } from '../../dynamics/units.svelte.js';
	import DynamicsUnitEditor from './DynamicsUnitEditor.svelte';

	const app = getAppState();
	const dyn = getController().dynamics;
	const catalog = dyn.catalog;

	let filter = $state('');
	const shown = $derived.by(() => {
		const q = filter.trim().toLowerCase();
		const hit = q
			? catalog.units.filter((u) =>
					`${u.bus} ${u.machine_id} ${unitSummary(u)}`.toLowerCase().includes(q)
				)
			: catalog.units;
		return hit.slice(0, 80);
	});
	let picked = $state<string | null>(null);
	// A bus selected on the map picks its first unit. The units reload after
	// every edit; that keeps the pick (another unit of the bus, or one picked
	// from the list) as long as the unit still exists.
	let pickedFor: number | null = null;
	$effect(() => {
		const b = app.selectedBus ?? null;
		const units = catalog.units;
		if (b === null) {
			pickedFor = null;
			return;
		}
		const cur = untrack(() => picked);
		if (b === pickedFor && cur !== null && units.some((u) => unitKey(u) === cur)) return;
		const u = units.find((x) => x.bus === b);
		if (u) {
			picked = unitKey(u);
			pickedFor = b;
		}
	});
</script>

<section class="models dyn-form" aria-label="Unit models" data-testid="dyn-models">
	<p class="dim">
		Every generating unit with its dynamic models; on the map, click a unit's G, I or S chip. Changes
		write new dynamic data and initialize the case again.
		<span class="pill-experimental">Experimental</span>
	</p>
	{#if catalog.error}<p class="err">{catalog.error}</p>{/if}
	<input
		class="filter"
		type="search"
		placeholder="Filter units: bus, id or model"
		bind:value={filter}
		aria-label="Filter units"
	/>
	<ul class="units" aria-label="Units">
		{#each shown as u (unitKey(u))}
			<li>
				<button aria-pressed={unitKey(u) === picked} onclick={() => (picked = unitKey(u))}
					><span class="mono"
						>{u.bus}{u.machine_id.trim() === '1' ? '' : ` '${u.machine_id.trim()}'`}</span
					>
					<span class="dim">{unitSummary(u)}</span></button
				>
			</li>
		{/each}
		{#if catalog.units.length > shown.length}<li class="dim">… filter to see the others</li>{/if}
	</ul>
	{#if picked}
		<div class="editor"><DynamicsUnitEditor unitKey={picked} /></div>
	{:else}
		<p class="dim">Pick a unit, or select a bus on the map.</p>
	{/if}
</section>

<style>
	.models {
		display: grid;
		gap: 8px;		grid-template-columns: minmax(0, 1fr);
		min-width: 0;
	}
	p {
		margin: 0;
	}
	.filter {
		font: inherit;
	}
	.units {
		margin: 0;
		padding: 0;
		list-style: none;
		max-height: 180px;
		overflow: auto;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
	}
	.units button {
		display: flex;
		gap: 8px;
		width: 100%;
		border: none;
		background: none;
		padding: 2px 6px;
		text-align: left;
		font: inherit;
		font-size: var(--fs-control);
		color: var(--ink);
		cursor: pointer;
	}
	.units button[aria-pressed='true'] {
		background: var(--surface-chip);
		box-shadow: var(--inset-active);
	}
	.editor {
		padding-top: 6px;
		border-top: 1px solid var(--line);
	}
	.mono {
		font-family: var(--font-mono);
	}
	.dim {
		color: var(--text-tertiary);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
</style>
