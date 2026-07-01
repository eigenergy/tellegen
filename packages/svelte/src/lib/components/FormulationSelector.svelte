<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { formulationHint } from '../format.js';
	import { FORMULATIONS, type Formulation } from '@tellegen/engine';

	const ctrl = getController();
</script>

{#if ctrl.activeSolvable}
	{@const c = ctrl.activeSolvable}
	<div class="formulation">
		<label
			class="formulation-row mono"
			for="formulation-select"
			title={formulationHint(c.formulation)}
		>
			<span>formulation</span>
			<select
				id="formulation-select"
				class="mono"
				disabled={c.solving}
				value={c.formulation}
				onchange={(e) => ctrl.changeFormulation(c, e.currentTarget.value as Formulation)}
			>
				{#each FORMULATIONS as f (f.id)}
					<option value={f.id} disabled={f.disabled}>
						{f.label}{f.disabled ? ' (coming soon)' : ''}
					</option>
				{/each}
			</select>
		</label>
	</div>
{/if}

<style>
	.formulation {
		margin-top: 10px;
	}

	.formulation-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: 12px;
		color: var(--ink);
	}

	.formulation-row select {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 3px 22px 3px 8px;
		border: 1px solid var(--line);
		border-radius: 2px;
		background: var(--surface-control);
		color: var(--ink);
		cursor: pointer;
		/* Native arrow on the right, drawn so the control reads as a control in the panel. */
		appearance: none;
		-webkit-appearance: none;
		background-image:
			linear-gradient(45deg, transparent 50%, var(--ink-dim) 50%),
			linear-gradient(135deg, var(--ink-dim) 50%, transparent 50%);
		background-position:
			right 10px center,
			right 6px center;
		background-size:
			4px 4px,
			4px 4px;
		background-repeat: no-repeat;
	}

	.formulation-row select:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--text-accent);
	}

	.formulation-row select:disabled {
		opacity: 0.55;
		cursor: progress;
	}
</style>
