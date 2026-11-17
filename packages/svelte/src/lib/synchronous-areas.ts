import type { PlannedHvdcLink, SynchronousArea } from '@tellegen/engine/synth';

export interface SynchronousAreaView {
	areas: SynchronousArea[];
	links: PlannedHvdcLink[];
	byBus: ReadonlyMap<number, SynchronousArea>;
}

/** Read only complete planning records; never guess an area's membership. */
export function parseSynchronousAreas(
	value: unknown
): SynchronousAreaView | null {
	if (!value || typeof value !== 'object') return null;
	const data = value as Record<string, unknown>;
	if (!Array.isArray(data.areas) || !Array.isArray(data.links)) return null;
	const areas: SynchronousArea[] = [];
	const byBus = new Map<number, SynchronousArea>();
	const ids = new Set<number>();
	const id = (v: unknown): v is number =>
		typeof v === 'number' && Number.isSafeInteger(v) && v >= 0;
	const finite = (v: unknown): v is number =>
		typeof v === 'number' && Number.isFinite(v);
	for (const area of data.areas) {
		if (
			!area ||
			!id(area.id) ||
			ids.has(area.id) ||
			typeof area.name !== 'string' ||
			typeof area.basis !== 'string' ||
			!finite(area.demand_mw) ||
			area.demand_mw < 0 ||
			!Array.isArray(area.bus_ids) ||
			!area.bus_ids.every(id) ||
			(area.source_url != null && typeof area.source_url !== 'string')
		)
			return null;
		ids.add(area.id);
		const copy: SynchronousArea = {
			id: area.id,
			name: area.name,
			basis: area.basis,
			demand_mw: area.demand_mw,
			bus_ids: [...area.bus_ids],
			source_url: area.source_url
		};
		for (const bus of copy.bus_ids) {
			if (byBus.has(bus)) return null;
			byBus.set(bus, copy);
		}
		areas.push(copy);
	}
	const links: PlannedHvdcLink[] = [];
	for (const link of data.links) {
		if (
			!link ||
			!id(link.from_bus) ||
			!id(link.to_bus) ||
			!id(link.from_area) ||
			!id(link.to_area) ||
			link.from_area === link.to_area ||
			byBus.get(link.from_bus)?.id !== link.from_area ||
			byBus.get(link.to_bus)?.id !== link.to_area ||
			!finite(link.rating_mw) ||
			link.rating_mw <= 0 ||
			!finite(link.transfer_mw) ||
			Math.abs(link.transfer_mw) > link.rating_mw
		)
			return null;
		links.push({
			from_bus: link.from_bus,
			to_bus: link.to_bus,
			from_area: link.from_area,
			to_area: link.to_area,
			rating_mw: link.rating_mw,
			transfer_mw: link.transfer_mw
		});
	}
	return { areas, links, byBus };
}

export function areaSourceUrl(
	value: string | null | undefined
): string | undefined {
	if (!value) return undefined;
	try {
		const url = new URL(value);
		return url.protocol === 'https:' || url.protocol === 'http:'
			? url.href
			: undefined;
	} catch {
		return undefined;
	}
}
