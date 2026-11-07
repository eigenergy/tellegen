import type { NetworkBus } from './api.js';

/** The `extras` key under which tellegen-synth records each element's
 * annotation in the modules it writes. */
const SYNTH_EXTRAS = 'tellegen_synth';

/** Each bus's kind as a generated grid records it, by bus id: `generator`,
 * `substation`, `converter`, `load` or `tap`. tellegen-synth writes the kind
 * in each bus row's `extras.tellegen_synth.kind`; the map view does not carry
 * it. Empty for any other module, and for text that does not parse. */
export function readBusKinds(moduleJson: string | null | undefined): ReadonlyMap<number, string> {
	const kinds = new Map<number, string>();
	// Only a module tellegen-synth wrote is parsed a second time.
	if (!moduleJson?.includes(`"${SYNTH_EXTRAS}"`)) return kinds;
	let buses: unknown;
	try {
		buses = JSON.parse(moduleJson)?.value?.data?.buses;
	} catch {
		return kinds;
	}
	if (!Array.isArray(buses)) return kinds;
	for (const bus of buses) {
		const kind = bus?.extras?.[SYNTH_EXTRAS]?.kind;
		if (typeof bus?.id === 'number' && typeof kind === 'string') kinds.set(bus.id, kind);
	}
	return kinds;
}

/** Ids of the buses in `kinds` that are line taps: points where a line is
 * tapped, with no load or machine. */
export function tapsOf(kinds: ReadonlyMap<number, string>): ReadonlySet<number> {
	const ids = new Set<number>();
	for (const [id, kind] of kinds) if (kind === 'tap') ids.add(id);
	return ids;
}

/** Ids of the buses a generated grid marks as line taps. Empty for any other
 * module, and for text that does not parse. */
export function tapBusIds(moduleJson: string | null | undefined): ReadonlySet<number> {
	return tapsOf(readBusKinds(moduleJson));
}

/** `buses` less the taps, the ones the map draws as markers. The same array
 * when there is no tap to drop, so the map layer keeps its data. */
export function withoutTaps(buses: NetworkBus[], taps: ReadonlySet<number>): NetworkBus[] {
	return taps.size === 0 ? buses : buses.filter((b) => !taps.has(b.id));
}
