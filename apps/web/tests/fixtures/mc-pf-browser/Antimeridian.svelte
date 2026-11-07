<script lang="ts">
	import TellegenProvider from '../../../../../packages/svelte/src/lib/TellegenProvider.svelte';
	import type { MapProjection } from '../../../../../packages/svelte/src/lib/basemap.js';
	import Host from './AntimeridianHost.svelte';

	let { projection }: { projection: MapProjection } = $props();
</script>

<TellegenProvider>
	<Host {projection} />
</TellegenProvider>
