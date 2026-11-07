import type { MapTheme } from '../../../../../packages/svelte/src/lib/basemap.js';

/** The harness basemap: one plain tone per theme, so the spec can tell every
 * marker pixel from it. */
export const BACKGROUND: Record<MapTheme, [number, number, number]> = {
	light: [255, 255, 255],
	dark: [24, 26, 31]
};
