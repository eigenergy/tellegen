import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url';

/** The URL of MapLibre's worker, which the host's Vite build bundles with its
 * shared chunk. `TellegenMap` hands it to `setWorkerUrl` before it creates a
 * map. Reached through the `@tellegen/svelte/maplibre-worker` export; see
 * `maplibre-worker.dev.ts` for the dev server. */
export async function maplibreWorkerUrl(): Promise<string> {
	return workerUrl;
}
