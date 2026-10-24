import type { DistGraph } from '@tellegen/engine';
import type { DisplayTree } from './display-tree.js';
import type { GalvanicZones } from './galvanic-zones.js';

/** A feeder: everything a network supplies through one branch leaving its supply bus. On MV that
 * is a feeder leaving the substation bus; on LV a circuit leaving the transformer's LV bus. Read
 * from the display tree, so through a closed loop it is the drawn feeder, not a unique one. */
export interface Feeder {
	/** The galvanic zone (network) it belongs to. */
	zone: number;
	/** The branch leaving the supply bus. */
	head: number;
	/** The supply bus: a source bus, or the bus a transformer feeds the network at. */
	supply: number;
	/** Its number among the feeders leaving the same supply bus, from 1, in graph order. */
	number: number;
	buses: number[];
	edges: number[];
}

export interface Feeders {
	feeders: Feeder[];
	/** Per bus: its feeder, or −1 for a supply bus (it belongs to its network, not one feeder). */
	busFeeder: Int32Array;
	/** Per edge: its feeder. A transformer is equipment on the feeder upstream of it; −1 where none. */
	edgeFeeder: Int32Array;
}

/** Feeders of every network. A bus starts a feeder when its tree parent is a supply bus (a root, or
 * entered through a transformer) and the branch between them is not a transformer. */
export function feeders(graph: DistGraph, tree: DisplayTree, zones: GalvanicZones): Feeders {
	const n = graph.buses.length;
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const busFeeder = new Int32Array(n).fill(-1);
	const edgeFeeder = new Int32Array(graph.edges.length).fill(-1);
	const list: Feeder[] = [];
	const isSupply = (i: number) => {
		const k = tree.parentEdge[i];
		return k < 0 || graph.edges[k].kind === 'transformer';
	};
	// Parents before children: buses in order of depth.
	const order = Array.from({ length: n }, (_, i) => i).sort(
		(a, b) => tree.depth[a] - tree.depth[b]
	);
	const perSupply = new Map<number, number>();
	for (const i of order) {
		if (isSupply(i)) continue;
		const p = tree.parent[i];
		if (isSupply(p)) {
			const number = (perSupply.get(p) ?? 0) + 1;
			perSupply.set(p, number);
			busFeeder[i] = list.length;
			list.push({
				zone: zones.busZone[i],
				head: tree.parentEdge[i],
				supply: p,
				number,
				buses: [],
				edges: []
			});
		} else busFeeder[i] = busFeeder[p];
		list[busFeeder[i]].buses.push(i);
	}
	graph.edges.forEach((e, k) => {
		const a = index.get(e.from.toLowerCase()) ?? -1;
		const b = index.get(e.to.toLowerCase()) ?? -1;
		if (a < 0 || b < 0) return;
		const [up, down] = tree.depth[a] <= tree.depth[b] ? [a, b] : [b, a];
		// A transformer sits on the feeder that reaches it; a branch belongs where its far end does.
		const f = e.kind === 'transformer' ? busFeeder[up] : busFeeder[down];
		edgeFeeder[k] = f;
		if (f >= 0) list[f].edges.push(k);
	});
	return { feeders: list, busFeeder, edgeFeeder };
}

/** Edges on the drawn path from bus `i` back to its source, across transformers. */
export function pathToSource(tree: DisplayTree, i: number): number[] {
	const out: number[] = [];
	for (let at = i; at >= 0 && tree.parentEdge[at] >= 0; at = tree.parent[at])
		out.push(tree.parentEdge[at]);
	return out;
}
