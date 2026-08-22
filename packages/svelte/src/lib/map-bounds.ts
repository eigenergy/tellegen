export type MapBounds = [[number, number], [number, number]];

/** Half-width given to an axis whose extent collapsed to a single value. A
 * zero-span box makes `fitBounds` divide by zero, clamp to the map's maximum
 * zoom, and slam the camera onto one point instead of framing it. */
const DEGENERATE_PAD_DEGREES = 0.005;

/** Fold finite Web Mercator coordinates into map bounds. Invalid points are
 * ignored so one malformed row cannot hide an otherwise usable network, and a
 * single surviving point still yields a box `fitBounds` can frame. */
export function foldMapBounds(points: Iterable<readonly [number, number]>): MapBounds | null {
	let minLon = Infinity;
	let minLat = Infinity;
	let maxLon = -Infinity;
	let maxLat = -Infinity;
	for (const [lon, lat] of points) {
		if (
			!Number.isFinite(lon) ||
			!Number.isFinite(lat) ||
			lon < -180 ||
			lon > 180 ||
			lat < -85.05 ||
			lat > 85.05
		) {
			continue;
		}
		minLon = Math.min(minLon, lon);
		minLat = Math.min(minLat, lat);
		maxLon = Math.max(maxLon, lon);
		maxLat = Math.max(maxLat, lat);
	}
	if (!Number.isFinite(minLon) || !Number.isFinite(minLat)) return null;
	if (minLon === maxLon) {
		minLon -= DEGENERATE_PAD_DEGREES;
		maxLon += DEGENERATE_PAD_DEGREES;
	}
	if (minLat === maxLat) {
		minLat -= DEGENERATE_PAD_DEGREES;
		maxLat += DEGENERATE_PAD_DEGREES;
	}
	return [
		[minLon, minLat],
		[maxLon, maxLat]
	];
}
