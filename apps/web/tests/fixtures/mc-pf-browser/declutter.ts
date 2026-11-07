import { mount } from 'svelte';
import type { Map as MapLibreMap } from 'maplibre-gl';
import '../../../../../packages/svelte/src/lib/styles.css';
import Declutter from './Declutter.svelte';

// `?projection=globe` mounts the map on the globe, `?theme=dark` uses the dark
// palette and `?declutter=0` draws every marker.
const params = new URLSearchParams(location.search);
mount(Declutter, {
	target: document.getElementById('app')!,
	props: {
		projection: params.get('projection') === 'globe' ? 'globe' : 'mercator',
		theme: params.get('theme') === 'dark' ? 'dark' : 'light',
		declutter: params.get('declutter') !== '0'
	}
});

declare global {
	interface Window {
		declutter: {
			loaded(): boolean;
			map(): MapLibreMap | null;
			selectBus(id: number): Promise<void> | void;
			/** Ids of the layer's markers drawn on top anywhere on screen. */
			visible(layerId: string): number[];
			/** Id of the layer's marker drawn on top at a coordinate, if any. */
			topAt(layerId: string, lon: number, lat: number): number | null;
		};
	}
}
