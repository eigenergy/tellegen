<script lang="ts">
	import { getController } from '$lib/context.svelte';
	import { formulationLabel, solveMetaLabel } from '$lib/format';
	import Sparkline from '$lib/Sparkline.svelte';

	const ctrl = getController();
</script>

{#if ctrl.activeSolvable && (ctrl.activeSolvable.solving || ctrl.activeSolvable.solveMs != null)}
	<div class="solvecard">
		<div class="solvecard-head mono">
			<span><b>OPF solve</b></span>
		</div>
		{#if (ctrl.activeSolvable.iterations ?? []).length > 1}
			<Sparkline iterations={ctrl.activeSolvable.iterations ?? []} />
		{/if}
		<div class="solve-meta mono dim">
			<span class="solve-formulation">{formulationLabel(ctrl.activeSolvable.formulation)}</span>
			<span>{solveMetaLabel(ctrl.activeSolvable)}</span>
			{#if ctrl.activeSolvable.solveMs != null}<span>{ctrl.activeSolvable.solveMs} ms</span>{/if}
		</div>
		{#if ctrl.activeSolvable.solveBackend === 'rust-server' && ctrl.activeSolvable.solveFallbackReason}
			<p class="fallback-reason mono dim" title={ctrl.activeSolvable.solveFallbackReason}>
				fallback: {ctrl.activeSolvable.solveFallbackReason}
			</p>
		{/if}
	</div>
{/if}

<style>
	.solvecard {
		position: absolute;
		top: 64px;
		right: 20px;
		z-index: 10;
		width: 240px;
		padding: 12px 14px 10px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.3s ease-out both;
	}

	.solvecard-head {
		font-size: 10.5px;
		margin-bottom: 6px;
		white-space: nowrap;
	}

	.solve-meta {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 12px;
		font-size: 10px;
		margin-top: 4px;
	}

	.solve-formulation {
		color: var(--accent);
	}

	.fallback-reason {
		margin: 6px 0 0;
		font-size: 10px;
		line-height: 1.35;
		overflow-wrap: anywhere;
	}

	@media (max-width: 760px) {
		.solvecard {
			top: 124px;
			left: auto;
			right: 10px;
			width: min(230px, calc(100% - 20px));
		}
	}
</style>
