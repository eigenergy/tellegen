<script lang="ts">
	import type { DynRunRecord } from '../../dynamics/controller.svelte.js';
	import { getUiConfig } from '../../context.svelte.js';
	import { announcement, headline, rows } from '../../dynamics/verdict.js';

	let { record }: { record: DynRunRecord } = $props();
	const s = $derived(record.run?.screening ?? null);
	const h = $derived(headline(s, record.failure, record.tEnd));
	const table = $derived(s ? rows(s, record.events.length) : []);
	const said = $derived(announcement(s, record.tEnd, record.events.length));
	const config = getUiConfig();
	const docs = $derived(`${config.docsHref.replace(/\/?$/, '/')}rms-validation.html`);
</script>

<section class="verdict" aria-labelledby="dyn-verdict-h" data-testid="dyn-verdict">
	<p class="sr-only" role="status">{record.failure ? h.word : said}</p>
	<h3 id="dyn-verdict-h" class={['headline', h.tone]}>
		<span aria-hidden="true" class="icon">{h.icon}</span>
		<span data-testid="dyn-verdict-word">{h.word}</span>
		{#if h.detail}<span class="detail">{h.detail}</span>{/if}
	</h3>
	{#if s}<a class="caveat" data-testid="dyn-caveat" href={docs} target="_blank" rel="noreferrer"
			>Screening only</a
		>{/if}
	<div class="badges">
		{#if record.badge}
			<span
				class={['badge', record.badge.validated ? 'validated' : 'differs']}
				data-testid="dyn-badge"
				title={record.badge.detail}>{record.badge.validated ? '✓ ' : ''}{record.badge.text}</span
			>
		{/if}
		{#if record.network === 'validation'}<span class="badge" data-testid="dyn-network"
				>Validation loads</span
			>{/if}
		{#if record.agent}<span class="badge agent">Agent</span>{/if}
	</div>
	{#if record.failure}
		<details class="failure">
			<summary>Details</summary>
			<p>{record.failure.message}</p>
		</details>
	{:else if s}
		<table>
			<thead><tr><th>Metric</th><th>Value</th><th><span class="sr-only">Pass</span></th></tr></thead
			>
			<tbody>
				{#each table as r (r.metric)}
					<tr>
						<th scope="row">{r.metric}</th>
						<td>{r.value}</td>
						<td
							class={['mark', r.status === '✓' ? 'ok' : r.status === '✗' ? 'bad' : '']}
							aria-label={r.status === '✓' ? 'meets' : r.status === '✗' ? 'fails' : 'reported'}
							>{r.status}</td
						>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</section>

<style>
	.verdict {
		display: grid;
		gap: 6px;
	}
	.headline {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 6px;
		margin: 0;
		padding: 6px 8px;
		border-left: 4px solid var(--line);
		font-size: var(--fs-title);
	}
	.headline.ok {
		border-left-color: var(--status-ok);
	}
	.headline.warning {
		border-left-color: var(--warn);
	}
	.headline.failure {
		border-left-color: var(--text-danger);
	}
	.headline.failure .icon {
		color: var(--text-danger);
	}
	.headline.ok .icon {
		color: var(--status-ok);
	}
	.detail {
		font-size: var(--fs-label);
		color: var(--text-secondary);
	}
	.badges {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.badge {
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		font-size: var(--fs-control);
		line-height: 1.35;
		background: rgb(var(--ink-rgb) / 0.05);
		border-left: 3px solid var(--line);
	}
	.badge.validated {
		border-left-color: var(--status-ok);
	}
	.badge.differs {
		border-left-color: var(--warn);
	}
	.badge.agent {
		border-left-color: var(--agent);
		background: var(--agent-soft);
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--fs-control);
	}
	th,
	td {
		padding: 3px 4px;
		text-align: left;
		vertical-align: top;
		border-bottom: 1px solid var(--line);
	}
	thead th {
		color: var(--text-tertiary);
		font-weight: 500;
	}
	tbody th {
		font-weight: 600;
		white-space: nowrap;
	}
	td {
		font-variant-numeric: tabular-nums;
	}
	.mark {
		font-weight: 700;
	}
	.mark.ok {
		color: var(--status-ok);
	}
	.mark.bad {
		color: var(--text-danger);
	}
	.failure p {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		white-space: pre-wrap;
	}
	details {
		font-size: var(--fs-control);
		color: var(--text-secondary);
	}
	.caveat {
		justify-self: start;
		font-size: var(--fs-control);
		color: var(--text-accent);
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
	}
</style>
