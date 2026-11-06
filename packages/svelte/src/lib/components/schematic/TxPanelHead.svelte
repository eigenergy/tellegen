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
<p class:stale={context.stale} class="freshness" role="status" hidden={brief && !context.stale}>
	{context.freshness} · {context.status} · base case
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
		gap: 4px 12px;
		align-items: center;
	}
	.heading strong {
		color: var(--ink);
	}
	.heading .relaxation,
	.stale {
		color: #925b05;
	}
	.freshness {
		margin: 3px 0;
		font-size: 10px;
	}
	.lead {
		margin: 4px 0 6px;
		padding-left: 8px;
		border-left: 3px solid var(--line);
		font-family: var(--font-display);
	}
	.lead.ok {
		border-left-color: #6b8f5e;
	}
	.lead.warning {
		border-left-color: #c98a12;
	}
	.lead.violation {
		border-left-color: #be2024;
	}
	.lead p {
		margin: 2px 0;
		line-height: 1.4;
	}
	.text {
		font-size: 13px;
		font-weight: 600;
		color: var(--ink);
	}
	.why {
		font-size: 12px;
		color: var(--ink);
		opacity: 0.85;
	}
	a {
		color: inherit;
		white-space: nowrap;
	}
</style>
