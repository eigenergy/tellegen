<script lang="ts">
	import type { McNetworkScope as NetworkScope, McExcludedIsland } from '@tellegen/engine';
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
		<strong>{preview ? 'Unpowered islands' : 'Energized network only'}</strong>
		<dl>
			<div>
				<dt>Islands</dt>
				<dd>{scope.excluded_island_count}</dd>
			</div>
			<div>
				<dt>Excluded buses</dt>
				<dd>{scope.excluded_bus_count}</dd>
			</div>
			<div>
				<dt>Excluded equipment</dt>
				<dd>{scope.excluded_equipment_count}</dd>
			</div>
			{#if scope.excluded_load_count}<div>
					<dt>Excluded loads</dt>
					<dd>{scope.excluded_load_count}</dd>
				</div>{/if}
		</dl>
		{#if islands.length}
			<details>
				<summary>Excluded islands</summary>
				{#each islands as island, i (i)}
					<h4>Island {i + 1}</h4>
					<p>Buses: {island.buses.join(', ')}</p>
					<ul>
						{#each island.equipment as item, k (k)}<li>
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
		border-left: 4px solid var(--warn);
		border-radius: var(--radius-control);
		font-size: 12px;
		overflow-wrap: anywhere;
	}
	p {
		margin: 0.4rem 0;
	}
	dl {
		margin: 0.4rem 0 0;
	}
	dl div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
	}
	dt {
		color: var(--text-secondary);
	}
	dd {
		margin: 0;
		font-variant-numeric: tabular-nums;
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
