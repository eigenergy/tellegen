<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { signedExp } from '../format.js';

	const ctrl = getController();
	const units = $derived(
		(ctrl.selectedSensitivity?.units ?? '(objective units/MW)/MW').replaceAll(
			'objective_unit',
			'objective units'
		)
	);
	const unitTitle = $derived(`Nodal value response per MW of demand, in ${units}.`);
</script>

<div class="movers-block">
	{#if !ctrl.previewing && ctrl.topMovers.length > 0}
		<table class="mono">
			<caption class="mono dim" title={unitTitle}>
				largest nodal value response <span class="unit">{units}</span>
			</caption>
			<tbody>
				{#each ctrl.topMovers as mover (mover.bus)}
					<tr>
						<td>bus {mover.bus}</td>
						<td class:pos={mover.value > 0} class:neg={mover.value < 0}>
							{signedExp(mover.value)}
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
		margin-top: 12px;
		border-collapse: collapse;
		font-size: 12px;
	}

	caption {
		text-align: left;
		font-size: 10.5px;
		margin-bottom: 4px;
	}

	.unit {
		white-space: nowrap;
	}

	td {
		padding: 3px 0;
		border-top: 1px solid var(--line);
	}

	td:last-child {
		text-align: right;
	}

	.pos {
		color: var(--pos);
	}

	.neg {
		color: var(--neg);
	}
</style>
