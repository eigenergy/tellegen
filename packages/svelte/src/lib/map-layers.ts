/** Which of its network layers `TellegenMap` draws. */

/** `TellegenMap`'s `layers` prop. A layer left out is shown. */
export interface MapLayers {
	/** Lines and transformers. */
	branches?: boolean;
	/** Bus markers, less the buses a generated grid records as substations. */
	buses?: boolean;
	/** Substation markers: a generated grid's substation buses and the
	 * substation points of a display file. */
	substations?: boolean;
}

export type ResolvedMapLayers = Readonly<Required<MapLayers>>;

/** Every network layer shown. */
export const ALL_MAP_LAYERS: ResolvedMapLayers = Object.freeze({
	branches: true,
	buses: true,
	substations: true
});

/** `layers` with every missing entry shown. */
export function resolveMapLayers(layers: MapLayers | undefined): ResolvedMapLayers {
	return {
		branches: layers?.branches ?? true,
		buses: layers?.buses ?? true,
		substations: layers?.substations ?? true
	};
}

const NONE: never[] = [];

/** The buses a marker layer draws under `layers`, given each bus's kind as a
 * generated grid records it. When the layers do not split the markers (both
 * shown, or both hidden and the layer invisible) this is `buses` itself, so
 * deck.gl keeps the layer's attributes. Without kinds no bus is a
 * substation, so the buses follow `layers.buses` alone. */
export function busesForLayers<T extends { id: number }>(
	buses: T[],
	kinds: ReadonlyMap<number, string> | undefined,
	layers: Pick<ResolvedMapLayers, 'buses' | 'substations'>
): T[] {
	if (layers.buses === layers.substations) return buses;
	if (!kinds || kinds.size === 0) return layers.buses ? buses : NONE;
	const substations = layers.substations;
	return buses.filter((bus) => (kinds.get(bus.id) === 'substation') === substations);
}
