<script lang="ts">
	import {
		mcNetworkScopeLabel,
		type McNetworkScope as NetworkScope,
		type McExcludedIsland
	} from '@tellegen/engine';
	let {
		scope,
		islands = [],
		preview = false
	}: {
		scope?: NetworkScope | null;
		islands?: McExcludedIsland[];
		preview?: boolean;
	} = $props();
</script>

{#if scope}
	<aside aria-label="Network calculation scope">
		<strong>{preview ? 'Unpowered islands detected' : mcNetworkScopeLabel(scope)}</strong>
		{#if preview}<p>
				{scope.excluded_island_count}
				{scope.excluded_island_count === 1 ? 'island has' : 'islands have'} no voltage source.
			</p>{/if}
		<p>
			{scope.excluded_bus_count}
			{scope.excluded_bus_count === 1 ? 'bus' : 'buses'} and {scope.excluded_equipment_count}
			equipment
			{scope.excluded_equipment_count === 1 ? 'record' : 'records'}
			{preview
				? 'will be excluded when you solve the energized network'
				: 'are excluded from the calculation'}
			because their {scope.excluded_island_count === 1 ? 'island has' : 'islands have'} no voltage source.
			The full network remains available for inspection. Excluded equipment has no calculated result;
			its voltage and power are not reported as zero.
		</p>
		{#if scope.excluded_load_count}<p>
				{scope.excluded_load_count}
				{scope.excluded_load_count === 1 ? 'load has' : 'loads have'} no voltage source and {preview
					? 'will be omitted'
					: scope.excluded_load_count === 1
						? 'is absent'
						: 'are absent'} from solved totals.
			</p>{/if}
		{#if islands.length}
			<details>
				<summary
					>{preview
						? 'Islands and equipment to exclude'
						: 'Excluded islands and equipment'}</summary
				>
				{#each islands as island, i}
					<h4>Island {i + 1}: no voltage source</h4>
					<p>Buses: {island.buses.join(', ')}</p>
					<ul>
						{#each island.equipment as item}<li>
								{item.kind}: {item.name}
							</li>{/each}
					</ul>
				{/each}
			</details>
		{/if}
	</aside>
{/if}

<style>
	aside {
		margin: 0.75rem 0;
		padding: 0.65rem;
		border: 1px solid var(--line);
		border-left: 4px solid #b7791f;
		border-radius: 4px;
		font-size: 12px;
		overflow-wrap: anywhere;
	}
	p {
		margin: 0.4rem 0;
		line-height: 1.5;
	}
	summary {
		cursor: pointer;
	}
	details {
		max-height: 20rem;
		overflow: auto;
	}
	h4 {
		margin: 0.75rem 0 0.25rem;
	}
</style>
