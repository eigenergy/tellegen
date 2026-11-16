<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import {
		cleanStatus,
		duration,
		iterationCount,
		savedVerdict,
		solveVerdict,
		statusWords
	} from '../diagram/verdict.js';
	import { formulationLabel } from '../format.js';
	import Sparkline from '../Sparkline.svelte';
	import PanelFrame from './PanelFrame.svelte';

	/** Render inside another panel (the shell puts it in the Network panel)
	 * instead of registering a Solver panel of its own. */
	let { embedded = false }: { embedded?: boolean } = $props();
	let open = $state(true);

	const ctrl = getController();
	const app = getAppState();
	const saved = $derived(app.studyView);
	const active = $derived(ctrl.activeSolvable);
	/** A calculation the user chose that failed, while the previous result stays on screen. */
	const attempted = $derived(saved || active?.solving ? null : (active?.failedCalculation ?? null));
	/** The convergence record: an interior-point trace (an array) or, for AC power flow, the
	 * Newton summary the engine returns in its place. */
	const record = $derived<unknown>(
		saved ? saved.solution?.iterations : attempted ? [] : (active?.iterations ?? [])
	);
	const trace = $derived(Array.isArray(record) ? record : []);
	const count = $derived(iterationCount(record));
	/** A solve started (it picked a backend) but ended without a time: it failed. */
	const failed = $derived(
		!!attempted ||
			(!!active && !active.solving && active.solveBackend !== null && active.solveMs == null)
	);
	const verdict = $derived(
		saved
			? savedVerdict({ status: saved.solution?.status })
			: active
				? solveVerdict({
						formulation: attempted ?? active.formulation,
						solving: active.solving,
						failed,
						status: attempted ? null : active.solution?.status,
						failure: failed ? app.error : null
					})
				: null
	);
	/** Named only when a server took part; the browser engine is the default. */
	const server = $derived(
		!attempted &&
			(active?.solveBackend === 'rust-server' ||
				active?.solveBackend === 'clarabel-wasm-server-sensitivity')
			? 'Server'
			: null
	);
	const fallback = $derived(
		!attempted && active?.solveBackend === 'rust-server' ? active.solveFallbackReason : null
	);
	const status = $derived(saved ? saved.solution?.status : active?.solution?.status);
	/** The calculation behind the result on screen. */
	const calculation = $derived(saved ? saved.formulation : (active?.formulation ?? null));
	/** A power flow that solves is "Converged"; "Feasible" is an optimiser's word. */
	const statusText = $derived(
		calculation === 'acpf' && cleanStatus(status) ? 'Converged' : statusWords(status)
	);
	const ms = $derived(saved || attempted ? null : (active?.solveMs ?? null));
	/** The served base result of a hosted case: solved ahead of time, so it has no
	 * iterations or time of its own. */
	const precomputed = $derived(
		!saved &&
			!attempted &&
			!!active?.solution &&
			!active.solving &&
			active.solution === active.baseSolution &&
			!active.solveBackend &&
			count === null &&
			active.solveMs == null
	);
	const failure = $derived(failed ? app.error : null);
	const hasDetails = $derived(
		!!failure ||
			trace.length > 1 ||
			!!status ||
			count !== null ||
			ms !== null ||
			!!server ||
			(!attempted && !!active?.solveDetail)
	);
</script>

{#snippet card()}
	<div
		class="solvecard"
		data-calculation={active?.solving ? undefined : (calculation ?? undefined)}
	>
		<!-- In the Network panel the Calculation row already says "Solving…". -->
		{#if verdict && !(embedded && verdict.tone === 'pending' && !saved)}
			<p class={['verdict', verdict.tone]} role="status">{verdict.text}</p>
		{/if}
		{#if hasDetails && !active?.solving}
			<details class="internals">
				<summary>Details</summary>
				{#if trace.length > 1}
					{#key trace}
						<Sparkline iterations={trace} />
					{/key}
				{/if}
				<dl>
					{#if calculation && (saved || !embedded)}
						<div>
							<dt>Calculation</dt>
							<dd>{formulationLabel(attempted ?? calculation)}</dd>
						</div>
					{/if}
					{#if status && !failed}
						<div>
							<dt>Status</dt>
							<dd>{statusText}</dd>
						</div>
					{/if}
					{#if count !== null}
						<div>
							<dt>Iterations</dt>
							<dd class="num">{count}</dd>
						</div>
					{/if}
					{#if ms !== null}
						<div>
							<dt>Time</dt>
							<dd class="num">{duration(ms)}</dd>
						</div>
					{:else if precomputed}
						<div>
							<dt>Source</dt>
							<dd>Precomputed</dd>
						</div>
					{/if}
					{#if !saved && !attempted && active?.solveDetail}
						<div>
							<dt>Method</dt>
							<dd>{active.solveDetail}</dd>
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
							<dd>{fallback}</dd>
						</div>
					{/if}
				</dl>
				{#if failure}<pre class="failure-text">{failure}</pre>{/if}
			</details>
		{/if}
	</div>
{/snippet}

{#if saved || (active && (active.solving || hasDetails))}
	{#if embedded}
		<section class="embedded" aria-label="Solver">{@render card()}</section>
	{:else}
		<PanelFrame id="solver" title="Solver" side="right" order={10} width={320} bind:open>
			{@render card()}
		</PanelFrame>
	{/if}
{/if}

<style>
	.solvecard {
		padding: var(--space-lg) var(--space-xl) var(--space-md);
	}

	/* In the Network panel the solver's line is a quiet status under the numbers. */
	.embedded .verdict {
		font-weight: 400;
		color: var(--text-secondary);
	}
	.embedded .verdict.failure {
		font-weight: 600;
		color: var(--text-danger);
	}

	.embedded .solvecard {
		margin: 4px 0 12px;
		padding: 0;
	}

	.verdict {
		margin: 0 0 var(--space-sm);
		font-size: var(--fs-body);
		font-weight: 600;
		line-height: var(--lh-snug);
		color: var(--text-primary);
		text-wrap: pretty;
	}

	.verdict.failure {
		color: var(--text-danger);
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

	.embedded .internals {
		margin-top: 0;
		border-top: 0;
		padding-top: 0;
	}

	.failure-text {
		margin: var(--space-xs) 0 0;
		font: var(--fs-label) / var(--lh-snug) var(--font-code);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		color: var(--text-secondary);
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
