<script lang="ts">
	import type { DynRunRecord } from '../../dynamics/controller.svelte.js';
	import { getUiConfig } from '../../context.svelte.js';
	import { announcement, checks, details, headline, stats } from '../../dynamics/verdict.js';
	import StatBoxes from '../StatBoxes.svelte';

	let { record }: { record: DynRunRecord } = $props();
	const s = $derived(record.run?.screening ?? null);
	const h = $derived(headline(s, record.failure, record.tEnd));
	const numbers = $derived(s ? stats(s) : []);
	const passes = $derived(s ? checks(s) : []);
	const more = $derived(s ? details(s, record.events.length) : []);
	const said = $derived(announcement(s, record.tEnd));
	const config = getUiConfig();
	const docs = $derived(`${config.docsHref.replace(/\/?$/, '/')}rms-validation.html`);
</script>

<section class="verdict" aria-labelledby="dyn-verdict-h" data-testid="dyn-verdict">
	<p class="sr-only" role="status">{record.failure ? h.word : said}</p>
	<div class="head">
		<h3 id="dyn-verdict-h" class={['headline', h.tone]}>
			<span aria-hidden="true" class="icon">{h.icon}</span>
			<span data-testid="dyn-verdict-word">{h.word}</span>
			{#if h.detail}<span class="detail">{h.detail}</span>{/if}
		</h3>
		{#if record.agent}<span class="badge agent">Agent</span>{/if}
		{#if s}<a class="docs" href={docs} target="_blank" rel="noreferrer">Docs</a>{/if}
	</div>
	{#if record.failure}
		<details>
			<summary>Details</summary>
			<p class="raw">{record.failure.message}</p>
		</details>
	{:else if s}
		<StatBoxes stats={numbers} label="Simulation result" />
		<dl class="rows" aria-label="Checks">
			{#each passes as c (c.label)}
				<div>
					<dt>{c.label}</dt>
					<dd class={c.ok ? 'ok' : 'bad'}>{c.ok ? 'Yes' : 'No'}</dd>
				</div>
			{/each}
			{#if record.badge}
				<!-- A scenario of the validation study: whether this run repeats it as published. -->
				<div>
					<dt>Validation study</dt>
					<dd data-testid="dyn-badge" title={record.badge.detail}>{record.badge.text}</dd>
				</div>
			{/if}
		</dl>
		<details>
			<summary>Details</summary>
			<dl class="rows">
				{#each more as d (d.label)}
					<div>
						<dt>{d.label}</dt>
						<dd>{d.value}</dd>
					</div>
				{/each}
			</dl>
		</details>
	{/if}
</section>

<style>
	.verdict {
		display: grid;
		gap: 8px;
	}
	.verdict :global(dl.stats) {
		margin: 0;
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}
	.headline {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 6px;
		margin: 0;
		margin-right: auto;
		font-size: var(--fs-title);
		color: var(--text-primary);
	}
	.headline.ok .icon {
		color: var(--status-ok);
	}
	.headline.warning .icon {
		color: var(--amber);
	}
	.headline.failure .icon {
		color: var(--text-danger);
	}
	.detail {
		font-size: var(--fs-label);
		font-weight: 400;
		color: var(--text-secondary);
	}
	.badge {
		padding: 1px 8px;
		border-radius: var(--radius-full);
		font-size: var(--fs-label);
		background: var(--surface-chip);
		color: var(--text-primary);
	}
	.badge.agent {
		background: var(--agent-soft);
		color: var(--agent);
	}
	.docs {
		font-size: var(--fs-label);
		color: var(--text-accent);
		text-decoration: none;
	}
	.docs:hover {
		text-decoration: underline;
	}
	.rows {
		margin: 0;
		font-size: var(--fs-label);
		font-variant-numeric: tabular-nums;
	}
	.rows div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		min-height: 28px;
		align-items: center;
		border-bottom: 1px solid var(--line);
	}
	.rows dt {
		color: var(--text-primary);
	}
	.rows dd {
		margin: 0;
		min-width: 0;
		text-align: right;
		color: var(--text-secondary);
		overflow-wrap: anywhere;
	}
	.rows dd.ok {
		color: var(--text-primary);
	}
	.rows dd.bad {
		color: var(--red);
		font-weight: 600;
	}
	details {
		font-size: var(--fs-label);
	}
	summary {
		line-height: 28px;
		cursor: pointer;
		color: var(--text-primary);
	}
	.raw {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		color: var(--text-secondary);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
	}
</style>
