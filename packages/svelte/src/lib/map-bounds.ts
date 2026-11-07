export type MapBounds = [[number, number], [number, number]];

/** Half-width given to an axis whose extent collapsed to a single value. A
 * zero-span box makes `fitBounds` divide by zero, clamp to the map's maximum
 * zoom, and slam the camera onto one point instead of framing it. */
const DEGENERATE_PAD_DEGREES = 0.005;

/** Whether a point can be framed on a Web Mercator map: finite, longitude in
 * [-180, 180], latitude inside the projection's ±85.05° limit. */
export function isMapPoint([lon, lat]: readonly [number, number]): boolean {
	return (
		Number.isFinite(lon) &&
		Number.isFinite(lat) &&
		lon >= -180 &&
		lon <= 180 &&
		lat >= -85.05 &&
		lat <= 85.05
	);
}

/** Fold finite Web Mercator coordinates into map bounds. Invalid points are
 * ignored so one malformed row cannot hide an otherwise usable network, and a
 * single surviving point still yields a box `fitBounds` can frame. */
export function foldMapBounds(points: Iterable<readonly [number, number]>): MapBounds | null {
	const kept: (readonly [number, number])[] = [];
	for (const point of points) if (isMapPoint(point)) kept.push(point);
	return spanBounds(kept);
}

/** The box around points already checked, or `null` when there are none. An
 * axis that collapsed to a single value is padded so `fitBounds` can frame
 * it. Longitudes are taken as they are, so an unwrapped route may give a west
 * edge below -180 or an east edge above 180. */
export function spanBounds(points: Iterable<readonly [number, number]>): MapBounds | null {
	let minLon = Infinity;
	let minLat = Infinity;
	let maxLon = -Infinity;
	let maxLat = -Infinity;
	for (const [lon, lat] of points) {
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
