import type { DistGraph } from '@tellegen/engine';

/** Which buses and branches are connected to a source through closed branches. Transformers carry
 * energisation; an open switch is live when either side is. */
export interface Energisation {
	buses: Uint8Array;
	edges: Uint8Array;
	/** Buses not reached from any source. */
	deadBuses: number;
}

export function energisation(graph: DistGraph): Energisation {
	const n = graph.buses.length;
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const ends = graph.edges.map((e) => [
		index.get(e.from.toLowerCase()) ?? -1,
		index.get(e.to.toLowerCase()) ?? -1
	]);
	const adjacent: number[][] = Array.from({ length: n }, () => []);
	ends.forEach(([a, b], k) => {
		if (a < 0 || b < 0 || !graph.edges[k].closed) return;
		adjacent[a].push(b);
		adjacent[b].push(a);
	});
	const buses = new Uint8Array(n);
	const queue: number[] = [];
	graph.buses.forEach((b, i) => {
		if (b.has_source) {
			buses[i] = 1;
			queue.push(i);
		}
	});
	for (let head = 0; head < queue.length; head++)
		for (const j of adjacent[queue[head]])
			if (!buses[j]) {
				buses[j] = 1;
				queue.push(j);
			}
	const edges = Uint8Array.from(ends, ([a, b]) => (buses[a] || buses[b] ? 1 : 0));
	return { buses, edges, deadBuses: n - queue.length };
}
