<script lang="ts">
	import type { McLoadModelAssessment } from '@tellegen/engine';
	let { model, compact = false }: { model?: McLoadModelAssessment | null; compact?: boolean } =
		$props();
</script>

<aside class:warning={model?.status !== 'original'} aria-label="Load-model qualification">
	<strong
		>{model?.status === 'original'
			? 'Load model verified'
			: model?.status === 'modified'
				? `Load envelope used (${model.modified_branch_count})`
				: 'Unverified'}</strong
	>
	{#if model && !compact}
		<details>
			<summary>Settings used</summary>
			<dl>
				<div>
					<dt>Residual</dt>
					<dd>
						{model.original_kcl_residual === null
							? '—'
							: `${model.original_kcl_residual.toExponential(3)} A`}
					</dd>
				</div>
				<div>
					<dt>Load envelope</dt>
					<dd>
						{model.options.voltage_envelope ? 'On' : 'Off'}, {model.options.v_low_pu}/{model.options
							.v_min_pu}/{model.options.v_max_pu} pu
					</dd>
				</div>
				<div>
					<dt>Max iterations</dt>
					<dd>{model.options.max_iterations}</dd>
				</div>
				<div>
					<dt>Damping</dt>
					<dd>{model.options.damping}</dd>
				</div>
				<div>
					<dt>Voltage tolerance</dt>
					<dd>{model.options.tolerance} V</dd>
				</div>
				<div>
					<dt>Current tolerance</dt>
					<dd>{model.options.absolute_kcl_tolerance} A</dd>
				</div>
			</dl>
		</details>
	{/if}
</aside>

<style>
	aside {
		margin: 0.75rem 0;
		padding: 0.65rem;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		font-size: 12px;
	}
	.warning {
		border-left: 4px solid var(--warn);
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
		margin-top: 0.4rem;
	}
</style>
