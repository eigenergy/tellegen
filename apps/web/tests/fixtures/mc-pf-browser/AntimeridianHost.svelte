<script lang="ts">
	import { onMount } from 'svelte';
	import type { Map as MapLibreMap } from 'maplibre-gl';
	import TellegenMap from '../../../../../packages/svelte/src/lib/TellegenMap.svelte';
	import type { MapProjection } from '../../../../../packages/svelte/src/lib/basemap.js';
	import {
		getAppState,
		getController
	} from '../../../../../packages/svelte/src/lib/context.svelte.js';

	let { projection }: { projection: MapProjection } = $props();
	const app = getAppState();
	const ctrl = getController();
	let map: MapLibreMap | null = null;

	onMount(() => {
		window.antimeridian = {
			loaded: () => app.active?.network != null,
			selectBranch: (id) => ctrl.selectBranch(app.activeCaseId!, id, { focus: true }),
			selectBus: (id) => ctrl.selectBus(app.activeCaseId!, id),
			selected: () => app.selectedBranch,
			map: () => map
		};
		void ctrl.load();
	});
</script>

<TellegenMap
	{projection}
	onbusclick={ctrl.selectBus}
	onlocalbusclick={ctrl.selectLocalBus}
	onbranchclick={ctrl.selectBranch}
	onlocalbranchclick={ctrl.selectLocalBranch}
	onmultibusclick={ctrl.selectMultiBus}
	onmultiedgeclick={ctrl.selectMultiEdge}
	onplacecase={ctrl.placeCase}
	onmapclick={ctrl.clearSelection}
	onMapReady={(m) => {
		map = m;
	}}
/>
