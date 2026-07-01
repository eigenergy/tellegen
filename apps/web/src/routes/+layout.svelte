<script lang="ts">
	import '@fontsource-variable/bricolage-grotesque';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '@tellegen/svelte/styles.css';
	import { TellegenProvider } from '@tellegen/svelte';

	let { children } = $props();
</script>

<TellegenProvider>
	{@render children()}
</TellegenProvider>
