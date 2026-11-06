import type { NetworkBus } from './api.js';

/** The `extras` key under which tellegen-synth records each element's
 * annotation in the modules it writes. */
const SYNTH_EXTRAS = 'tellegen_synth';

/** Ids of the buses a generated grid marks as line taps: points where a line
 * is tapped, with no load or machine. tellegen-synth records the kind in
 * each bus row's `extras.tellegen_synth.kind`; the map view does not carry
 * it. Empty for any other module, and for text that does not parse. */
export function tapBusIds(moduleJson: string | null | undefined): ReadonlySet<number> {
	const ids = new Set<number>();
	// Only a module that names a tap is parsed a second time.
	if (!moduleJson?.includes('"tap"')) return ids;
	let buses: unknown;
	try {
		buses = JSON.parse(moduleJson)?.value?.data?.buses;
	} catch {
		return ids;
	}
	if (!Array.isArray(buses)) return ids;
	for (const bus of buses) {
		if (typeof bus?.id === 'number' && bus.extras?.[SYNTH_EXTRAS]?.kind === 'tap') ids.add(bus.id);
	}
	return ids;
}

/** `buses` less the taps, the ones the map draws as markers. The same array
 * when there is no tap to drop, so the map layer keeps its data. */
export function withoutTaps(buses: NetworkBus[], taps: ReadonlySet<number>): NetworkBus[] {
	return taps.size === 0 ? buses : buses.filter((b) => !taps.has(b.id));
}
