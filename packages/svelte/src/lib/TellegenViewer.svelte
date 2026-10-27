<script lang="ts">
	import TellegenProvider from './TellegenProvider.svelte';
	import TellegenShell from './TellegenShell.svelte';
	import type { TellegenUiOptions } from './context.svelte.js';

	let {
		apiBase,
		acOpfWasmUrl,
		loadDefaultCases,
		docsHref,
		orgHref,
		orgLabel,
		showFooter,
		theme,
		cartoApiKey
	}: TellegenUiOptions = $props();
</script>

<TellegenProvider
	{apiBase}
	{acOpfWasmUrl}
	{loadDefaultCases}
	{docsHref}
	{orgHref}
	{orgLabel}
	{showFooter}
	{theme}
	{cartoApiKey}
>
	<TellegenShell />
</TellegenProvider>
