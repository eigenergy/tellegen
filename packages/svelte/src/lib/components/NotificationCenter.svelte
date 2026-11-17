<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getAppState } from '../context.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import LogEntryView from './LogEntryView.svelte';
	// The toast: the latest messages sent to the Log with `notify`, until they
	// time out or are closed. The Log panel lists them all.
	const app = getAppState();
	const log = app.log;
	const layout = getPanelLayout();
	const toast = $derived(log.toast);
	const only = $derived(toast?.entries.length === 1 ? toast.entries[0] : null);
	/** Details has something to add: a group's entries, or more than the title. */
	const more = $derived(
		!only || !!(only.title || only.details || only.code || only.hint || only.buses?.length)
	);
	/** The toast sits over the map, between the docked panels. */
	const inset = (side: 'left' | 'right') => {
		const width = layout.compact || app.zen ? 0 : layout.dockWidth(side);
		return `${width ? width + 16 : 0}px`;
	};
	let expanded = $state(false);
	let focused = false;
	let hovered = false;
	/** The toast stays while the reader points at it, is in it, or reads its details. */
	const hold = () => log.pause(focused || hovered || expanded);
	function close() {
		focused = hovered = expanded = false;
		log.pause(false);
		log.dismiss();
	}
	onDestroy(close);
</script>

<div class="sr-only" role="status" aria-live="polite" aria-atomic="true">
	{log.announcement}
</div>
{#if toast}
	<section
		class="notifications"
		aria-label="Notifications"
		style:--left={inset('left')}
		style:--right={inset('right')}
		onfocusin={() => {
			focused = true;
			hold();
		}}
		onfocusout={(event) => {
			focused = event.currentTarget.contains(event.relatedTarget as Node | null);
			hold();
		}}
		onpointerenter={() => {
			hovered = true;
			hold();
		}}
		onpointerleave={() => {
			hovered = false;
			hold();
		}}
	>
		<div class="notice" data-testid="notification-toast" data-kind={toast.severity}>
			<div class="row">
				<span class="message">{toast.title}</span>
				{#if only && only.count > 1}<span class="count" aria-label={`${only.count} occurrences`}
						>{only.count}</span
					>{/if}
				{#if only?.retry && (only.canRetry?.() ?? true)}<button onclick={only.retry}>Retry</button
					>{/if}
				{#if more}<button
						aria-expanded={expanded}
						aria-controls="notice-details"
						onclick={() => {
							expanded = !expanded;
							hold();
						}}>Details</button
					>{/if}
				<button aria-label="Dismiss notification" onclick={close}>Close</button>
			</div>
			{#if expanded && more}
				<ol class="inline" id="notice-details" data-testid="notification-details">
					{#each toast.entries as entry (entry.id)}
						<li><LogEntryView {entry} bare={!!only} /></li>
					{/each}
				</ol>
			{/if}
		</div>
	</section>
{/if}

<style>
	.notifications {
		position: fixed;
		z-index: var(--z-toast);
		/* Above the map's control row (44 px), which sits on the dynamics playback
		   dock while that is open. The fallback keeps the closed-dock position. */
		bottom: max(40px, calc(var(--tellegen-timeline-clear, -1000px) + 44px));
		left: var(--left);
		right: var(--right);
		width: min(540px, calc(100vw - var(--left) - var(--right) - 24px));
		margin-inline: auto;
		font: 13px/1.4 var(--font-display);
		pointer-events: none;
	}
	.notice {
		pointer-events: auto;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		box-shadow: var(--elev-2);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 38px;
		padding: 3px 8px 3px 12px;
		box-sizing: border-box;
	}
	.inline {
		max-height: min(240px, 36dvh);
		overflow-y: auto;
		list-style: none;
		margin: 0;
		padding: 4px 12px;
		border-top: 1px solid var(--line);
	}
	.inline li {
		padding: 6px 0;
	}
	/* The message says what to do next, so it wraps instead of being cut off. */
	.message {
		min-width: 0;
		flex: 1;
		padding: 5px 0;
		overflow-wrap: anywhere;
	}
	button {
		font: inherit;
		color: var(--ink);
		border: 0;
		background: transparent;
		padding: 6px;
		border-radius: var(--radius-control);
		cursor: pointer;
		flex-shrink: 0;
	}
	button:hover {
		background: var(--accent-soft);
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.count {
		margin-left: 5px;
		font: 12px var(--font-mono);
		color: var(--text-secondary);
	}
	@media (max-width: 960px) {
		.notifications {
			/* Above the control row, on the dock or on the open sheet. */
			bottom: max(
				60px,
				calc(var(--tellegen-timeline-clear, -1000px) + 44px),
				calc(var(--tellegen-sheet-clear, -1000px) + 52px)
			);
		}
		button {
			min-height: 36px;
		}
	}
</style>
