<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import AgentGuide from './AgentGuide.svelte';
	import { PanelFrame } from '@tellegen/svelte';
	import type {
		ExperimentRecord,
		JsonValue,
		TellegenToolActivityEvent,
		ToolResponse
	} from '@tellegen/webmcp';
	import {
		capacityPlanExactPhiDelta,
		capacityPlanFirstOrderError,
		capacityPlanPredictedPhiDelta
	} from './capacity-plan-metrics.js';
	import type { CapacityPlanActivity, PlanningActivityStore } from './planning-activity.svelte.js';

	let {
		supported,
		registrationError,
		activities,
		planning,
		experiments,
		exportJournal
	}: {
		studyExpanded: boolean;
		closeStudy: () => void;
		supported: boolean;
		registrationError: string | null;
		activities: TellegenToolActivityEvent[];
		planning: PlanningActivityStore;
		experiments: ExperimentRecord[];
		exportJournal: () => void;
	} = $props();

	let open = $state(false);
	let tab = $state<'activity' | 'connect'>('connect');
	let welcome = $state(false);
	const welcomeKey = 'tellegen.welcome.studies-v1';
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

	function firstOrderError(entry: CapacityPlanActivity): number | null {
		return capacityPlanFirstOrderError(entry.outcome);
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
			? 'unavailable'
			: value.toLocaleString(undefined, { maximumFractionDigits: digits });
	}

	function formatDelta(value: number | null): string {
		if (value === null) return 'unavailable';
		return `${value >= 0 ? '+' : ''}${formatNumber(value)}`;
	}

	function elapsed(activity: TellegenToolActivityEvent): string {
		if (!finished(activity)) return 'working';
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

<PanelFrame id="agent" title="Agent" side="right" order={20} width={370} bind:open onopen={show}>
	{#snippet headerActions()}
		{#if experiments.length > 0}<button
				class="export"
				data-testid="export-experiment-journal"
				onclick={exportJournal}>Export</button
			>{/if}
	{/snippet}
	<div class="activity-panel" data-webmcp-activity="open" aria-label="WebMCP activity">
		<nav aria-label="Agent panel">
			<button
				class:active={tab === 'activity'}
				aria-pressed={tab === 'activity'}
				onclick={() => (tab = 'activity')}>Activity</button
			>
			<button
				class:active={tab === 'connect'}
				aria-pressed={tab === 'connect'}
				onclick={() => (tab = 'connect')}>Connect</button
			>
		</nav>
		<div class="agent-content">
			{#if tab === 'connect'}<AgentGuide {supported} {registrationError} />{:else}
				<p class="call-count" role="status">
					{activities.length
						? `${experiments.length} completed WebMCP calls${activities.some((activity) => !finished(activity)) ? ', working' : ''}`
						: 'No WebMCP calls yet'}
				</p>
				{#if registrationError}
					<p class="registration-error mono" data-testid="webmcp-registration-error">
						WebMCP tools could not be registered: {registrationError}
					</p>
				{/if}

				{#if activities.length === 0 && planning.entries.length === 0}
					<div class="empty">
						<p>No agent activity yet.</p>
						<button class="copy" onclick={() => (tab = 'connect')}>Connect an agent</button>
					</div>
				{/if}

				{#if planning.entries.length > 0}
					<section class="plans" aria-label="capacity planning activity">
						<div class="section-head">
							<h3 class="mono">capacity proposals</h3>
						</div>
						<ol aria-live="polite">
							{#each planning.entries as entry (entry.id)}
								{@const outcome = entry.outcome}
								{@const status = entry.decision}
								<li class="plan" data-testid="capacity-plan-card" data-activity-id={entry.id}>
									<div class="plan-head">
										<strong>capacity proposal</strong>
										<span
											class="chip-status mono"
											data-testid="capacity-plan-status"
											data-status={status}>{status}</span
										>
									</div>
									{#if outcome}
										<div class="phi mono">
											<span class="label">Φ</span>
											<span
												>{formatNumber(outcome.baseline_phi)} → {formatNumber(outcome.final_phi)}
												({formatDelta(exactPhiDelta(entry))} objective units/MW)</span
											>
										</div>
										<div class="phi-detail mono">
											<span>predicted {formatDelta(predictedPhiDelta(entry))}</span>
											<span>exact {formatDelta(exactPhiDelta(entry))}</span>
											<span>first order error {formatNumber(firstOrderError(entry), 4)}</span>
										</div>
										{#if entry.displayProposal.length > 0}
											<ul class="changes mono">
												{#each entry.displayProposal as change (change.branchId)}
													<li>
														<span>{change.branchId}</span>
														<span>{formatDelta(change.deltaMw)} MW rating</span>
													</li>
												{/each}
											</ul>
										{/if}
									{/if}
									{#if isStaged(entry)}
										<div class="review">
											{#if isApproved(entry)}
												<span class="approved mono" data-testid="capacity-plan-approved"
													>approved — the agent may apply once</span
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
								class:running={!finished(activity)}
								class:failed={finished(activity) && !activity.response.ok}
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
										<span class="mono label">predicted Δ objective</span>
										<b class="mono">{formatDelta(number(predicted.objective_delta))}</b>
										<span class="mono dim">state unchanged</span>
									</div>
								{/if}

								{#if compare}
									<div class="comparison">
										<div class="comparison-head mono">
											<span>exact result</span><span>before</span><span>after</span>
										</div>
										<div class="metric mono">
											<span>objective</span>
											<span>{formatNumber(number(compare.before.objective))}</span>
											<span>{formatNumber(number(compare.after.objective))}</span>
										</div>
										<div class="metric mono">
											<span>binding lines</span>
											<span>{formatNumber(number(compare.before.binding_branches), 0)}</span>
											<span>{formatNumber(number(compare.after.binding_branches), 0)}</span>
										</div>
										<div class="metric mono">
											<span>edits (demand/rating)</span>
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

								{#if finished(activity) && !activity.response.ok}
									<p class="error mono">
										{activity.response.error.code}: {activity.response.error.message}
									</p>
								{/if}
								{#if check}
									<div class="preview-result mono" data-testid="experiment-prediction-check">
										<span>predicted Δ {formatDelta(check.predictedDelta)}</span>
										<span>exact Δ {formatDelta(check.exactDelta)}</span>
										<span>prediction error {formatNumber(check.absoluteError, 4)}</span>
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
{#if welcome && !open}<aside class="welcome" aria-label="New to Tellegen">
		<a href={resolve('/changelog')}>New: saved studies and agent tools</a>
		<button aria-label="Dismiss introduction" onclick={dismiss}>Close</button>
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
	.export {
		border: 0;
		padding: 4px;
		background: none;
		color: var(--text-secondary);
		font-size: 11px;
	}

	nav {
		display: flex;
		gap: 24px;
		padding: 0 16px;
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
		padding: 12px 16px;
		font-size: 11px;
		color: var(--text-secondary);
		border-bottom: 1px solid var(--line);
	}
	.registration-error {
		margin: 0;
		padding: 16px;
		color: var(--danger);
		font-size: 12px;
	}
	.empty {
		padding: 24px 20px;
	}
	.empty p {
		margin: 0 0 12px;
		color: var(--text-secondary);
	}
	.copy {
		padding: 7px 10px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: white;
		color: var(--ink);
	}
	.welcome {
		position: fixed;
		bottom: 12px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 30;
		display: flex;
		gap: 12px;
		align-items: center;
		padding: 5px 10px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--paper);
		font: 11px/1.4 var(--font-display);
	}
	.welcome a {
		color: var(--text-accent);
		text-underline-offset: 3px;
	}
	.welcome button {
		padding: 2px;
		border: 0;
		background: none;
		color: var(--text-secondary);
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
		font-size: 8px;
		font-weight: 500;
		letter-spacing: 0.08em;
		text-transform: uppercase;
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
		font-size: 8px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
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
		font-size: 9.5px;
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
		font-size: 8.5px;
	}

	.changes {
		margin: 7px 0 0 16px;
		padding: 6px 9px;
		list-style: none;
		border-left: 2px solid var(--accent);
		background: rgb(var(--paper-rgb) / 0.55);
		font-size: 9px;
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
		font-size: 9px;
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
		font-size: 9px;
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

	.running .status {
		background: var(--accent-bright);
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
		font-size: 8.5px;
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
		font-size: 10px;
	}

	.preview-result .dim {
		grid-column: 1 / -1;
		font-size: 8.5px;
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
		font-size: 8px;
		text-transform: uppercase;
	}

	.comparison-head span:first-child,
	.metric span:first-child {
		text-align: left;
	}

	.metric {
		padding: 3px 0;
		border-top: 1px solid rgb(var(--ink-rgb) / 0.06);
		font-size: 9px;
	}

	.error {
		margin: 7px 0 0 16px;
		color: var(--text-danger);
		font-size: 9px;
		line-height: 1.45;
	}
	@media (max-width: 960px) {
		.welcome {
			display: none;
		}
	}
</style>
