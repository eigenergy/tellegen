<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { dollarUnits } from '../display.js';
	import { signedSmall } from '../format.js';

	const ctrl = getController();
	const units = $derived(
		dollarUnits(ctrl.selectedSensitivity?.units ?? '($/MWh)/MW').replace(
			/\(\$\/MWh\)\/(MW|MVA)\b/,
			'$$/MWh per $1'
		)
	);
</script>

<div class="movers-block">
	{#if !ctrl.previewing && ctrl.topMovers.length > 0}
		<table>
			<caption class="dim">
				Top price changes <span class="unit">({units})</span>
			</caption>
			<tbody>
				{#each ctrl.topMovers as mover (mover.bus)}
					<tr>
						<td>Bus <span class="mono">{mover.bus}</span></td>
						<td class={['mono', { pos: mover.value > 0, neg: mover.value < 0 }]}>
							{signedSmall(mover.value)}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>

<style>
	.movers-block {
		min-height: 114px;
	}

	table {
		width: 100%;
		margin-top: var(--space-lg);
		border-collapse: collapse;
		font-size: var(--fs-label);
	}

	caption {
		text-align: left;
		font-size: var(--fs-data);
		margin-bottom: var(--space-2xs);
	}

	.unit {
		white-space: nowrap;
	}

	td {
		padding: var(--space-3xs) 0;
		border-top: 1px solid var(--line);
	}

	td:last-child {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	.pos {
		color: var(--pos);
	}

	.neg {
		color: var(--neg);
	}
</style>
