import type { DistGraph } from '@tellegen/engine';

/** A breadth-first spanning forest over closed edges, rooted at source buses. It orders the drawing
 * and the voltage profile; it is not a claim about a unique electrical path through a mesh. */
export interface DisplayTree {
	/** Parent bus index; a root is its own parent. Unreached buses are their own root too. */
	parent: Int32Array;
	/** Edge index from the parent, or −1 for roots. */
	parentEdge: Int32Array;
	depth: Int32Array;
	/** Component index, one per root. */
	component: Int32Array;
	roots: number[];
}

/** Roots are source buses in graph order; any bus a source cannot reach starts its own component,
 * so every bus has a depth and nothing is silently dropped. Open switches are not traversed. */
export function displayTree(graph: DistGraph): DisplayTree {
	const n = graph.buses.length;
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const adjacency: { to: number; edge: number }[][] = Array.from({ length: n }, () => []);
	graph.edges.forEach((e, k) => {
		const a = index.get(e.from.toLowerCase());
		const b = index.get(e.to.toLowerCase());
		if (!e.closed || a === undefined || b === undefined || a === b) return;
		adjacency[a].push({ to: b, edge: k });
		adjacency[b].push({ to: a, edge: k });
	});
	const parent = new Int32Array(n).fill(-1);
	const parentEdge = new Int32Array(n).fill(-1);
	const depth = new Int32Array(n).fill(-1);
	const component = new Int32Array(n).fill(-1);
	const roots: number[] = [];
	const queue = new Int32Array(n);
	const grow = (root: number) => {
		const c = roots.length;
		roots.push(root);
		parent[root] = root;
		depth[root] = 0;
		component[root] = c;
		let head = 0;
		let tail = 0;
		queue[tail++] = root;
		while (head < tail) {
			const u = queue[head++];
			for (const { to, edge } of adjacency[u]) {
				if (depth[to] >= 0) continue;
				parent[to] = u;
				parentEdge[to] = edge;
				depth[to] = depth[u] + 1;
				component[to] = c;
				queue[tail++] = to;
			}
		}
	};
	graph.buses.forEach((b, i) => {
		if (b.has_source && depth[i] < 0) grow(i);
	});
	for (let i = 0; i < n; i++) if (depth[i] < 0) grow(i);
	return { parent, parentEdge, depth, component, roots };
}

/** Root-to-bus path as bus and edge indices, root first. */
export function treePath(tree: DisplayTree, bus: number): { buses: number[]; edges: number[] } {
	const buses = [bus];
	const edges: number[] = [];
	let u = bus;
	while (tree.parent[u] !== u) {
		edges.push(tree.parentEdge[u]);
		u = tree.parent[u];
		buses.push(u);
		if (buses.length > tree.parent.length) throw new Error('Display tree has a cycle');
	}
	return { buses: buses.reverse(), edges: edges.reverse() };
}
