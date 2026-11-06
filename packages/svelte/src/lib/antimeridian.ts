/** Coordinate helpers for geographic paths drawn across the antimeridian.
 *
 * Stored longitudes stay in [-180, 180]. A drawn path is unwrapped so each
 * vertex lies within 180° of the one before, and every segment takes the
 * short way across ±180 instead of crossing the whole map.
 */

import { isMapPoint, spanBounds, type MapBounds } from './map-bounds.js';

export type LngLat = readonly [number, number];

/** `lon` moved by whole turns of 360° to lie within 180° of `reference`.
 * Returned unchanged when either value is not finite. */
export function nearestLongitude(lon: number, reference: number): number {
	const delta = lon - reference;
	if (!Number.isFinite(delta) || Math.abs(delta) <= 180) return lon;
	return lon - 360 * Math.round(delta / 360);
}

/** Return a display path whose consecutive longitudes differ by at most 180°.
 * The returned longitudes may lie outside [-180, 180]; use only for drawing,
 * never for serialization or exported GeoJSON. A vertex whose longitude is
 * not finite is copied as it is and does not anchor the next one. */
export function unwrapLongitudePath(path: readonly LngLat[]): [number, number][] {
	const out: [number, number][] = [];
	let previous: number | null = null;
	for (const [rawLongitude, latitude] of path) {
		if (!Number.isFinite(rawLongitude)) {
			out.push([rawLongitude, latitude]);
			continue;
		}
		const longitude: number =
			previous === null ? rawLongitude : nearestLongitude(rawLongitude, previous);
		out.push([longitude, latitude]);
		previous = longitude;
	}
	return out;
}

/** Bounds of a stored route around its short arc, so a seam-crossing route
 * frames the few degrees it spans rather than the whole world. West may lie
 * below -180 or east above 180. Vertices `foldMapBounds` ignores are ignored
 * here too. */
export function routeBounds(path: readonly LngLat[]): MapBounds | null {
	return spanBounds(unwrapLongitudePath(path.filter(isMapPoint)));
}
