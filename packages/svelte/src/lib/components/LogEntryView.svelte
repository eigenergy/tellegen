<script lang="ts">
	import { logTitle, type LogEntry, type LogEntryInput } from '../log.svelte.js';
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
	const title = $derived(logTitle(entry.message));
	const details = $derived(
		Object.entries(entry.details ?? {}).map(
			([key, value]) => [key, typeof value === 'string' ? value : JSON.stringify(value)] as const
		)
	);
	/** Everything past the title, for Details: the reader's own words and identifiers. */
	const rows = $derived(
		[
			['Message', title !== entry.message ? entry.message : ''],
			['From', entry.source !== 'tellegen' ? entry.source : ''],
			['Code', entry.code ?? ''],
			['In', showSubject ? (entry.subject ?? '') : ''],
			['Element', entry.target ?? ''],
			['Suggestion', entry.hint ?? ''],
			...details.map(
				([key, value]) => [key.charAt(0).toUpperCase() + key.slice(1).replaceAll('_', ' '), value] as const
			)
		].filter(([, value]) => value) as [string, string][]
	);
</script>

<div class="entry" data-severity={entry.severity} data-testid="log-entry">
	<span class="severity">{LABEL[entry.severity]}</span>
	<div class="body">
		<p class="message" title={entry.message}>
			{title}{#if count > 1}<span class="count" aria-label={`reported ${count} times`}
					>{count} times</span
				>{/if}
		</p>
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
		{#if rows.length}
			<details>
				<summary>Details</summary>
				<dl class="details">
					{#each rows as [key, value] (key)}
						<div>
							<dt>{key}</dt>
							<dd>{value}</dd>
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
		font: 600 var(--fs-meta, 12px) / 1.45 var(--font-display);
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
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.count {
		margin-left: 6px;
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}
	.details div {
		display: grid;
		grid-template-columns: 76px minmax(0, 1fr);
		gap: 8px;
		min-width: 0;
	}
	dt {
		color: var(--text-tertiary, var(--text-secondary));
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
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
		font: inherit;
		font-variant-numeric: tabular-nums;
		padding: 1px 6px;
		border-radius: var(--radius-control);
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
		color: var(--text-secondary);
	}
</style>
