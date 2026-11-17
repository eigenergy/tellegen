<script lang="ts">
	import type { ControlResult } from '@tellegen/engine';
	import { sixFigures } from '../../format.js';
	let { controls }: { controls: ControlResult[] } = $props();
</script>

{#if controls.length}
	<section aria-label="Regulator results">
		<h4>Regulators</h4>
		<ul>
			{#each controls as c (c.id)}
				<li>
					<strong>{c.id}</strong>: {c.status.replaceAll('_', ' ')}
					<span class="reading"
						>Setting {sixFigures(c.setting)}
						{c.units}, measured {sixFigures(c.measured)}
						{c.measured_units}, target {sixFigures(c.target_low)}–{sixFigures(c.target_high)}</span
					>
				</li>
			{/each}
		</ul>
	</section>
{/if}

<style>
	ul {
		padding-left: var(--space-xl);
		overflow-wrap: anywhere;
	}
	li {
		margin-block: var(--space-sm);
	}
	.reading {
		display: block;
		font-variant-numeric: tabular-nums;
	}
</style>
