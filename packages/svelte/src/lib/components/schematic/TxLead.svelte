<script lang="ts">
	import { getUiConfig } from '../../context.svelte.js';
	import { docsLink, type DOC_SECTIONS, type Lead } from '../../diagram/transmission-explain.js';

	let {
		lead,
		view,
		brief = false
	}: {
		lead: Lead;
		view: keyof typeof DOC_SECTIONS;
		/** Collapsed panel: the lead alone, without the explanation. */
		brief?: boolean;
	} = $props();
	const config = getUiConfig();
</script>

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
	p {
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
