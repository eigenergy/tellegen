<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { formulationLabel, solveMetaLabel } from '../format.js';
	import Sparkline from '../Sparkline.svelte';

	const ctrl = getController();
	const active = $derived(ctrl.activeSolvable);
	const iterations = $derived(active?.iterations ?? []);
</script>

{#if active && (active.solving || active.solveMs != null)}
	<div class="solvecard">
		<div class="solvecard-head mono">
			<span><b>OPF solve</b></span>
		</div>
		{#if iterations.length > 1}
			{#key iterations}
				<Sparkline {iterations} />
			{/key}
		{/if}
		<div class="solve-meta mono dim">
			<span class="solve-formulation">{formulationLabel(active.formulation)}</span>
			<span>{solveMetaLabel(active)}</span>
			{#if active.solveMs != null}<span>{active.solveMs} ms</span>{/if}
		</div>
		{#if active.solveBackend === 'rust-server' && active.solveFallbackReason}
			<p class="fallback-reason mono dim" title={active.solveFallbackReason}>
				fallback: {active.solveFallbackReason}
			</p>
		{/if}
	</div>
{/if}

<style>
	.solvecard {
		position: absolute;
		top: 76px;
		right: 20px;
		z-index: 10;
		width: 300px;
		padding: 13px 15px 11px;
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
		gap: 10px 14px;
		font-size: 10px;
		margin-top: 6px;
	}

	.solve-formulation {
		color: var(--text-accent);
	}

	.fallback-reason {
		margin: 6px 0 0;
		font-size: 10px;
		line-height: 1.35;
		overflow-wrap: anywhere;
	}

	@media (max-width: 760px) {
		.solvecard {
			top: 184px;
			left: auto;
			right: 10px;
			width: min(300px, calc(100% - 20px));
		}
	}
</style>
