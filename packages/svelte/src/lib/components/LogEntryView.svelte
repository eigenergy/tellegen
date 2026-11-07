<script lang="ts">
	import type { LogEntry, LogEntryInput } from '../log.svelte.js';
	let {
		entry,
		showSubject = true,
		onbus
	}: {
		entry: LogEntryInput | LogEntry;
		/** Off inside a group that already names the subject. */
		showSubject?: boolean;
		/** Makes each named bus a button, for a host that can select it. */
		onbus?: (bus: number, entry: LogEntryInput) => void;
	} = $props();
	const LABEL = { error: 'Error', warning: 'Warning', note: 'Note' } as const;
	const MAX_BUSES = 12;
	const count = $derived('count' in entry ? entry.count : 1);
	const buses = $derived(entry.buses ?? []);
	const details = $derived(
		Object.entries(entry.details ?? {}).map(
			([key, value]) => [key, typeof value === 'string' ? value : JSON.stringify(value)] as const
		)
	);
</script>

<div class="entry" data-severity={entry.severity} data-testid="log-entry">
	<span class="severity">{LABEL[entry.severity]}</span>
	<div class="body">
		<p class="message">
			{entry.message}{#if count > 1}<span class="count" aria-label={`reported ${count} times`}
					>{count}×</span
				>{/if}
		</p>
		<dl class="meta">
			<div>
				<dt>From</dt>
				<dd>{entry.source}</dd>
			</div>
			{#if entry.code}<div>
					<dt>Code</dt>
					<dd class="mono">{entry.code}</dd>
				</div>{/if}
			{#if showSubject && entry.subject}<div>
					<dt>In</dt>
					<dd>{entry.subject}</dd>
				</div>{/if}
			{#if entry.target}<div>
					<dt>Element</dt>
					<dd class="mono">{entry.target}</dd>
				</div>{/if}
		</dl>
		{#if buses.length}
			<p class="buses">
				<span class="label">{buses.length === 1 ? 'Bus' : 'Buses'}</span>
				{#each buses.slice(0, MAX_BUSES) as bus (bus)}
					{#if onbus}<button type="button" onclick={() => onbus(bus, entry)}>{bus}</button
						>{:else}<span class="bus">{bus}</span>{/if}
				{/each}
				{#if buses.length > MAX_BUSES}<span class="more">and {buses.length - MAX_BUSES} more</span
					>{/if}
			</p>
		{/if}
		{#if entry.hint}<p class="hint">{entry.hint}</p>{/if}
		{#if details.length}
			<details>
				<summary>Details</summary>
				<dl class="details">
					{#each details as [key, value] (key)}
						<div>
							<dt class="mono">{key}</dt>
							<dd class="mono">{value}</dd>
						</div>
					{/each}
				</dl>
			</details>
		{/if}
	</div>
</div>

<style>
	.entry {
		--log-warning: #8a5300;
		display: grid;
		grid-template-columns: 62px minmax(0, 1fr);
		gap: 8px;
		font: var(--fs-meta, 12px) / 1.45 var(--font-display);
		color: var(--text-primary, var(--ink));
	}
	:global(:root[data-theme='dark']) .entry {
		--log-warning: #f0c36d;
	}
	.severity {
		font: 600 10px/1.9 var(--font-mono);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-secondary);
	}
	[data-severity='error'] .severity {
		color: var(--text-danger, var(--red));
	}
	[data-severity='warning'] .severity {
		color: var(--log-warning);
	}
	.body {
		min-width: 0;
	}
	p {
		margin: 0;
	}
	.message {
		overflow-wrap: anywhere;
	}
	.count {
		margin-left: 6px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		gap: 2px 12px;
		margin: 3px 0 0;
		color: var(--text-secondary);
	}
	.meta div,
	.details div {
		display: flex;
		gap: 4px;
		min-width: 0;
	}
	dt {
		color: var(--text-tertiary, var(--text-secondary));
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 11px;
	}
	.buses {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px;
		margin-top: 4px;
	}
	.label,
	.more {
		color: var(--text-secondary);
	}
	.bus,
	.buses button {
		font: 11px var(--font-mono);
		padding: 1px 6px;
		border-radius: 3px;
		background: var(--surface-chip, var(--accent-soft));
		color: inherit;
		border: 0;
	}
	.buses button {
		cursor: pointer;
	}
	.buses button:hover {
		background: var(--accent-soft);
	}
	.buses button:focus-visible,
	summary:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.hint {
		margin-top: 4px;
		color: var(--text-secondary);
	}
	details {
		margin-top: 4px;
	}
	summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	.details {
		display: grid;
		gap: 2px;
		margin: 4px 0 0;
	}
</style>
