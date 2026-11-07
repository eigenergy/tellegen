<script lang="ts">
	import { onMount } from 'svelte';
	import type { MapLibreOverlay } from '@deck.gl/maplibre';
	import type { Map as MapLibreMap, StyleSpecification } from 'maplibre-gl';
	import TellegenMap from '../../../../../packages/svelte/src/lib/TellegenMap.svelte';
	import type { MapProjection, MapTheme } from '../../../../../packages/svelte/src/lib/basemap.js';
	import {
		getAppState,
		getController
	} from '../../../../../packages/svelte/src/lib/context.svelte.js';
	import { BACKGROUND } from './declutter-background.js';

	let {
		projection,
		theme,
		declutter
	}: { projection: MapProjection; theme: MapTheme; declutter: boolean } = $props();
	const app = getAppState();
	const ctrl = getController();
	let map: MapLibreMap | null = null;
	let overlay: MapLibreOverlay | null = null;

	// A plain background in the theme's tone, so every other pixel is a marker.
	const style = $derived<StyleSpecification>({
		version: 8,
		sources: {},
		layers: [
			{
				id: 'background',
				type: 'background',
				paint: { 'background-color': `rgb(${BACKGROUND[theme].join(',')})` }
			}
		]
	});

	/** Ids of the objects drawn on top in a rectangle of the screen. */
	function idsIn(layerId: string, x: number, y: number, width: number, height: number) {
		return (overlay?.pickObjects({ x, y, width, height, layerIds: [layerId] }) ?? []).map(
			(info) => (info.object as { id: number }).id
		);
	}

	onMount(() => {
		window.declutter = {
			loaded: () => app.active?.network != null && map != null && overlay != null,
			map: () => map,
			selectBus: (id) => ctrl.selectBus(app.activeCaseId!, id),
			visible: (layerId) => {
				const canvas = map?.getCanvas();
				return canvas ? idsIn(layerId, 0, 0, canvas.clientWidth, canvas.clientHeight) : [];
			},
			topAt: (layerId, lon, lat) => {
				if (!map) return null;
				const { x, y } = map.project([lon, lat]);
				return idsIn(layerId, Math.round(x) - 1, Math.round(y) - 1, 3, 3)[0] ?? null;
			}
		};
		void ctrl.load();
	});
</script>

<TellegenMap
	{projection}
	{theme}
	mapStyle={style}
	fallbackMapStyle={null}
	declutterMarkers={declutter}
	onbusclick={ctrl.selectBus}
	onlocalbusclick={ctrl.selectLocalBus}
	onbranchclick={ctrl.selectBranch}
	onlocalbranchclick={ctrl.selectLocalBranch}
	onmultibusclick={ctrl.selectMultiBus}
	onmultiedgeclick={ctrl.selectMultiEdge}
	onplacecase={ctrl.placeCase}
	onmapclick={ctrl.clearSelection}
	onMapReady={(m, o) => {
		map = m;
		overlay = o;
	}}
/>
