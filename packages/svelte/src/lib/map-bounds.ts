export type MapBounds = [[number, number], [number, number]];

/** Fold finite Web Mercator coordinates into map bounds. Invalid points are
 * ignored so one malformed row cannot hide an otherwise usable network. */
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
	return [
		[minLon, minLat],
		[maxLon, maxLat]
	];
}
