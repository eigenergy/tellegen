<script lang="ts">
	import type { ControlResult } from '@tellegen/engine';
	let { controls }: { controls: ControlResult[] } = $props();
	const number = (n: number) => Number(n.toPrecision(6));
</script>

{#if controls.length}
	<section aria-label="Regulator results">
		<h4>Regulator outcomes</h4>
		<p>
			A reached target is inside its deadband. At a limit, the setting stays bounded and the target
			may be missed. Discrete steps may straddle a narrow deadband.
		</p>
		<ul>
			{#each controls as c (c.id)}
				<li>
					<strong>{c.id}</strong>: {c.status.replaceAll('_', ' ')}<br />Setting {number(c.setting)}
					{c.units}; measured {number(c.measured)}
					{c.measured_units}, target {number(c.target_low)}–{number(c.target_high)}.
				</li>
			{/each}
		</ul>
	</section>
{/if}

<style>
	ul {
		padding-left: 18px;
		overflow-wrap: anywhere;
	}
	li {
		margin-block: 8px;
	}
</style>
