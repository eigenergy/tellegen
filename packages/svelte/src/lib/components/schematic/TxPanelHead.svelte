<script lang="ts">
	import { getUiConfig } from '../../context.svelte.js';
	import { docsLink, type DOC_SECTIONS, type Lead } from '../../diagram/transmission-explain.js';
	import { freshnessNote, type operatingContext } from '../../diagram/transmission-results.js';
	import StatBoxes from '../StatBoxes.svelte';

	/** The top of a result panel: the view's name, a docs link, a status chip when the result is
	 * not current, and the view's key numbers. */
	let {
		title,
		context,
		lead,
		view
	}: {
		title: string;
		context: ReturnType<typeof operatingContext>;
		lead: Lead;
		view: keyof typeof DOC_SECTIONS;
	} = $props();
	const config = getUiConfig();
	const note = $derived(freshnessNote(context));
</script>

<div class="heading">
	<strong>{title}</strong>
	{#if note}<span class={['chip', context.stale && 'stale']} role="status">{note}</span>{/if}
	<a
		class="docs"
		href={docsLink(config.docsHref, view)}
		target="_blank"
		rel="noreferrer"
		aria-label="{title} docs">Docs</a
	>
</div>
{#if lead.empty}
	<p class="empty">{lead.empty}</p>
{:else}
	<StatBoxes stats={lead.stats} label={title} />
{/if}

<style>
	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-bottom: var(--space-xs);
		font-size: var(--fs-label);
	}
	.heading strong {
		color: var(--ink);
		font-size: var(--fs-body);
	}
	.chip {
		padding: 1px 8px;
		border-radius: var(--radius-full);
		background: var(--surface-chip);
		color: var(--text-secondary);
		white-space: nowrap;
	}
	.chip.stale {
		color: var(--text-accent);
		background: var(--accent-soft);
	}
	.docs {
		margin-left: auto;
		color: var(--text-accent);
		text-decoration: none;
	}
	.docs:hover {
		text-decoration: underline;
	}
	.empty {
		margin: 0 0 var(--space-xs);
		color: var(--text-secondary);
	}
</style>
