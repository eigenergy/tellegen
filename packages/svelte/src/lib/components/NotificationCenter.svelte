<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { formatPowerIoDiagnostic } from '@tellegen/engine';
	import { getAppState, getController } from '../context.svelte.js';
	import { powerIoEntry, type LogEntryInput } from '../log.svelte.js';
	import { getNoticeCenter, errorNoticeCanRetry, errorNoticeTitle } from '../notices.svelte.js';
	import LogEntryView from './LogEntryView.svelte';
	import { DEMO_LABEL } from '../dynamics/controller.svelte.js';
	const app = getAppState();
	const ctrl = getController();
	const notices = getNoticeCenter();
	let root = $state.raw<HTMLElement>();
	let focused = $state(false);
	let hovered = $state(false);
	const observed = new WeakSet<object>();
	$effect(() => {
		const message = app.error;
		const detail = app.errorDetail;
		const revision = app.errorRevision;
		const retry = app.errorRetry;
		if (message)
			untrack(() =>
				notices.push({
					kind: 'error',
					// A message with its own detail is already written for people.
					title: detail ? message : errorNoticeTitle(message),
					details: detail ?? message,
					retry: errorNoticeCanRetry(detail ?? message) ? () => ctrl.retryError() : undefined,
					// The Network panel shows a failed case list with its own Retry.
					quiet: /^Case list:/.test(message),
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
		const subject = local?.fileName ?? multi?.fileName;
		const entries: LogEntryInput[] = summary
			? [
					...summary.diagnostics.map((d) => powerIoEntry(d, { stage: 'Open', subject })),
					...('warnings' in summary ? summary.warnings : []).map((message) => ({
						severity: 'note' as const,
						stage: 'Open',
						source: 'tellegen',
						subject,
						message
					}))
				]
			: [];
		/** A coordinate note that only reports success: every bus placed, nothing unmatched. */
		const clean = (note: string) =>
			/^(\d+) of \1 buses placed from /.test(note) || /; 0 unmatched objects$/.test(note);
		const count = (n: number, what: string) => `${n} ${what} note${n === 1 ? '' : 's'}`;
		const problems = entries.filter((entry) => entry.severity !== 'note').length;
		const geoCount = geo?.length ?? 0;
		for (const [identity, title, details, items, quiet] of [
			[
				summary,
				`${problems} import ${problems === 1 ? 'warning' : 'warnings'}`,
				summary
					? [
							...summary.diagnostics.map(formatPowerIoDiagnostic),
							...('warnings' in summary ? summary.warnings : [])
						].join('\n\n')
					: '',
				entries,
				// Notes alone are not a notice: the Log keeps them. The dynamics demo's notes are
				// counted in the Dynamics panel's Notes row instead.
				entries.every((entry) => entry.severity === 'note') || local?.label === DEMO_LABEL
			],
			[
				geo,
				count(geoCount, 'coordinate'),
				geo?.join('\n\n') ?? '',
				(geo ?? []).map((message) => ({
					severity: 'note' as const,
					stage: 'Coordinates',
					source: 'tellegen',
					subject,
					message
				})),
				// A success is not a notice: the Log keeps it.
				!!geo && geo.every(clean)
			]
		] as Array<[object | undefined, string, string, LogEntryInput[] | undefined, boolean?]>) {
			if (identity && !observed.has(identity)) {
				observed.add(identity);
				if (details)
					untrack(() => {
						if (!quiet) notices.push({ kind: 'warning', title, details, entries: items });
						// The notice closes itself; the Log keeps what it said.
						if (items?.length) app.log.record(items, { repeat: false });
					});
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
		<!-- The open notice's details expand inside it; the list of past messages is for
		     after it closes. -->
		{#if notices.expanded && !notices.active}
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
					{#each notices.entries as entry, i (entry.id)}
						<li>
							<!-- The latest message opens with its full text: Details shows it at once. -->
							<details open={i === 0}>
								<summary
									>{entry.title}{#if entry.count > 1}<span class="count">{entry.count}</span
										>{/if}</summary
								>
								{#if entry.entries?.length}
									<ol class="entries" data-testid="notification-details">
										{#each entry.entries as item, i (i)}
											<li><LogEntryView entry={item} showSubject={false} /></li>
										{/each}
									</ol>
								{:else}
									<pre data-testid="notification-details">{entry.details}</pre>
								{/if}
							</details>
							{#if entry.retry && entry.canRetry?.()}<button onclick={entry.retry}>Retry</button
								>{/if}
						</li>
					{/each}
				</ol>
			</div>
		{/if}
		<!-- Once a message is dismissed, nothing floats over the map: the Log tab
		     counts it, and the Log lists it. -->
		<div
			class="notice"
			data-testid="notification-toast"
			data-kind={notices.active?.kind}
			class:quiet={!notices.active}
			hidden={!notices.active && !notices.expanded}
		>
			<div class="row">
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
					aria-controls={notices.active ? 'notice-details' : 'recent-notifications'}
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
			{#if notices.active && notices.expanded}
				<div class="inline" id="notice-details">
					{#if notices.active.entries?.length}
						<ol class="entries" data-testid="notification-details">
							{#each notices.active.entries as item, i (i)}
								<li><LogEntryView entry={item} showSubject={false} /></li>
							{/each}
						</ol>
					{:else}
						<pre data-testid="notification-details">{notices.active.details}</pre>
					{/if}
				</div>
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
	.notice,
	.recent {
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
		padding: 0 12px 10px;
		border-top: 1px solid var(--line);
	}
	.inline pre {
		margin-top: 8px;
		color: var(--text-secondary);
	}
	/* The message says what to do next, so it wraps instead of being cut off. */
	.message {
		min-width: 0;
		flex: 1;
		padding: 5px 0;
		overflow-wrap: anywhere;
	}
	.notice[hidden] {
		display: none;
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
		border-radius: var(--radius-control);
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
		font: 12px var(--font-mono);
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
	.entries {
		margin-top: 8px;
	}
	.entries li {
		padding: 6px 0;
	}
	pre {
		font: 12px/1.5 var(--font-mono);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		margin: 10px 0 0;
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
