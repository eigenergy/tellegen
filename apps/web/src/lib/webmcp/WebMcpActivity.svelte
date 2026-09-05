<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import AgentGuide from './AgentGuide.svelte';
	import { getAppState } from '@tellegen/svelte';
	const app = getAppState();
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
		studyExpanded,
		closeStudy,
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
		if (app.compactLayout) closeStudy();
		open = true;
		tab = activities.length || planning.entries.length ? 'activity' : 'connect';
		dismiss();
	}

	let autoOpened = false;
	$effect(() => {
		if (studyExpanded && app.compactLayout) open = false;
	});

	$effect(() => {
		if (
			(registrationError || activities.length > 0 || planning.entries.length > 0) &&
			!autoOpened
		) {
			autoOpened = true;
			if (app.compactLayout) closeStudy();
			tab = 'activity';
			open = true;
		}
	});

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

<div class="agent-workspace" style:--activity-top="{app.headerInset + 10}px">
	{#if open}
		<aside class="activity-panel" data-webmcp-activity="open" aria-label="WebMCP activity">
			<header>
				<h2>Agent <span class="activity-count">{experiments.length || ''}</span></h2>
				{#if experiments.length > 0}<button
						class="export"
						data-testid="export-experiment-journal"
						onclick={exportJournal}>Export activity</button
					>{/if}
				<button class="close" aria-label="Close agent panel" onclick={() => (open = false)}
					><svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"
						><path d="m4 4 8 8m0-8-8 8" fill="none" stroke="currentColor" stroke-width="1.5" /></svg
					></button
				>
			</header>
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
		</aside>
	{:else}
		<button
			class="activity-toggle"
			data-webmcp-activity="collapsed"
			aria-label="Agent"
			aria-expanded="false"
			onclick={show}
		>
			<svg
				width="16"
				height="16"
				viewBox="0 0 20 20"
				fill="none"
				stroke="currentColor"
				stroke-width="1.3"
				aria-hidden="true"
				><rect x="3" y="6" width="14" height="11" rx="3" /><path
					d="M10 3v3M7 10v3m6-3v3M1 10h2m14 0h2"
				/></svg
			>
			Agent {#if activities.length}<span class="count">{activities.length}</span>{/if}
		</button>
		{#if welcome}<aside class="welcome" aria-label="New to Tellegen">
				<span>New: saved studies and agent tools.</span><a href={resolve('/changelog')}
					>What's new</a
				><button aria-label="Dismiss introduction" onclick={dismiss}
					><svg width="12" height="12" viewBox="0 0 16 16" aria-hidden="true"
						><path d="m4 4 8 8m0-8-8 8" fill="none" stroke="currentColor" stroke-width="1.5" /></svg
					></button
				>
			</aside>{/if}
	{/if}
</div>

<style>
	.agent-workspace {
		position: fixed;
		top: var(--activity-top);
		right: 20px;
		z-index: var(--z-overlay);
		color: var(--ink);
		font: 13px/1.5 var(--font-display);
	}
	.activity-panel {
		width: min(370px, calc(100vw - 40px));
		max-height: calc(100dvh - var(--activity-top) - 132px);
		display: flex;
		flex-direction: column;
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 6px;
		box-shadow: var(--elev-2);
		overflow: hidden;
	}
	header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 16px;
		flex: none;
	}
	h2 {
		flex: 1;
		font-size: 16px;
		font-weight: 600;
		margin: 0;
	}
	.activity-count {
		color: var(--text-secondary);
		font-size: 12px;
		margin-left: 6px;
		font-weight: 400;
	}
	button {
		font: inherit;
		cursor: pointer;
	}
	.close {
		width: 28px;
		height: 28px;
		display: grid;
		place-items: center;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--text-secondary);
		border-radius: 4px;
	}
	.export {
		border: 0;
		padding: 5px 0;
		background: none;
		color: var(--text-secondary);
		font-size: 11px;
	}
	header button:hover {
		color: var(--text-accent);
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
		position: absolute;
		top: 46px;
		right: 0;
		display: flex;
		align-items: center;
		gap: 12px;
		width: max-content;
		max-width: calc(100vw - 40px);
		padding: 10px 12px;
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 4px;
		font-size: 11px;
		box-shadow: var(--elev-1);
	}
	.welcome a {
		color: var(--text-accent);
		white-space: nowrap;
		text-underline-offset: 3px;
	}
	.welcome button {
		display: grid;
		place-items: center;
		padding: 3px;
		border: 0;
		background: none;
		color: var(--text-secondary);
	}
	:global(body:has(.study-workspace.expanded)) .welcome {
		display: none;
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

	.activity-toggle {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
		min-height: 36px;
		padding: 7px 12px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--paper);
		color: var(--ink);
		box-shadow: var(--elev-1);
	}
	.count {
		font-size: 11px;
		color: var(--text-secondary);
	}
	@media (max-width: 760px) {
		:global(body:has(.activity-panel) .maplibregl-ctrl-bottom-right) {
			bottom: 12px;
			opacity: 1;
			pointer-events: auto;
		}
		.agent-workspace {
			right: 12px;
		}
		.activity-panel {
			width: calc(100vw - 24px);
			max-height: calc(100dvh - var(--activity-top) - 60px);
		}
		.welcome {
			max-width: calc(100vw - 24px);
			gap: 8px;
			font-size: 10px;
		}
		:global(body:has(.activity-panel) aside.panel) {
			visibility: hidden;
			pointer-events: none;
		}
	}
</style>
