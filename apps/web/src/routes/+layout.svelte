<script lang="ts">
	// Name the stylesheet, not the package root: a specifier with no extension
	// cannot match the ambient `*.css` declaration. Same file, via `./*.css`.
	import '@fontsource-variable/bricolage-grotesque/index.css';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '@tellegen/svelte/styles.css';
	import { TellegenProvider } from '@tellegen/svelte';
	import { PUBLIC_CARTO_BASEMAP_KEY } from '$app/env/public';
	import Analytics from '#lib/analytics/Analytics.svelte';
	import { PUBLIC_TELLEGEN_ACOPF_WASM_URL } from '$app/env/public';
	import { PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN } from '$app/env/public';

	let { children } = $props();
	// Svelte's meta typings predate origin trials, so the attributes go through a spread.
	const originTrial: Record<string, string> = {
		'http-equiv': 'origin-trial',
		content: PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN
	};
</script>

<svelte:head>
	{#if PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN}
		<!-- Enables WebMCP (document.modelContext) for visitors during the browser origin trial. -->
		<meta {...originTrial} />
	{/if}
</svelte:head>

<Analytics />
<TellegenProvider
	acOpfWasmUrl={PUBLIC_TELLEGEN_ACOPF_WASM_URL}
	cartoApiKey={PUBLIC_CARTO_BASEMAP_KEY}
>
	{@render children()}
</TellegenProvider>
