<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getAppState } from '../context.svelte.js';
	import LogEntryView from './LogEntryView.svelte';
	// The toast: the latest messages sent to the Log with `notify`, until they
	// time out or are closed. The Log panel lists them all.
	const log = getAppState().log;
	const toast = $derived(log.toast);
	const only = $derived(toast?.entries.length === 1 ? toast.entries[0] : null);
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

<div class="announcement" role="status" aria-live="polite" aria-atomic="true">
	{log.announcement}
</div>
{#if toast}
	<section
		class="notifications"
		aria-label="Notifications"
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
				<span class="message"
					>{toast.title}{#if only?.title}<span class="text">{only.message}</span>{/if}</span
				>
				{#if only && only.count > 1}<span class="count" aria-label={`${only.count} occurrences`}
						>{only.count}</span
					>{/if}
				{#if only?.retry && (only.canRetry?.() ?? true)}<button onclick={only.retry}>Retry</button
					>{/if}
				<button
					aria-expanded={expanded}
					aria-controls="notice-details"
					onclick={() => {
						expanded = !expanded;
						hold();
					}}>Details</button
				>
				<button aria-label="Dismiss notification" onclick={close}>Close</button>
			</div>
			{#if expanded}
				<ol class="inline" id="notice-details" data-testid="notification-details">
					{#each toast.entries as entry (entry.id)}
						<li><LogEntryView {entry} open={!!only} /></li>
					{/each}
				</ol>
			{/if}
		</div>
	</section>
{/if}

<style>
	.announcement {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}
	.notifications {
		position: fixed;
		z-index: 65;
		/* While the dynamics playback dock is open: above the dock, the map's
		   control row on it (44 px) and the host's one-line notice above that
		   row (32 px). The fallback keeps the closed-dock position. */
		bottom: max(40px, calc(var(--tellegen-timeline-clear, -1000px) + 76px));
		left: 50%;
		transform: translateX(-50%);
		width: min(540px, calc(100vw - 200px));
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
	/* What the error itself said, under the title; Details has all of it. */
	.text {
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		color: var(--text-secondary);
		font-size: 12px;
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
			width: calc(100vw - 24px);
			/* The host's notice is hidden here: clear the control row only. */
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
