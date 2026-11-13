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
	// Chrome and Edge run separate WebMCP trials, each with its own token, so the
	// setting may hold several, separated by commas or whitespace (tokens are base64).
	// Svelte's meta typings predate origin trials, so the attributes go through a spread.
	const originTrials: Record<string, string>[] = (PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN ?? '')
		.split(/[\s,]+/)
		.filter(Boolean)
		.map((content) => ({ 'http-equiv': 'origin-trial', content }));
</script>

<svelte:head>
	<!-- Enables WebMCP (document.modelContext) for visitors during the browser origin trials. -->
	{#each originTrials as originTrial (originTrial.content)}
		<meta {...originTrial} />
	{/each}
</svelte:head>

<Analytics />
<TellegenProvider
	acOpfWasmUrl={PUBLIC_TELLEGEN_ACOPF_WASM_URL}
	cartoApiKey={PUBLIC_CARTO_BASEMAP_KEY}
>
	{@render children()}
</TellegenProvider>
