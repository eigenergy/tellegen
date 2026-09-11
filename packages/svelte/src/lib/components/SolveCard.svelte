<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { formulationLabel, solveMetaLabel } from '../format.js';
	import Sparkline from '../Sparkline.svelte';
	import PanelFrame from './PanelFrame.svelte';
	let open = $state(true);

	const ctrl = getController();
	const app = getAppState();
	const saved = $derived(app.studyView);
	const active = $derived(ctrl.activeSolvable);
	const iterations = $derived(
		saved
			? Array.isArray(saved.solution?.iterations)
				? saved.solution.iterations
				: []
			: (active?.iterations ?? [])
	);
</script>

{#if saved || (active && (active.solving || active.solveMs != null))}
	<PanelFrame id="solver" title="Solver" side="right" order={10} width={320} bind:open>
		<div class="solvecard">
			<div class="solve-label mono">
				{saved ? 'Saved result' : active?.formulation === 'acpf' ? 'Power flow' : 'OPF solve'}
			</div>
			{#if iterations.length > 1}
				{#key iterations}
					<Sparkline {iterations} />
				{/key}
			{/if}
			{#if saved}
				<div class="solve-meta mono dim">
					<span class="solve-formulation">{formulationLabel(saved.formulation)}</span>
					<span>{saved.solution?.status ?? 'Not solved'}</span>
					<span>Saved state: {saved.label}</span>
				</div>
			{:else if active}
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
			{/if}
		</div>
	</PanelFrame>
{/if}

<style>
	.solve-label {
		font-size: 11px;
		margin-bottom: 8px;
	}
	.solvecard {
		padding: 13px 15px 11px;
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
</style>
