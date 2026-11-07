import { mount } from 'svelte';
import type { Map as MapLibreMap } from 'maplibre-gl';
import '../../../../../packages/svelte/src/lib/styles.css';
import Antimeridian from './Antimeridian.svelte';

// `?projection=globe` mounts the map on the globe; anything else is flat.
const projection =
	new URLSearchParams(location.search).get('projection') === 'globe' ? 'globe' : 'mercator';
mount(Antimeridian, { target: document.getElementById('app')!, props: { projection } });

declare global {
	interface Window {
		antimeridian: {
			loaded(): boolean;
			selectBranch(id: number): Promise<void>;
			selectBus(id: number): Promise<void> | void;
			selected(): number | null;
			map(): MapLibreMap | null;
		};
	}
}
