/** Coordinate helpers for geographic paths rendered on a Web Mercator map.
 *
 * Source GeoJSON coordinates stay normalized to [-180, 180]. The display-only
 * path is unwrapped so adjacent points take the short route across the
 * antimeridian instead of jumping almost an entire world.
 */

export type LngLat = readonly [number, number];

/** Return a display path whose consecutive longitudes differ by at most 180°.
 * The returned longitudes may lie outside [-180, 180]; use only for drawing,
 * never for serialization or exported GeoJSON. */
export function unwrapLongitudePath(path: readonly LngLat[]): [number, number][] {
	if (path.length === 0) return [];
	const out: [number, number][] = [[path[0][0], path[0][1]]];
	let previous = path[0][0];
	for (const [rawLongitude, latitude] of path.slice(1)) {
		let longitude = rawLongitude;
		while (longitude - previous > 180) longitude -= 360;
		while (longitude - previous < -180) longitude += 360;
		out.push([longitude, latitude]);
		previous = longitude;
	}
	return out;
}

/** Bounds an already unwrapped route without folding its longitudes back into
 * [-180, 180], so a seam-crossing route frames the short arc. */
export function routeBounds(path: readonly LngLat[]): [[number, number], [number, number]] | null {
	let minLon = Infinity;
	let minLat = Infinity;
	let maxLon = -Infinity;
	let maxLat = -Infinity;
	for (const [lon, lat] of path) {
		if (!Number.isFinite(lon) || !Number.isFinite(lat) || lat < -85.05 || lat > 85.05) {
			continue;
		}
		minLon = Math.min(minLon, lon);
		minLat = Math.min(minLat, lat);
		maxLon = Math.max(maxLon, lon);
		maxLat = Math.max(maxLat, lat);
	}
	if (!Number.isFinite(minLon) || !Number.isFinite(minLat)) return null;
	if (minLon === maxLon) {
		minLon -= 0.005;
		maxLon += 0.005;
	}
	if (minLat === maxLat) {
		minLat -= 0.005;
		maxLat += 0.005;
	}
	return [[minLon, minLat], [maxLon, maxLat]];
}
