<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { FORMULATIONS, type Formulation } from '@tellegen/engine';

	const ctrl = getController();
</script>

{#if ctrl.activeSolvable}
	{@const c = ctrl.activeSolvable}
	{@const chosen = c.solving ? (c.settings.calculation ?? c.formulation) : c.formulation}
	<!-- While a calculation runs, the select names it; otherwise the one on screen. -->
	<div class="formulation">
		<label class="formulation-row" for="formulation-select">
			<span>Calculation</span>
			<select
				id="formulation-select"
				disabled={c.solving}
				value={chosen}
				onchange={(e) => void ctrl.chooseCalculation(c, e.currentTarget.value as Formulation)}
			>
				{#each FORMULATIONS as f (f.id)}
					<option value={f.id} disabled={!ctrl.calculationOffered(c, f.id)}>
						{f.label}
					</option>
				{/each}
			</select>
		</label>
		{#if c.solving}
			<p class="solving" role="status">
				<span>
					{ctrl.acOpfLoading ? 'Loading…' : 'Solving…'}
				</span>
				{#if ctrl.canCancelSettings(c)}
					<button class="cancel" onclick={() => ctrl.cancelSettings(c)}>Cancel</button>
				{/if}
			</p>
		{/if}
	</div>
{/if}

<style>
	.formulation {
		display: grid;
		gap: 6px;
		margin-top: 10px;
	}

	.formulation-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: var(--fs-body);
		color: var(--ink);
		white-space: nowrap;
	}

	.formulation-row select {
		min-width: 0;
		font-size: var(--fs-body);
	}

	.formulation-row select:disabled {
		opacity: 0.55;
		cursor: progress;
	}

	.solving {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 0;
		font-size: var(--fs-body);
		color: var(--text-secondary);
	}

	.cancel {
		font: inherit;
		padding: 2px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		cursor: pointer;
	}

	.cancel:hover {
		border-color: var(--border-strong);
	}

	@media (hover: none), (pointer: coarse) {
		/* The select is sized to its longest option, which runs off a 320px screen.
		   Let it shrink around the current value rather than truncate the label.
		   16px also keeps Safari from zooming the layout viewport on focus. */
		.formulation-row select {
			flex: 0 1 auto;
			min-width: 0;
			min-height: 44px;
			font-size: 16px;
			padding-block: 6px;
		}

		.formulation-row > span {
			flex: 0 0 auto;
		}

		.cancel {
			min-height: 44px;
		}
	}
</style>
