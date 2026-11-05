import { untrack } from 'svelte';
import {
	applyBasemapTuning,
	attachBasemapFallback,
	type BasemapProvider,
	type BasemapSpec
} from '../../basemap.js';
import { basemapCamera, ease, type DiagramCamera } from '../../diagram/index.js';

const FADE_MS = 700;

export interface UnderlayInputs {
	/** Show the tiles: geographic positions on screen and the viewer has not turned them off. */
	wanted: boolean;
	container: HTMLDivElement | undefined;
	spec: BasemapSpec;
	/** The Web Mercator point the drawing's positions are relative to. */
	origin: [number, number] | null;
	camera: DiagramCamera;
	/** Canvas size, so the map resizes with it. */
	size: [number, number];
}

/** Map tiles under a diagram's geographic positions, as on the main map. A non-interactive MapLibre
 * map is created the first time the tiles are wanted and follows the diagram camera only while
 * wanted: fading out, it stays where it was on screen as the drawing moves to another layout.
 * Construct during component initialisation; the effects end with the component. */
export class BasemapUnderlay {
	opacity = $state(0);
	/** Who serves the tiles on screen, for the credit line. */
	provider = $state<BasemapProvider>('custom');
	#map: import('maplibre-gl').Map | null = null;
	#loading = false;
	#destroyed = false;
	#frame = 0;

	constructor(inputs: () => UnderlayInputs) {
		// Only these change the fade and the map size; the camera changes every frame.
		const wanted = $derived(inputs().wanted);
		const size = $derived(inputs().size.join('x'));
		$effect(() => {
			const target = wanted ? 1 : 0;
			untrack(() => this.#fade(target));
		});
		$effect(() => {
			const { wanted, container } = inputs();
			if (!wanted || !container || this.#map || this.#loading) return;
			untrack(() => this.#create(container, inputs));
		});
		const follow = () => {
			const { wanted, origin, camera } = inputs();
			if (wanted && origin) this.#map?.jumpTo(basemapCamera(camera, origin));
		};
		$effect(() => {
			void size;
			untrack(() => {
				this.#map?.resize();
				follow();
			});
		});
		$effect(() => {
			follow();
		});
		$effect(() => () => {
			this.#destroyed = true;
			cancelAnimationFrame(this.#frame);
			this.#map?.remove();
			this.#map = null;
		});
	}

	#fade(target: number) {
		cancelAnimationFrame(this.#frame);
		const start = this.opacity;
		if (start === target) return;
		if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
			this.opacity = target;
			return;
		}
		const t0 = performance.now();
		const step = (now: number) => {
			const k = Math.min(1, (now - t0) / FADE_MS);
			this.opacity = start + (target - start) * ease(k);
			if (k < 1) this.#frame = requestAnimationFrame(step);
		};
		this.#frame = requestAnimationFrame(step);
	}

	#create(container: HTMLDivElement, inputs: () => UnderlayInputs) {
		this.#loading = true;
		const { spec } = inputs();
		this.provider = spec.primaryProvider ?? 'custom';
		Promise.all([
			import('maplibre-gl'),
			// As the map loads it: a package export keeps the worker out of the dev pre-bundle.
			import('@tellegen/svelte/maplibre-worker').then((m) => m.maplibreWorkerUrl())
		])
			.then(([maplibre, workerUrl]) => {
				// Torn down while the imports loaded: no map, so no WebGL context left behind.
				if (this.#destroyed) return;
				maplibre.setWorkerUrl(workerUrl);
				const { origin, camera } = inputs();
				const m = new maplibre.Map({
					container,
					style: spec.primary,
					interactive: false,
					attributionControl: false,
					...basemapCamera(camera, origin ?? [0, 0])
				});
				// The same style handling as the map: labels hidden when asked, OpenFreeMap tuned, and
				// the fallback style when the primary cannot be shown.
				m.on('style.load', () => applyBasemapTuning(m, spec, 'light'));
				attachBasemapFallback(m, spec, {
					onFallback: (reason) => (this.provider = reason.to ?? 'custom')
				});
				this.#map = m;
			})
			.catch(() => {
				this.#loading = false;
			});
	}
}
