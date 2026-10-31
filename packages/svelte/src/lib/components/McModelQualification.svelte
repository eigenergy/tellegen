<script lang="ts">
	import { mcLoadModelLabel, type McLoadModelAssessment } from '@tellegen/engine';
	let { model, compact = false }: { model?: McLoadModelAssessment | null; compact?: boolean } =
		$props();
</script>

<aside class:warning={model?.status !== 'original'} aria-label="Load-model qualification">
	<strong>{mcLoadModelLabel(model)}</strong>
	{#if model?.status === 'modified'}
		<p>
			The calculation converged using modified load equations on {model.modified_branch_count}
			load {model.modified_branch_count === 1 ? 'branch' : 'branches'}. These values are not a
			verified solution of the original load model. The qualification is retained when saving or
			exporting.
		</p>
	{:else if model?.status !== 'original'}
		<p>
			These results have no verified solution of the original load equations. Rerun to assess the
			load model.
		</p>
	{/if}
	{#if model && !compact}
		<details>
			<summary>Model assessment and settings used</summary>
			<p>
				Original-model KCL residual: {model.original_kcl_residual === null
					? 'undefined at this voltage'
					: `${model.original_kcl_residual.toExponential(3)} A`}. Scaled residual: {model.original_scaled_kcl_residual ===
				null
					? 'unavailable'
					: model.original_scaled_kcl_residual.toPrecision(4)} (at most 1 passes).
			</p>
			<p>
				Load-model envelope {model.options.voltage_envelope ? 'enabled' : 'disabled'}; Vlow {model
					.options.v_low_pu}, Vmin {model.options.v_min_pu}, Vmax {model.options.v_max_pu} pu. These thresholds
				change load equations; they are not operating voltage limits.
			</p>
			<p>
				Maximum iterations {model.options.max_iterations}; damping {model.options.damping};
				voltage-change tolerance {model.options.tolerance} V; zero-voltage tolerance {model.options
					.zero_voltage_tolerance} V; KCL tolerances {model.options.absolute_kcl_tolerance} A absolute
				and {model.options.relative_kcl_tolerance} relative.
			</p>
		</details>
	{/if}
</aside>

<style>
	aside {
		margin: 0.75rem 0;
		padding: 0.65rem;
		border: 1px solid var(--line);
		border-radius: 4px;
		font-size: 12px;
	}
	.warning {
		border-left: 4px solid #b7791f;
	}
	p {
		margin: 0.4rem 0;
		line-height: 1.5;
	}
	summary {
		cursor: pointer;
		margin-top: 0.4rem;
	}
</style>
