<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { signedExp } from '../format.js';

	const ctrl = getController();
</script>

<div class="movers-block">
	{#if !ctrl.previewing && ctrl.topMovers.length > 0}
		<table class="mono">
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
