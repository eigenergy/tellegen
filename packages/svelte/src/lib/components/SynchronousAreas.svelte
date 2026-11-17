<script lang="ts">
	import { readModuleText } from '../module-text.js';
	import { areaSourceUrl } from '../synchronous-areas.js';
	import { readable } from '../format.js';

	let {
		moduleJson,
		selectedBus = null,
		onselect
	}: {
		moduleJson?: string | null;
		selectedBus?: number | null;
		onselect?: (bus: number) => void;
	} = $props();
	const reads = $derived(readModuleText(moduleJson));
	const plan = $derived(reads.synchronousAreas);
	const selectedArea = $derived(
		selectedBus == null ? undefined : plan?.byBus.get(selectedBus)?.id
	);
</script>

{#if reads.synchronousAreasInvalid}
	<p role="status">
		Synchronous-area details are unavailable: the saved planning records are
		incomplete.
	</p>
{:else if plan?.areas.length}
	<section aria-label="Synchronous areas" class="area-plan">
		<h3>Synchronous areas</h3>
		<p class="note">
			Approximate areas in this generated network. HVDC links are synthetic,
			lossless links with fixed planned transfers.
		</p>
		<ul class="areas">
			{#each plan.areas as area (area.id)}
				<li class={{ selected: selectedArea === area.id }}>
					<strong>{area.name}</strong>
					{#if selectedArea === area.id}<span class="selection"
							>Selected bus’s area</span
						>{/if}
					<span
						>{area.bus_ids.length.toLocaleString('en-US')} buses · {readable(
							area.demand_mw,
							'MW'
						).text} planned demand</span
					>
					<details>
						<summary>Area basis</summary>
						<p>{area.basis}</p>
						{#if areaSourceUrl(area.source_url)}<a
								href={areaSourceUrl(area.source_url)}
								target="_blank"
								rel="noopener noreferrer">Source for {area.name}</a
							>{/if}
					</details>
					{#if onselect && area.bus_ids.length}<button
							onclick={() => onselect?.(area.bus_ids[0])}
							>Inspect a bus in {area.name}</button
						>{/if}
				</li>
			{/each}
		</ul>
		<h4>Planned HVDC links</h4>
		{#if !plan.links.length}<p>No links between areas.</p>{/if}
		<ul class="links">
			{#each plan.links as link (link)}
				{@const from = plan.byBus.get(link.from_bus)!}
				{@const to = plan.byBus.get(link.to_bus)!}
				<li>
					<strong
						>{link.transfer_mw < 0 ? to.name : from.name} {link.transfer_mw === 0 ? '↔' : '→'} {link.transfer_mw <
						0
							? from.name
							: to.name}</strong
					>
					<span
						>{readable(Math.abs(link.transfer_mw), 'MW').text} planned transfer ·
						{readable(link.rating_mw, 'MW').text} rating</span
					>
					<div class="endpoints">
						{#if onselect}<button onclick={() => onselect?.(link.from_bus)}
								>Inspect bus {link.from_bus}</button
							><button onclick={() => onselect?.(link.to_bus)}
								>Inspect bus {link.to_bus}</button
							>
						{:else}<span>Bus {link.from_bus} ↔ bus {link.to_bus}</span>{/if}
					</div>
				</li>
			{/each}
		</ul>
	</section>
{/if}

<style>
	.area-plan {
		margin: var(--space-md) 0;
		font-size: var(--fs-label);
	}
	h3,
	h4 {
		margin: var(--space-sm) 0;
		font-size: inherit;
	}
	.note {
		color: var(--text-secondary);
		line-height: var(--lh-normal);
	}
	ul {
		list-style: none;
		padding: 0;
		margin: 0;
	}
	li {
		display: grid;
		gap: var(--space-2xs);
		padding: var(--space-xs);
		border-top: 1px solid var(--line);
		overflow-wrap: anywhere;
	}
	li.selected {
		background: var(--accent-soft);
	}
	.selection {
		color: var(--text-accent);
	}
	button {
		justify-self: start;
		font: inherit;
		cursor: pointer;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		padding: var(--space-2xs) var(--space-xs);
	}
	.endpoints {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xs);
	}
	summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	@media (pointer: coarse) {
		button {
			min-height: 44px;
		}
	}
</style>
