<script lang="ts">
	import { onMount } from 'svelte';
	import AgentGuide from './AgentGuide.svelte';
	import { PanelFrame, getAppState } from '@tellegen/svelte';
	import type {
		ExperimentRecord,
		JsonValue,
		TellegenToolActivityEvent,
		ToolResponse
	} from '@tellegen/webmcp';
	import {
		capacityPlanExactPhiDelta,
		capacityPlanPredictedPhiDelta
	} from './capacity-plan-metrics.js';
	import type { CapacityPlanActivity, PlanningActivityStore } from './planning-activity.svelte.js';
	import type { WebMcpBrowserState } from './browser-support.js';

	let {
		supported,
		registrationError,
		browser = null,
		pageToolCalls = true,
		toolCount = 0,
		activities,
		planning,
		experiments,
		exportJournal
	}: {
		studyExpanded: boolean;
		closeStudy: () => void;
		supported: boolean;
		registrationError: string | null;
		browser?: WebMcpBrowserState | null;
		pageToolCalls?: boolean;
		toolCount?: number;
		activities: TellegenToolActivityEvent[];
		planning: PlanningActivityStore;
		experiments: ExperimentRecord[];
		exportJournal: () => void;
	} = $props();

	let open = $state(false);
	let tab = $state<'activity' | 'connect'>('connect');
	const app = getAppState();
	let welcome = $state(false);
	/** The first-visit hint follows the Connect tab's status: tools to connect, a browser setting
	 * to turn on, or nothing where this browser cannot run them. */
	const hint = $derived.by(() => {
		if (registrationError) return null;
		if (supported || browser?.kind === 'ready')
			return { text: 'Agent tools available', action: 'Connect' };
		if (browser?.kind === 'flag' || browser?.kind === 'update' || browser?.kind === 'legacy')
			return { text: 'Agent tools are off', action: 'Set up' };
		return null;
	});
	const welcomeKey = 'tellegen.welcome.webmcp-v1';
	onMount(() => {
		try {
			welcome = localStorage.getItem(welcomeKey) !== 'seen';
		} catch {
			welcome = true;
		}
	});
	function dismiss() {
		welcome = false;
		try {
			localStorage.setItem(welcomeKey, 'seen');
		} catch {
			/* Dismissal applies to this visit. */
		}
	}
	// The welcome pill disappears once the panel opens, taking focus with it,
	// so "Show me" asks for the Connect tab to take focus when it is attached.
	let focusRequest = $state(0);
	let focusHandled = 0;
	function showConnect() {
		open = true;
		tab = 'connect';
		focusRequest += 1;
		dismiss();
	}
	function takeFocus(node: HTMLButtonElement) {
		if (focusRequest === focusHandled) return;
		focusHandled = focusRequest;
		node.focus();
	}
	function show() {
		open = true;
		tab = activities.length || planning.entries.length ? 'activity' : 'connect';
		dismiss();
	}

	function exactPhiDelta(entry: CapacityPlanActivity): number {
		return capacityPlanExactPhiDelta(entry.outcome);
	}

	function predictedPhiDelta(entry: CapacityPlanActivity): number {
		return capacityPlanPredictedPhiDelta(entry.outcome);
	}

	function isStaged(entry: CapacityPlanActivity): boolean {
		return planning.proposal?.activityId === entry.id;
	}

	function isApproved(entry: CapacityPlanActivity): boolean {
		const staged = planning.proposal;
		return !!staged && staged.activityId === entry.id && planning.isApproved(staged);
	}

	function finished(
		activity: TellegenToolActivityEvent
	): activity is Extract<TellegenToolActivityEvent, { type: 'finished' }> {
		return activity.type === 'finished';
	}

	function record(value: JsonValue | undefined): Record<string, JsonValue> | null {
		return value && typeof value === 'object' && !Array.isArray(value)
			? (value as Record<string, JsonValue>)
			: null;
	}

	function data(response: ToolResponse): Record<string, JsonValue> | null {
		return response.ok ? response.data : null;
	}

	function number(value: JsonValue | undefined): number | null {
		return typeof value === 'number' && Number.isFinite(value) ? value : null;
	}

	function formatNumber(value: number | null, digits = 2): string {
		return value === null
			? '—'
			: value.toLocaleString(undefined, { maximumFractionDigits: digits });
	}

	function formatDelta(value: number | null): string {
		if (value === null) return '—';
		return `${value >= 0 ? '+' : ''}${formatNumber(value)}`;
	}

	function elapsed(activity: TellegenToolActivityEvent): string {
		if (!finished(activity)) return '…';
		const ms = activity.finishedAt - activity.startedAt;
		return ms < 1_000 ? `${ms} ms` : `${(ms / 1_000).toFixed(1)} s`;
	}

	function comparison(activity: TellegenToolActivityEvent): {
		before: Record<string, JsonValue>;
		after: Record<string, JsonValue>;
	} | null {
		if (!finished(activity) || !activity.response.ok) return null;
		const result = data(activity.response);
		const before = record(result?.before);
		const after = record(result?.after);
		return before && after ? { before, after } : null;
	}

	function prediction(activity: TellegenToolActivityEvent): Record<string, JsonValue> | null {
		if (!finished(activity) || !activity.response.ok) return null;
		return record(data(activity.response)?.prediction);
	}
</script>

<PanelFrame id="agent" title="Agent" side="left" order={30} width={370} bind:open onopen={show}>
	<div class="activity-panel" data-webmcp-activity="open" aria-label="WebMCP activity">
		<nav aria-label="Agent panel">
			<button
				class={{ active: tab === 'activity' }}
				aria-pressed={tab === 'activity'}
				onclick={() => (tab = 'activity')}>Activity</button
			>
			<button
				{@attach takeFocus}
				class={{ active: tab === 'connect' }}
				aria-pressed={tab === 'connect'}
				onclick={() => (tab = 'connect')}>Connect</button
			>
			{#if experiments.length > 0}<button
					class="export"
					data-testid="export-experiment-journal"
					onclick={exportJournal}>Export</button
				>{/if}
		</nav>
		<div class="agent-content">
			{#if tab === 'connect'}<AgentGuide
					{supported}
					{registrationError}
					{browser}
					{pageToolCalls}
					{toolCount}
					callCount={experiments.length}
				/>{:else}
				<p class="call-count" role="status">
					{activities.length ? `Calls: ${experiments.length}` : ''}
				</p>
				{#if registrationError}
					<p
						class="registration-error"
						data-testid="webmcp-registration-error"
						title={registrationError}
					>
						Tools failed to load
					</p>
				{/if}

				{#if activities.length === 0 && planning.entries.length === 0}
					<p class="empty">No activity</p>
				{/if}

				{#if planning.entries.length > 0}
					<section class="plans" aria-label="capacity planning activity">
						<div class="section-head">
							<h3>Capacity plans</h3>
						</div>
						<ol aria-live="polite">
							{#each planning.entries as entry (entry.id)}
								{@const outcome = entry.outcome}
								{@const status = entry.decision}
								<li class="plan" data-testid="capacity-plan-card" data-activity-id={entry.id}>
									<div class="plan-head">
										<strong>Capacity plan</strong>
										<span
											class="chip-status mono"
											data-testid="capacity-plan-status"
											data-status={status}>{status.charAt(0).toUpperCase() + status.slice(1)}</span
										>
									</div>
									{#if outcome}
										<div class="phi mono">
											<span class="label">Weighted price</span>
											<span
												>{formatNumber(outcome.baseline_phi)} → {formatNumber(outcome.final_phi)} $/MWh
												({formatDelta(exactPhiDelta(entry))})</span
											>
										</div>
										<div class="phi-detail mono">
											<span>Predicted {formatDelta(predictedPhiDelta(entry))}</span>
											<span>Solved {formatDelta(exactPhiDelta(entry))}</span>
										</div>
										{#if entry.displayProposal.length > 0}
											<ul class="changes mono">
												{#each entry.displayProposal as change (change.branchId)}
													<li>
														<span>{change.branchId}</span>
														<span>Rating {formatDelta(change.deltaMw)} MW</span>
													</li>
												{/each}
											</ul>
										{/if}
									{/if}
									{#if isStaged(entry)}
										<div class="review">
											{#if isApproved(entry)}
												<span class="approved mono" data-testid="capacity-plan-approved"
													>Approved</span
												>
											{:else}
												<button
													class="approve mono"
													data-testid="capacity-plan-approve"
													onclick={() => planning.approve()}
												>
													Approve
												</button>
											{/if}
											<button
												class="reject mono"
												data-testid="capacity-plan-reject"
												onclick={() => planning.rejectStaged()}
											>
												Reject
											</button>
										</div>
									{/if}
								</li>
							{/each}
						</ol>
					</section>
				{/if}

				{#if activities.length > 0}
					<ol aria-live="polite">
						{#each activities as activity (activity.id)}
							{@const compare = comparison(activity)}
							{@const predicted = prediction(activity)}
							{@const check = experiments.find(
								(entry) => entry.id === activity.id
							)?.predictionCheck}
							<li
								class={{
									running: !finished(activity),
									failed: finished(activity) && !activity.response.ok
								}}
							>
								<div class="activity-head">
									<span class="status" aria-hidden="true"></span>
									<div>
										<strong>{activity.title}</strong>
									</div>
									<span class="elapsed mono">{elapsed(activity)}</span>
								</div>

								{#if predicted}
									<div class="preview-result">
										<span class="label">Predicted</span>
										<b class="mono">{formatDelta(number(predicted.objective_delta))} $/h</b>
									</div>
								{/if}

								{#if compare}
									<div class="comparison">
										<div class="comparison-head">
											<span></span><span>Before</span><span>After</span>
										</div>
										<div class="metric mono">
											<span class="name">Operational cost ($/h)</span>
											<span>{formatNumber(number(compare.before.objective))}</span>
											<span>{formatNumber(number(compare.after.objective))}</span>
										</div>
										<div class="metric mono">
											<span class="name">Lines at limit</span>
											<span>{formatNumber(number(compare.before.binding_branches), 0)}</span>
											<span>{formatNumber(number(compare.after.binding_branches), 0)}</span>
										</div>
										<div class="metric mono">
											<span class="name">Edits</span>
											<span
												>{formatNumber(number(compare.before.demand_edit_count), 0)} / {formatNumber(
													number(compare.before.rating_edit_count),
													0
												)}</span
											>
											<span
												>{formatNumber(number(compare.after.demand_edit_count), 0)} / {formatNumber(
													number(compare.after.rating_edit_count),
													0
												)}</span
											>
										</div>
									</div>
								{/if}

								{#if finished(activity) && activity.callerCancelled && activity.response.ok}
									<p class="error mono" data-testid="caller-cancelled">
										Cancelled after completing
									</p>
								{/if}
								{#if finished(activity) && !activity.response.ok}
									<p class="error">{activity.response.error.message}</p>
								{/if}
								{#if check}
									<div class="preview-result mono" data-testid="experiment-prediction-check">
										<span>Predicted {formatDelta(check.predictedDelta)} $/h</span>
										<span>Solved {formatDelta(check.exactDelta)} $/h</span>
									</div>
								{/if}
							</li>
						{/each}
					</ol>
				{/if}
			{/if}
		</div>
	</div>
</PanelFrame>
{#if welcome && hint && !open && !app.zen}<aside
		class="welcome"
		class:compact={app.compactLayout}
		style:--sheet-clear={app.compactLayout ? `${app.sheetInset + 8}px` : undefined}
		aria-label="About AI agents"
	>
		<span>{hint.text}</span>
		<button class="welcome-go" onclick={showConnect}>{hint.action}</button>
		<button
			class="welcome-close"
			aria-label="Dismiss introduction"
			title="Dismiss"
			onclick={dismiss}
			><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
				><path d="m3 3 6 6m0-6-6 6" stroke="currentColor" stroke-width="1.4" /></svg
			></button
		>
	</aside>{/if}

<style>
	.activity-panel {
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	button {
		font: inherit;
		cursor: pointer;
	}
	nav .export {
		margin-left: auto;
		color: var(--text-accent);
	}

	nav {
		display: flex;
		gap: 24px;
		padding: 0 18px;
		border-bottom: 1px solid var(--line);
		flex: none;
	}
	nav button {
		border: 0;
		border-bottom: 2px solid transparent;
		padding: 8px 0 10px;
		background: transparent;
		color: var(--text-secondary);
	}
	nav button.active {
		border-bottom-color: var(--accent);
		color: var(--ink);
	}
	.agent-content {
		overflow: auto;
		min-height: 0;
	}
	.call-count {
		margin: 0;
		padding: 12px 18px;
		font-size: 12px;
		color: var(--text-secondary);
		border-bottom: 1px solid var(--line);
	}
	/* The live region stays in place for announcements, but takes no room while empty. */
	.call-count:empty {
		padding: 0;
		border: 0;
	}
	.registration-error {
		margin: 0;
		padding: 16px 18px;
		color: var(--text-danger);
		font-size: 12px;
	}
	/* Every panel's empty line sits where its content would start: 16 px down, 18 px in. */
	.empty {
		margin: 0;
		padding: 16px 18px;
		color: var(--text-secondary);
	}
	/* First visit only: a small note in the map's lower left corner, clear of
	   the panels, the map controls and the data. */
	.welcome {
		position: fixed;
		/* Above the dynamics playback dock while it is open, in the map control row below the panels. */
		bottom: max(16px, calc(var(--tellegen-timeline-clear, -1000px) + 8px));
		left: 16px;
		z-index: 30;
		display: flex;
		gap: 4px;
		align-items: center;
		max-width: calc(100vw - 32px);
		padding: 4px 4px 4px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--panel);
		color: var(--text-secondary);
		font: var(--fs-label) / 1.4 var(--font-sans);
	}
	.welcome.compact {
		bottom: var(--sheet-clear);
		left: 12px;
		max-width: calc(100vw - 24px);
	}
	.welcome button {
		display: inline-grid;
		place-items: center;
		min-height: 24px;
		padding: 0 6px;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		font: inherit;
		cursor: pointer;
	}
	.welcome .welcome-go {
		color: var(--text-accent);
		font-weight: 500;
	}
	.welcome .welcome-close {
		color: var(--text-secondary);
	}
	.welcome button:hover {
		background: var(--surface-hover);
	}
	@media (pointer: coarse) {
		.welcome button {
			min-width: 44px;
			min-height: 44px;
		}
	}

	.plans {
		border-bottom: 1px solid var(--line);
	}

	.section-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
		padding: 9px 14px 0;
	}

	.section-head h3 {
		margin: 0;
		color: var(--text-tertiary);
		font-size: 12px;
		font-weight: 500;
	}

	.plan-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 8px;
	}

	.chip-status {
		padding: 2px 6px;
		border: 1px solid var(--line);
		border-radius: 999px;
		background: var(--surface-control);
		color: var(--text-secondary);
		font-size: 12px;
	}

	.chip-status[data-status='applied'] {
		border-color: var(--accent);
		color: var(--text-accent);
	}

	.chip-status[data-status='rejected'],
	.chip-status[data-status='expired'] {
		color: var(--text-danger);
	}

	.phi,
	.phi-detail {
		margin: 7px 0 0 16px;
		font-size: 12px;
	}

	.phi {
		display: flex;
		gap: 8px;
	}

	.phi-detail {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		color: var(--text-secondary);
		font-size: 12px;
	}

	.changes {
		margin: 7px 0 0 16px;
		padding: 6px 9px;
		list-style: none;
		border-left: 2px solid var(--accent);
		background: rgb(var(--paper-rgb) / 0.55);
		font-size: 12px;
	}

	.changes li {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		padding: 2px 0;
		border-bottom: 0;
	}

	.review {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 9px 0 0 16px;
	}

	.approve,
	.reject {
		padding: 5px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-xs);
		background: var(--surface-control);
		color: var(--text-secondary);
		font-size: 12px;
		cursor: pointer;
	}

	.approve {
		border-color: var(--accent);
		color: var(--text-accent);
	}

	.approve:hover,
	.reject:hover {
		border-color: var(--accent);
		color: var(--text-accent);
	}

	.approved {
		color: var(--text-accent);
		font-size: 12px;
	}

	ol {
		min-height: 0;
		margin: 0;
		padding: 0;
		list-style: none;
		overflow-y: auto;
	}

	li {
		padding: 11px 14px;
		border-bottom: 1px solid rgb(var(--ink-rgb) / 0.08);
	}

	li:last-child {
		border-bottom: 0;
	}

	.activity-head {
		display: grid;
		grid-template-columns: 8px 1fr auto;
		align-items: start;
		gap: 8px;
	}

	.status {
		width: 7px;
		height: 7px;
		margin-top: 4px;
		border-radius: 50%;
		background: var(--neg);
	}

	li.running {
		background: var(--agent-soft);
		box-shadow: inset 2px 0 0 var(--agent);
	}

	.running .status {
		background: var(--agent);
		animation: blink var(--dur-blink) var(--ease-in-out) infinite;
	}

	.failed .status {
		background: var(--red);
	}

	strong {
		display: block;
	}

	strong {
		font-size: 12px;
		font-weight: 600;
	}

	.elapsed {
		color: var(--text-tertiary);
		font-size: 12px;
	}

	.elapsed {
		padding-top: 2px;
	}

	.preview-result,
	.comparison {
		margin: 9px 0 0 16px;
		border-left: 2px solid var(--accent);
		background: rgb(var(--paper-rgb) / 0.55);
	}

	.preview-result {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 3px 10px;
		padding: 8px 9px;
		font-size: 12px;
	}

	.label {
		color: var(--text-secondary);
	}

	.comparison {
		padding: 7px 9px;
	}

	.comparison-head,
	.metric {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 72px 72px;
		gap: 5px;
		align-items: baseline;
		text-align: right;
	}

	.comparison-head {
		padding-bottom: 5px;
		color: var(--text-tertiary);
		font-size: 12px;
	}

	.comparison-head span:first-child,
	.metric span:first-child {
		text-align: left;
	}

	.metric {
		padding: 3px 0;
		border-top: 1px solid rgb(var(--ink-rgb) / 0.06);
		font-size: 12px;
	}

	.error {
		margin: 7px 0 0 16px;
		color: var(--text-danger);
		font-size: 12px;
		line-height: 1.45;
	}
	@media (max-width: 960px) {
		.welcome {
			display: none;
		}
	}
</style>
