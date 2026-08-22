<script lang="ts">
	// Name the stylesheet, not the package root: a specifier with no extension
	// cannot match the ambient `*.css` declaration. Same file, via `./*.css`.
	import '@fontsource-variable/bricolage-grotesque/index.css';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '@tellegen/svelte/styles.css';
	import { TellegenProvider } from '@tellegen/svelte';

	let { children } = $props();
</script>

<TellegenProvider>
	{@render children()}
</TellegenProvider>
