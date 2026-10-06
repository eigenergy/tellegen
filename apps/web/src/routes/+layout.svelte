<script lang="ts">
	// Name the stylesheet, not the package root: a specifier with no extension
	// cannot match the ambient `*.css` declaration. Same file, via `./*.css`.
	import '@fontsource-variable/bricolage-grotesque/index.css';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '@tellegen/svelte/styles.css';
	import { TellegenProvider } from '@tellegen/svelte';
	import Analytics from '#lib/analytics/Analytics.svelte';
	import { PUBLIC_TELLEGEN_ACOPF_WASM_URL } from '$app/env/public';

	let { children } = $props();
</script>

<Analytics />
<TellegenProvider acOpfWasmUrl={PUBLIC_TELLEGEN_ACOPF_WASM_URL}>
	{@render children()}
</TellegenProvider>
