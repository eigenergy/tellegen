/** Bus fields the declutter reads. `NetworkBus` has all of them. */
export interface DeclutterBus {
	id: number;
	lon: number;
	lat: number;
	demand_mw: number;
	gen_mw: number;
}

export interface DeclutterOptions {
	/** Map zoom the cells are laid out at. Pass a bucketed zoom
	 * (`declutterZoom`) so small camera moves keep the same result. */
	zoom: number;
	/** Cell side in CSS pixels. Each cell keeps one bus. */
	cellPx?: number;
	/** Bus kinds by id, as tellegen-synth records them (`generator`,
	 * `substation`, `converter`, `load`, `tap`). Without them, buses rank by
	 * power alone. */
	kinds?: ReadonlyMap<number, string>;
}

/** Default cell side in CSS pixels: about twice the diameter of the marker
 * for a bus of up to about 10 MW. */
export const DECLUTTER_CELL_PX = 16;

/** Zoom step the map lays its cells out at. */
export const DECLUTTER_ZOOM_STEP = 0.25;

// The Web Mercator world is this many CSS pixels across at zoom 0, as MapLibre
// and deck.gl lay it out.
const WORLD_PX = 512;
// Web Mercator's latitude limit, where the square world ends.
const MAX_LAT = 85.0511287798066;

/** `zoom` rounded down to a whole `step`, so panning and small zooms land on
 * the same value and leave the thinned markers as they are. */
export function declutterZoom(zoom: number, step = DECLUTTER_ZOOM_STEP): number {
	// Snap first, so 4.75 / 0.25 does not land a hair under 19.
	return Math.floor(Math.round((zoom / step) * 1e6) / 1e6) * step;
}

/** How strongly a bus kind claims its cell: a generator before a substation
 * or converter station, before anything else. */
export function kindRank(kind: string | undefined): number {
	if (kind === 'generator') return 2;
	if (kind === 'substation' || kind === 'converter') return 1;
	return 0;
}

function busPower(bus: DeclutterBus): number {
	const power = Math.abs(bus.gen_mw) + Math.abs(bus.demand_mw);
	return Number.isFinite(power) ? power : 0;
}

/** The buses a map at `zoom` draws markers for, so that a crowded grid does
 * not cover the map. Each bus is placed in Web Mercator world pixels (at a
 * MapLibre zoom the world is 512·2^zoom CSS pixels square) and binned into
 * square cells of `cellPx`; each cell keeps its strongest bus. A generator
 * outranks a substation or converter, which outranks any other bus. Then the
 * larger |generation| + |demand| wins, then the lower id, so the result does
 * not depend on the input order. The selection is not an input: `withBus`
 * adds a selected bus back on top, so selecting one never hides another.
 *
 * The kept buses come back in input order. When no two buses share a cell,
 * the input array itself comes back, so a map layer keeps its data. Longitudes
 * wrap, so 190 and -170 share a cell, and the cells tile the world with ±180
 * on a cell edge, so the seam thins like any other place. Latitudes clamp to
 * the Mercator limit. A bus without finite coordinates is kept, as there is no
 * cell to place it in. */
export function declutterBuses<T extends DeclutterBus>(buses: T[], options: DeclutterOptions): T[] {
	const { zoom, cellPx = DECLUTTER_CELL_PX, kinds } = options;
	if (buses.length < 2 || !Number.isFinite(zoom) || !(cellPx > 0)) return buses;
	const world = WORLD_PX * 2 ** zoom;
	// A whole number of columns, so the cells tile the world without a sliver
	// at the seam.
	const columns = Math.max(1, Math.round(world / cellPx));
	// Past about zoom 21 a 16 px cell is under a metre across, and its key
	// would outgrow exact integers. Nothing needs thinning there.
	if (!Number.isSafeInteger(columns * columns)) return buses;
	const cell = world / columns;
	const rank = (bus: T) => (kinds ? kindRank(kinds.get(bus.id)) : 0);
	const outranks = (a: T, b: T) => {
		const byKind = rank(a) - rank(b);
		if (byKind !== 0) return byKind > 0;
		const byPower = busPower(a) - busPower(b);
		if (byPower !== 0) return byPower > 0;
		return a.id < b.id;
	};

	const winners = new Map<number, T>();
	for (const bus of buses) {
		const { lon, lat } = bus;
		if (!Number.isFinite(lon) || !Number.isFinite(lat)) continue;
		const x = ((((lon + 180) % 360) + 360) % 360) / 360;
		const phi = (Math.max(-MAX_LAT, Math.min(MAX_LAT, lat)) * Math.PI) / 180;
		const y = 0.5 - Math.log(Math.tan(Math.PI / 4 + phi / 2)) / (2 * Math.PI);
		const column = Math.floor((x * world) / cell) % columns;
		const row = Math.min(columns - 1, Math.max(0, Math.floor((y * world) / cell)));
		const key = row * columns + column;
		const held = winners.get(key);
		if (!held || outranks(bus, held)) winners.set(key, bus);
	}

	const kept = new Set<T>(winners.values());
	const shown = buses.filter(
		(bus) => kept.has(bus) || !Number.isFinite(bus.lon) || !Number.isFinite(bus.lat)
	);
	return shown.length === buses.length ? buses : shown;
}

/** `shown` with bus `id` from `all` added at the end when it is missing, so a
 * selected bus that the map thinned out, or a line tap it never draws, still
 * gets its marker. `shown` itself when there is nothing to add. */
export function withBus<T extends { id: number }>(
	shown: T[],
	all: readonly T[],
	id: number | null | undefined
): T[] {
	if (id === null || id === undefined || shown.some((bus) => bus.id === id)) return shown;
	const bus = all.find((b) => b.id === id);
	return bus ? [...shown, bus] : shown;
}
