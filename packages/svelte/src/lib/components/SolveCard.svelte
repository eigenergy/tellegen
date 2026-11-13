<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { formulationLabel } from '../format.js';
	import { iterationCount, savedVerdict, solveVerdict, statusWords } from '../diagram/verdict.js';
	import Sparkline from '../Sparkline.svelte';
	import PanelFrame from './PanelFrame.svelte';
	let open = $state(true);

	const ctrl = getController();
	const app = getAppState();
	const saved = $derived(app.studyView);
	const active = $derived(ctrl.activeSolvable);
	/** The convergence record: an interior-point trace (an array) or, for AC power flow, the
	 * Newton summary the engine returns in its place. */
	const record = $derived<unknown>(saved ? saved.solution?.iterations : (active?.iterations ?? []));
	const trace = $derived(Array.isArray(record) ? record : []);
	const count = $derived(iterationCount(record));
	/** A solve started (it picked a backend) but ended without a time: it failed. */
	const failed = $derived(
		!!active && !active.solving && active.solveBackend !== null && active.solveMs == null
	);
	const verdict = $derived(
		saved
			? savedVerdict({
					formulation: saved.formulation,
					status: saved.solution?.status,
					label: saved.label
				})
			: active
				? solveVerdict({
						formulation: active.formulation,
						solving: active.solving,
						failed,
						status: active.solution?.status,
						failure: failed ? app.error : null,
						iterations: count,
						ms: active.solveMs
					})
				: null
	);
	/** Named only when a server took part; the browser engine is the default. */
	const server = $derived(
		active?.solveBackend === 'rust-server'
			? 'Server'
			: active?.solveBackend === 'clarabel-wasm-server-sensitivity'
				? 'Server price sensitivity'
				: null
	);
	const fallback = $derived(
		active?.solveBackend === 'rust-server' ? active.solveFallbackReason : null
	);
	const status = $derived(saved ? saved.solution?.status : active?.solution?.status);
	const hasDetails = $derived(
		trace.length > 1 || !!saved || !!server || !!fallback || (!!status && !failed)
	);
</script>

{#if saved || (active && (active.solving || active.solveMs != null || failed))}
	<PanelFrame id="solver" title="Solver" side="right" order={10} width={320} bind:open>
		<div class="solvecard">
			{#if verdict}
				<p class={['verdict', verdict.tone]} role="status">{verdict.text}</p>
			{/if}
			{#if saved}
				<dl>
					<div>
						<dt>Calculation</dt>
						<dd>{formulationLabel(saved.formulation)}</dd>
					</div>
					<div>
						<dt>Result</dt>
						<dd>{statusWords(saved.solution?.status)}</dd>
					</div>
				</dl>
			{:else if active}
				<dl>
					<div>
						<dt>Calculation</dt>
						<dd>{formulationLabel(active.formulation)}</dd>
					</div>
					{#if count !== null && count > 1}
						<div>
							<dt>Iterations</dt>
							<dd class="num">{count}</dd>
						</div>
					{/if}
					{#if active.solveDetail}
						<div>
							<dt>Solve</dt>
							<dd>{active.solveDetail}</dd>
						</div>
					{/if}
					{#if active.solveMs != null}
						<div>
							<dt>Time</dt>
							<dd class="num">{active.solveMs} ms</dd>
						</div>
					{/if}
				</dl>
			{/if}
			{#if hasDetails}
				<details class="internals">
					<summary>Solver details</summary>
					{#if trace.length > 1}
						{#key trace}
							<Sparkline iterations={trace} />
						{/key}
					{/if}
					<dl>
						{#if status && !failed}
							<div>
								<dt>Solver status</dt>
								<dd class="num">{status}</dd>
							</div>
						{/if}
						{#if saved}
							<div>
								<dt>Saved state</dt>
								<dd>{saved.label}</dd>
							</div>
						{/if}
						{#if server}
							<div>
								<dt>Solver</dt>
								<dd>{server}</dd>
							</div>
						{/if}
						{#if fallback}
							<div>
								<dt>Fallback</dt>
								<dd title={fallback}>{fallback}</dd>
							</div>
						{/if}
					</dl>
				</details>
			{/if}
		</div>
	</PanelFrame>
{/if}

<style>
	.solvecard {
		padding: var(--space-lg) var(--space-xl) var(--space-md);
	}

	.verdict {
		margin: 0 0 var(--space-sm);
		padding-left: var(--space-sm);
		border-left: 3px solid var(--line);
		font-size: var(--fs-body);
		font-weight: 600;
		line-height: var(--lh-snug);
		color: var(--text-primary);
		text-wrap: pretty;
	}

	.verdict.ok {
		border-left-color: var(--status-ok);
	}

	.verdict.warning {
		border-left-color: var(--accent);
	}

	.verdict.failure {
		border-left-color: var(--text-danger);
	}

	.verdict.pending {
		font-weight: 400;
		color: var(--text-secondary);
	}

	dl {
		margin: 0;
		font-size: var(--fs-label);
	}

	dl div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-lg);
		padding: var(--space-3xs) 0;
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

	.num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
	}

	.internals {
		margin-top: var(--space-sm);
		border-top: 1px solid var(--line);
		padding-top: var(--space-xs);
	}

	summary {
		cursor: pointer;
		font-size: var(--fs-label);
		color: var(--text-secondary);
		padding: var(--space-3xs) 0;
	}

	summary:hover {
		color: var(--text-primary);
	}

	.internals[open] summary {
		margin-bottom: var(--space-xs);
	}
</style>
