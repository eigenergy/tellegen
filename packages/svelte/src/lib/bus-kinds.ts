import type { NetworkBus } from './api.js';
import { readModuleText } from './module-text.js';

/** Each bus's kind as a generated grid records it, by bus id: `generator`,
 * `substation`, `converter`, `load` or `tap`. tellegen-synth writes the kind
 * in each bus row's `extras.tellegen_synth.kind`; the map view does not carry
 * it. Empty for any other module, and for text that does not parse. */
export function readBusKinds(moduleJson: string | null | undefined): ReadonlyMap<number, string> {
	return readModuleText(moduleJson).busKinds;
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
