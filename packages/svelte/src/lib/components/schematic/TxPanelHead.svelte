<script lang="ts">
	import { getUiConfig } from '../../context.svelte.js';
	import { formulationLabel } from '../../format.js';
	import { docsLink, type DOC_SECTIONS, type Lead } from '../../diagram/transmission-explain.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';

	/** The top of a result panel: the view's name, the calculation, whether the result is
	 * current, and the plain reading of it with the reason and a docs link. */
	let {
		title,
		formulation,
		context,
		lead,
		view,
		brief = false
	}: {
		title: string;
		formulation: Parameters<typeof formulationLabel>[0];
		context: ReturnType<typeof operatingContext>;
		lead: Lead;
		view: keyof typeof DOC_SECTIONS;
		/** Collapsed panel: the reading alone, and freshness only when stale. */
		brief?: boolean;
	} = $props();
	const config = getUiConfig();
</script>

<div class="heading">
	<strong>{title}</strong><span>{formulationLabel(formulation)}</span>{#if context.relaxed}<strong
			class="relaxation">Relaxation</strong
		>{/if}
</div>
<p class={['freshness', context.stale && 'stale']} role="status" hidden={brief && !context.stale}>
	{context.freshness}. Solver status: {context.status}. Base case.
</p>
<div class="lead {lead.tone}">
	<p class="text">{lead.text}</p>
	{#if !brief}
		<p class="why">
			{lead.why}
			<a href={docsLink(config.docsHref, view)} target="_blank" rel="noreferrer"
				>How to read this view</a
			>
		</p>
	{/if}
</div>

<style>
	.heading {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-lg);
		align-items: center;
		font-family: var(--font-display);
		font-size: var(--fs-label);
	}
	.heading strong {
		color: var(--ink);
	}
	.heading .relaxation,
	.stale {
		color: var(--text-accent);
	}
	.freshness {
		margin: var(--space-3xs) 0;
		font-family: var(--font-display);
		font-size: var(--fs-control);
		color: var(--text-secondary);
	}
	.lead {
		margin: var(--space-2xs) 0 var(--space-xs);
		padding-left: var(--space-sm);
		border-left: 3px solid var(--line);
		font-family: var(--font-display);
	}
	.lead.ok {
		/* no token names a success colour; this muted green reads on both themes */
		border-left-color: #6b8f5e;
	}
	.lead.warning {
		border-left-color: var(--accent);
	}
	.lead.violation {
		border-left-color: var(--red);
	}
	.lead p {
		margin: var(--space-3xs) 0;
		line-height: var(--lh-snug);
	}
	.text {
		font-size: var(--fs-data);
		font-weight: 600;
		color: var(--ink);
	}
	.why {
		font-size: var(--fs-label);
		color: var(--text-secondary);
	}
	a {
		color: inherit;
		white-space: nowrap;
	}
</style>
