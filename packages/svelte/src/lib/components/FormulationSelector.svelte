<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { formulationHint } from '../format.js';
	import { FORMULATIONS, type Formulation, type DcFormula } from '@tellegen/engine';

	const ctrl = getController();
</script>

{#if ctrl.activeSolvable}
	{@const c = ctrl.activeSolvable}
	<div class="formulation">
		<label class="formulation-row" for="formulation-select" title={formulationHint(c.formulation)}>
			<span>Calculation</span>
			<select
				id="formulation-select"
				disabled={c.solving}
				value={c.settings.calculation ?? c.formulation}
				onchange={(e) =>
					void ctrl
						.updateSettings(c, e.currentTarget.value as Formulation, {}, c.settings.revision)
						.catch((error) => {
							ctrl.app.error = String(error);
						})}
			>
				{#each FORMULATIONS as f (f.id)}
					{@const unavailable =
						(f.disabled && !(f.id === 'acopf' && ctrl.acOpfAvailable && !c.perturbed)) ||
						('declaredFormulation' in c &&
							c.declaredFormulation != null &&
							c.declaredFormulation !== f.id)}
					<option value={f.id} disabled={unavailable}>
						{f.label}{unavailable ? ' (unavailable)' : ''}
					</option>
				{/each}
			</select>
		</label>
		{#if c.formulation === 'dcopf'}
			<label class="formulation-row" for="dc-variant-select">
				<span>DC variant</span>
				<select
					id="dc-variant-select"
					disabled={c.solving || ('declaredFormulation' in c && !!c.declaredFormulation)}
					value={c.solution?.branch_susceptance_formula ??
						c.branchSusceptanceFormula ??
						'series_susceptance'}
					onchange={(e) => ctrl.changeDcFormula(c, e.currentTarget.value as DcFormula)}
				>
					<option value="series_susceptance">Legacy Tellegen (default)</option>
					<option value="series_susceptance_no_phase_shift">PowerModels DCP / PGLib DC</option>
					<option value="tap_adjusted_reactance">MATPOWER / PowerModels DCMP</option>
					<option value="tap_adjusted_series_susceptance">Tap-adjusted series susceptance</option>
					<option value="reactance_only">Reactance only (1/x)</option>
				</select>
			</label>
			<p class="variant-hint">
				Compatibility requires matching costs and limits. DC and AC optimal costs differ.
				<a
					href="https://eigenergy.github.io/tellegen/dc-compatibility.html"
					target="_blank"
					rel="noreferrer">Compatibility matrix</a
				>
			</p>
		{/if}
	</div>
{/if}

<style>
	.variant-hint {
		font-size: 12px;
		color: var(--ink-dim);
		margin: 6px 0;
	}
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
		font-family: inherit;
		font-size: 13px;
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
	}
</style>
