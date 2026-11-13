<script lang="ts">
	import { fly } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { createSubscriber } from 'svelte/reactivity';
	import { getAppState, getPanelLayout } from '@tellegen/svelte';
	import type { TellegenToolActivityEvent } from '@tellegen/webmcp';
	import { activitySummary, formatElapsed } from './activity-summary.js';

	let { activities }: { activities: TellegenToolActivityEvent[] } = $props();

	/** How long a finished call stays on screen: long enough to read a result,
	 * longer for a failure. */
	const DONE_MS = 4_500;
	const FAILED_MS = 8_000;

	const app = getAppState();
	const layout = getPanelLayout();

	// Newest first. A call still running outranks a newer one that finished.
	const current = $derived(
		activities.find((activity) => activity.type === 'started') ?? activities[0]
	);
	const status = $derived(
		!current || current.type === 'started' ? 'running' : current.response.ok ? 'done' : 'failed'
	);
	const until = $derived(
		!current || current.type === 'started'
			? Infinity
			: current.finishedAt + (current.response.ok ? DONE_MS : FAILED_MS)
	);

	// A clock that ticks only while something reads it. `visible` stops reading
	// it once the shown call's hold has passed, so an idle page does no work.
	const tick = createSubscriber((update) => {
		const timer = setInterval(update, 100);
		return () => clearInterval(timer);
	});
	function now() {
		tick();
		return Date.now();
	}

	const visible = $derived.by(() => {
		if (!current) return false;
		// Read without subscribing first: an expired call must not keep the clock alive.
		if (Date.now() >= until) return false;
		return now() < until;
	});
	const elapsed = $derived(
		!current
			? ''
			: formatElapsed(
					(current.type === 'finished' ? current.finishedAt : now()) - current.startedAt
				)
	);
	const summary = $derived(current?.type === 'finished' ? activitySummary(current) : null);
	const announcement = $derived(
		!current ? '' : `Agent: ${current.title}. ${summary ?? 'Working.'}`
	);
	const duration = $derived(prefersReducedMotion.current ? 0 : 200);

	function openPanel() {
		const panel = layout.panels.find((candidate) => candidate.id === 'agent');
		if (!panel) return;
		// Zen hides every panel; asking for the details is asking to leave it.
		if (app.zen) app.zen = false;
		const shown = layout.compact ? layout.drawer === panel.id : panel.open;
		if (!shown) layout.toggle(panel);
	}
</script>

<p class="announcer" role="status">{announcement}</p>
{#if visible && current}
	<div
		class="anchor"
		style:top="{app.headerInset + 4}px"
		transition:fly={{ y: -8, duration, opacity: 0 }}
	>
		<button
			class={['ribbon', status]}
			data-testid="agent-ribbon"
			data-status={status}
			onclick={openPanel}
			title="Show this call in the Agent panel"
		>
			<span class="mark" aria-hidden="true"></span>
			<span class="who">Agent</span>
			<span class="what">{current.title}</span>
			<span class="time" aria-hidden="true">{elapsed}</span>
			<span class="summary" data-testid="agent-ribbon-summary"
				>{status === 'running' ? 'Working' : summary}</span
			>
		</button>
	</div>
{/if}

<style>
	.announcer {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}

	.anchor {
		position: fixed;
		left: 50%;
		z-index: 40;
		transform: translateX(-50%);
		width: max-content;
		max-width: min(560px, calc(100vw - 32px));
	}

	.ribbon {
		display: grid;
		grid-template-columns: 8px auto minmax(0, 1fr) auto;
		align-items: baseline;
		column-gap: 8px;
		row-gap: 2px;
		width: 100%;
		min-width: min(300px, calc(100vw - 32px));
		padding: 8px 14px 9px 12px;
		border: 1px solid rgb(var(--agent-rgb) / 0.4);
		border-radius: 4px;
		background: var(--paper);
		box-shadow: var(--elev-cue);
		color: var(--ink);
		font: 13px/1.35 var(--font-display);
		text-align: left;
		cursor: pointer;
	}

	.ribbon.running {
		background: linear-gradient(var(--agent-soft), var(--agent-soft)), var(--paper);
	}

	.ribbon.failed {
		border-color: rgb(var(--ink-rgb) / 0.2);
	}

	.ribbon:hover {
		border-color: var(--agent);
	}

	.mark {
		align-self: center;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--agent);
	}

	.done .mark {
		background: transparent;
		box-shadow: inset 0 0 0 2px var(--agent);
	}

	.failed .mark {
		background: var(--text-danger);
	}

	.who {
		color: var(--agent);
		font-size: 12px;
		font-weight: 600;
	}

	.what {
		overflow: hidden;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.time {
		color: var(--text-secondary);
		font: 11px/1 var(--font-mono);
		font-variant-numeric: tabular-nums;
	}

	.summary {
		grid-column: 2 / -1;
		overflow: hidden;
		color: var(--text-secondary);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.failed .summary {
		color: var(--text-danger);
	}
</style>
