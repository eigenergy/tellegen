import type { SchematicGeometry } from './schematic-layout.js';

/** One branch of a shared route, drawn spread out from the others. */
export interface FannedBranch {
	edge: number;
	/** Polyline in drawing units: it starts and ends on the true route's ends and runs offset
	 * between them, so every branch still visibly meets its buses. */
	path: [number, number][];
	/** Where the branch's marks sit (the middle of the offset run) and its direction there. */
	mid: [number, number];
	dir: [number, number];
}

/** Lay the members of a shared route side by side: each is offset perpendicular to the route by a
 * fixed screen distance (`spreadPx` between neighbours), tapering to zero at both ends over at most
 * `taperPx` or a fifth of the route. `scale` is pixels per drawing unit. Members keep their order,
 * centred on the true route. */
export function fanRoute(
	members: number[],
	host: Int32Array,
	geometry: SchematicGeometry,
	scale: number,
	options: { spreadPx?: number; taperPx?: number } = {}
): FannedBranch[] {
	const spread = (options.spreadPx ?? 7) / scale;
	const taperMax = (options.taperPx ?? 14) / scale;
	const s = host[members[0]];
	if (s < 0) return [];
	const a: [number, number] = [geometry.sources[s * 3], geometry.sources[s * 3 + 1]];
	const b: [number, number] = [geometry.targets[s * 3], geometry.targets[s * 3 + 1]];
	const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
	if (!(len > 0)) return [];
	const [ux, uy] = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
	const [nx, ny] = [-uy, ux];
	const taper = Math.min(taperMax, len / 5);
	return members.map((edge, i) => {
		const off = (i - (members.length - 1) / 2) * spread;
		const p1: [number, number] = [a[0] + ux * taper + nx * off, a[1] + uy * taper + ny * off];
		const p2: [number, number] = [b[0] - ux * taper + nx * off, b[1] - uy * taper + ny * off];
		return {
			edge,
			path: [a, p1, p2, b],
			mid: [(p1[0] + p2[0]) / 2, (p1[1] + p2[1]) / 2],
			dir: [ux, uy]
		};
	});
}

/** The middle of a shared route, for its count badge and its member card. */
export function routeMiddle(
	members: number[],
	host: Int32Array,
	geometry: SchematicGeometry
): [number, number] | null {
	const s = host[members[0]];
	if (s < 0) return null;
	return [
		(geometry.sources[s * 3] + geometry.targets[s * 3]) / 2,
		(geometry.sources[s * 3 + 1] + geometry.targets[s * 3 + 1]) / 2
	];
}
