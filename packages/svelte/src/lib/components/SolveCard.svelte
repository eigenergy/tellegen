<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { formulationLabel } from '../format.js';
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
	/** Named only when a server took part; the browser engine is the default. */
	const server = $derived(
		active?.solveBackend === 'rust-server'
			? 'Server'
			: active?.solveBackend === 'clarabel-wasm-server-sensitivity'
				? 'Server price sensitivity'
				: null
	);
</script>

{#if saved || (active && (active.solving || active.solveMs != null))}
	<PanelFrame id="solver" title="Solver" side="right" order={10} width={320} bind:open>
		<div class="solvecard">
			{#if iterations.length > 1}
				{#key iterations}
					<Sparkline {iterations} />
				{/key}
			{/if}
			{#if saved}
				<dl>
					<div>
						<dt>Calculation</dt>
						<dd>{formulationLabel(saved.formulation)}</dd>
					</div>
					<div>
						<dt>Result</dt>
						<dd>{saved.solution?.status ?? 'Not solved'}</dd>
					</div>
					<div>
						<dt>Saved state</dt>
						<dd>{saved.label}</dd>
					</div>
				</dl>
			{:else if active}
				<dl>
					<div>
						<dt>Calculation</dt>
						<dd>{formulationLabel(active.formulation)}</dd>
					</div>
					{#if iterations.length > 1}
						<div>
							<dt>Iterations</dt>
							<dd class="mono">{iterations.length}</dd>
						</div>
					{/if}
					{#if active.solveDetail}
						<div>
							<dt>Solve</dt>
							<dd>{active.solveDetail}</dd>
						</div>
					{/if}
					{#if server}
						<div>
							<dt>Solver</dt>
							<dd>{server}</dd>
						</div>
					{/if}
					{#if active.solveMs != null}
						<div>
							<dt>Time</dt>
							<dd class="mono">{active.solveMs} ms</dd>
						</div>
					{/if}
					{#if active.solveBackend === 'rust-server' && active.solveFallbackReason}
						<div>
							<dt>Fallback</dt>
							<dd title={active.solveFallbackReason}>{active.solveFallbackReason}</dd>
						</div>
					{/if}
				</dl>
			{/if}
		</div>
	</PanelFrame>
{/if}

<style>
	.solvecard {
		padding: 13px 15px 11px;
	}

	dl {
		margin: 6px 0 0;
		font-size: 12px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 2px 0;
	}

	dt {
		flex: none;
		color: var(--text-secondary);
	}

	dd {
		min-width: 0;
		margin: 0;
		text-align: right;
		overflow-wrap: anywhere;
	}
</style>
