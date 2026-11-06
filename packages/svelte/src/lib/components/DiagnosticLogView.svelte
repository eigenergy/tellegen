<script lang="ts">
	import { getAppState } from '../context.svelte.js';
	import { downloadText } from '../controller.svelte.js';
	import {
		LOG_SEVERITIES,
		type DiagnosticLog,
		type LogEntry,
		type LogEntryInput,
		type LogSeverity
	} from '../log.svelte.js';
	import LogEntryView from './LogEntryView.svelte';
	let {
		log = getAppState().log,
		onbus,
		filename = 'tellegen-log.json',
		empty = 'Nothing to report. Findings from opening, solving and exporting a case appear here.'
	}: {
		log?: DiagnosticLog;
		/** Makes each named bus a button, for a host that can select it. */
		onbus?: (bus: number, entry: LogEntryInput) => void;
		filename?: string;
		empty?: string;
	} = $props();
	const PLURAL = { error: 'Errors', warning: 'Warnings', note: 'Notes' } as const;
	let filter = $state<LogSeverity | 'all'>('all');
	let copied = $state(false);
	const counts = $derived(log.counts);
	const shown = $derived(
		filter === 'all' ? log.entries : log.entries.filter((e) => e.severity === filter)
	);
	// One group per step and subject, in the order each first appeared.
	const groups = $derived.by(() => {
		const out = new Map<string, { stage: string; subject?: string; entries: LogEntry[] }>();
		for (const entry of shown) {
			const key = `${entry.stage}\u0000${entry.subject ?? ''}`;
			let group = out.get(key);
			if (!group)
				out.set(key, (group = { stage: entry.stage, subject: entry.subject, entries: [] }));
			group.entries.push(entry);
		}
		return [...out.values()];
	});
	$effect(() => {
		if (filter !== 'all' && counts[filter] === 0) filter = 'all';
	});
	const text = () => JSON.stringify(log.toJSON(), null, 2);
	async function copy() {
		try {
			await navigator.clipboard.writeText(text());
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			copied = false;
		}
	}
</script>

<section class="log" aria-label="Log" data-testid="diagnostic-log">
	{#if log.entries.length}
		<div class="toolbar">
			<div class="filters" role="group" aria-label="Show">
				<button type="button" aria-pressed={filter === 'all'} onclick={() => (filter = 'all')}
					>All <span class="n">{log.entries.length}</span></button
				>
				{#each LOG_SEVERITIES as severity (severity)}
					{#if counts[severity]}
						<button
							type="button"
							data-severity={severity}
							aria-pressed={filter === severity}
							onclick={() => (filter = severity)}
							>{PLURAL[severity]} <span class="n">{counts[severity]}</span></button
						>
					{/if}
				{/each}
			</div>
			<div class="actions">
				<button type="button" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>
				<button type="button" onclick={() => downloadText(text(), filename, 'application/json')}
					>Download</button
				>
				<button type="button" onclick={() => log.clear()}>Clear</button>
			</div>
		</div>
		{#each groups as group (`${group.stage}\u0000${group.subject ?? ''}`)}
			<section class="group" data-stage={group.stage}>
				<h4>
					<span class="stage">{group.stage}</span>{#if group.subject}<span class="subject"
							>{group.subject}</span
						>{/if}
				</h4>
				<ol>
					{#each group.entries as entry (entry.id)}
						<li><LogEntryView {entry} showSubject={false} {onbus} /></li>
					{/each}
				</ol>
			</section>
		{/each}
	{:else}
		<p class="empty">{empty}</p>
	{/if}
</section>

<style>
	.log {
		font: var(--fs-meta, 12px) / 1.45 var(--font-display);
		color: var(--text-primary, var(--ink));
	}
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		margin-bottom: 8px;
	}
	.filters,
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	button {
		font: inherit;
		color: var(--ink);
		border: 1px solid var(--line);
		background: transparent;
		padding: 3px 8px;
		border-radius: 3px;
		cursor: pointer;
	}
	button:hover {
		background: var(--accent-soft);
	}
	button[aria-pressed='true'] {
		background: var(--accent-soft);
		border-color: var(--border-strong, var(--line));
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.n {
		font: 10px var(--font-mono);
		color: var(--text-secondary);
	}
	.group + .group {
		margin-top: 10px;
	}
	h4 {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		align-items: baseline;
		margin: 0 0 4px;
		font: 600 var(--fs-meta, 12px) / 1.4 var(--font-display);
	}
	.subject {
		font-weight: 400;
		color: var(--text-secondary);
		overflow-wrap: anywhere;
	}
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		padding: 6px 0;
		border-top: 1px solid var(--line);
	}
	.empty {
		margin: 0;
		color: var(--text-secondary);
	}
</style>
