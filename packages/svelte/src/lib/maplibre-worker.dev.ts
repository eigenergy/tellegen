/** The dev server's `@tellegen/svelte/maplibre-worker`, chosen by the
 * `development` export condition.
 *
 * Vite's dev server pre-bundles this package with Rolldown, which cannot load
 * the `?worker&url` import in `maplibre-worker.ts`, so a static import of that
 * module would stop the dev server at startup. Here it is imported at run time
 * by URL instead, outside the pre-bundle, and the dev server resolves the
 * query itself. Vite rewrites the `new URL` in a pre-bundled file to point at
 * the original. A production build takes the default condition and imports
 * `maplibre-worker.ts` directly. */
export async function maplibreWorkerUrl(): Promise<string> {
	const url = new URL('./maplibre-worker.js', import.meta.url).href;
	const module: typeof import('./maplibre-worker.js') = await import(/* @vite-ignore */ url);
	return module.maplibreWorkerUrl();
}
