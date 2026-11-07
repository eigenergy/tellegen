<script lang="ts">
	import TellegenProvider from '../../../../../packages/svelte/src/lib/TellegenProvider.svelte';
	import type { MapProjection, MapTheme } from '../../../../../packages/svelte/src/lib/basemap.js';
	import Host from './DeclutterHost.svelte';

	let {
		projection,
		theme,
		declutter
	}: { projection: MapProjection; theme: MapTheme; declutter: boolean } = $props();
</script>

<TellegenProvider>
	<Host {projection} {theme} {declutter} />
</TellegenProvider>
