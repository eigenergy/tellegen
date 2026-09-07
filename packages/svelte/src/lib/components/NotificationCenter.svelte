<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { formatPowerIoDiagnostic } from '@tellegen/engine';
	import { getAppState, getController } from '../context.svelte.js';
	import { getNoticeCenter, errorNoticeTitle } from '../notices.svelte.js';
	const app = getAppState();
	const ctrl = getController();
	const notices = getNoticeCenter();
	let root = $state.raw<HTMLElement>();
	let focused = $state(false);
	let hovered = $state(false);
	const observed = new WeakSet<object>();
	$effect(() => {
		const message = app.error;
		const revision = app.errorRevision;
		const retry = app.errorRetry;
		if (message)
			untrack(() =>
				notices.push({
					kind: 'error',
					title: errorNoticeTitle(message),
					details: message,
					retry: () => ctrl.retryError(),
					canRetry: () =>
						app.errorRevision === revision && app.error === message && app.errorRetry === retry
				})
			);
	});
	$effect(() => {
		const local = app.activeLocal;
		const multi = app.activeMulti;
		const summary = local?.summary ?? multi?.summary;
		const geo = local?.geoWarnings ?? multi?.geoWarnings;
		for (const [identity, title, details] of [
			[
				summary,
				'Case import notes',
				summary
					? [
							...summary.diagnostics.map(formatPowerIoDiagnostic),
							...('warnings' in summary ? summary.warnings : [])
						].join('\n\n')
					: ''
			],
			[geo, 'Coordinate import notes', geo?.join('\n\n') ?? '']
		] as Array<[object | undefined, string, string]>) {
			if (identity && !observed.has(identity)) {
				observed.add(identity);
				if (details) untrack(() => notices.push({ kind: 'warning', title, details }));
			}
		}
	});
	$effect(() => {
		notices.pause(focused || hovered || notices.expanded);
	});
	onDestroy(() => notices.dispose());
</script>

<div class="announcement" role="status" aria-live="polite" aria-atomic="true">
	{notices.announcement}
</div>
{#if notices.entries.length}
	<section
		class="notifications"
		aria-label="Notifications"
		bind:this={root}
		onfocusin={() => (focused = true)}
		onfocusout={(event) => {
			focused = root?.contains(event.relatedTarget as Node | null) ?? false;
		}}
		onpointerenter={() => (hovered = true)}
		onpointerleave={() => (hovered = false)}
	>
		{#if notices.expanded}
			<div class="recent" id="recent-notifications">
				<header>
					<strong>Recent messages</strong><button
						onclick={() => {
							focused = false;
							hovered = false;
							notices.clear();
						}}>Clear</button
					>
				</header>
				<ol>
					{#each notices.entries as entry (entry.id)}
						<li>
							<details>
								<summary
									>{entry.title}{#if entry.count > 1}<span class="count">{entry.count}</span
										>{/if}</summary
								>
								<pre data-testid="notification-details">{entry.details}</pre>
							</details>
							{#if entry.retry && entry.canRetry?.()}<button onclick={entry.retry}>Retry</button
								>{/if}
						</li>
					{/each}
				</ol>
			</div>
		{/if}
		<div
			class="notice"
			data-testid="notification-toast"
			data-kind={notices.active?.kind}
			class:quiet={!notices.active}
		>
			{#if notices.active}
				<span class="message">{notices.active.title}</span>
				{#if notices.active.count > 1}<span
						class="count"
						aria-label={`${notices.active.count} occurrences`}>{notices.active.count}</span
					>{/if}
				{#if notices.active.retry && notices.active.canRetry?.()}<button
						onclick={notices.active.retry}>Retry</button
					>{/if}
			{/if}
			<button
				class="history"
				aria-expanded={notices.expanded}
				aria-controls="recent-notifications"
				onclick={() => (notices.expanded = !notices.expanded)}
				>{notices.active ? 'Details' : 'Messages'}{#if !notices.active}<span class="count"
						>{notices.entries.length}</span
					>{/if}</button
			>
			{#if notices.active}<button
					aria-label="Dismiss notification"
					onclick={() => notices.dismiss()}>Close</button
				>{/if}
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
		bottom: 40px;
		left: 50%;
		transform: translateX(-50%);
		width: min(540px, calc(100vw - 200px));
		font: 12px/1.4 var(--font-display);
		pointer-events: none;
	}
	.notice,
	.recent {
		pointer-events: auto;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 5px;
		box-shadow: var(--elev-2);
	}
	.notice {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 38px;
		padding: 3px 8px 3px 12px;
		box-sizing: border-box;
	}
	.message {
		min-width: 0;
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.quiet {
		width: fit-content;
		margin: auto;
		padding-left: 5px;
		background: var(--paper);
	}
	button {
		font: inherit;
		color: var(--ink);
		border: 0;
		background: transparent;
		padding: 6px;
		border-radius: 3px;
		cursor: pointer;
		flex-shrink: 0;
	}
	button:hover {
		background: var(--accent-soft);
	}
	button:focus-visible,
	summary:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.count {
		margin-left: 5px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
	}
	.recent {
		margin-bottom: 6px;
		max-height: min(320px, 40dvh);
		overflow-y: auto;
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 12px;
	}
	ol {
		list-style: none;
		padding: 0;
		margin: 0;
	}
	li {
		padding: 10px 12px;
		border-top: 1px solid var(--line);
	}
	summary {
		cursor: pointer;
	}
	pre {
		font: 11px/1.5 var(--font-mono);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		margin: 10px 0 0;
	}
	@media (max-width: 960px) {
		.notifications {
			width: calc(100vw - 24px);
			bottom: 60px;
		}
		button {
			min-height: 36px;
		}
	}
</style>
