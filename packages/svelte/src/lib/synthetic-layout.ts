import type { NetworkBranch, NetworkBus } from './api.js';
import { extent } from './format.js';
import type { Topology } from '@tellegen/engine';

export interface PlacedNetwork {
	buses: NetworkBus[];
	branches: NetworkBranch[];
}

export interface PlacementCenter {
	lon: number;
	lat: number;
}

type Point = { x: number; y: number };
const ALL_PAIRS_LIMIT = 500;

export function placeSyntheticTopology(
	topology: Topology,
	center: PlacementCenter,
	opts?: { roots?: number[] }
): PlacedNetwork {
	const buses = [...topology.buses].sort((a, b) => a.id - b.id);
	const index = new Map(buses.map((bus, i) => [bus.id, i]));
	const edges = topology.branches
		.filter((branch) => branch.status !== 0 && index.has(branch.from) && index.has(branch.to))
		.map((branch) => [index.get(branch.from)!, index.get(branch.to)!] as [number, number])
		.filter(([from, to]) => from !== to);
	const roots = new Set(
		(opts?.roots ?? []).flatMap((id) => {
			const i = index.get(id);
			return i === undefined ? [] : [i];
		})
	);
	const unit = forceLayout(buses.length, edges, roots);
	const span = Math.min(4.5, Math.max(0.32, Math.sqrt(Math.max(buses.length, 1)) * 0.08));
	const lonScale = Math.max(Math.cos((center.lat * Math.PI) / 180), 0.25);
	const latSpan = span;
	const lonSpan = span / lonScale;
	const coords = new Map<number, [number, number]>();

	for (let i = 0; i < buses.length; i++) {
		const p = unit[i] ?? { x: 0.5, y: 0.5 };
		coords.set(buses[i].id, [
			center.lon + (p.x - 0.5) * lonSpan,
			center.lat + (p.y - 0.5) * latSpan
		]);
	}

	return {
		buses: buses.map((bus) => {
			const [lon, lat] = coords.get(bus.id)!;
			return { id: bus.id, uid: bus.uid, lon, lat, demand_mw: bus.demand_mw, gen_mw: bus.gen_mw };
		}),
		branches: topology.branches
			.filter((branch) => coords.has(branch.from) && coords.has(branch.to))
			.map((branch) => ({
				id: branch.id,
				uid: branch.uid,
				from: branch.from,
				to: branch.to,
				rate_mw: branch.rate_mw,
				status: branch.status,
				path: [coords.get(branch.from)!, coords.get(branch.to)!]
			}))
	};
}

function forceLayout(n: number, edges: [number, number][], roots?: Set<number>): Point[] {
	if (n === 0) return [];
	if (n === 1) return [{ x: 0.5, y: 0.5 }];

	// Distribution feeders are trees (or nearly so once open ties drop out, and
	// those are filtered by status before this point). A force pass turns a
	// deep radial feeder into an illegible tangle; a tidy tree reads like the
	// one-line diagram an engineer expects. Tolerate a couple of closed loops:
	// their chord edges simply draw between the placed endpoints.
	const adjacency = graph(n, edges);
	const components = connectedComponents(adjacency);
	let degreeSum = 0;
	for (const neighbors of adjacency) degreeSum += neighbors.length;
	const cycles = degreeSum / 2 - (n - components.length);
	if (cycles <= Math.max(2, Math.ceil(0.02 * n))) {
		return treeLayout(n, adjacency, components, roots ?? new Set());
	}

	if (n > ALL_PAIRS_LIMIT) return fastGraphLayout(n, adjacency, components);

	const golden = Math.PI * (3 - Math.sqrt(5));
	const pos = Array.from({ length: n }, (_, i) => {
		const r = 0.44 * Math.sqrt((i + 0.5) / n);
		const theta = i * golden;
		return {
			x: 0.5 + r * Math.cos(theta) + 1e-4 * Math.sin(0.7 * (i + 1)),
			y: 0.5 + r * Math.sin(theta) + 1e-4 * Math.cos(1.3 * (i + 1))
		};
	});
	const disp = Array.from({ length: n }, () => ({ x: 0, y: 0 }));
	const k2 = 1 / n;
	const k = Math.sqrt(k2);
	const iters = n <= 120 ? 180 : n <= 500 ? 100 : 32;

	for (let iter = 0; iter < iters; iter++) {
		for (const d of disp) {
			d.x = 0;
			d.y = 0;
		}
		for (let i = 0; i < n; i++) {
			for (let j = i + 1; j < n; j++) {
				const dx = pos[i].x - pos[j].x;
				const dy = pos[i].y - pos[j].y;
				const f = k2 / (dx * dx + dy * dy + 1e-6);
				disp[i].x += dx * f;
				disp[i].y += dy * f;
				disp[j].x -= dx * f;
				disp[j].y -= dy * f;
			}
		}
		for (const [i, j] of edges) {
			const dx = pos[i].x - pos[j].x;
			const dy = pos[i].y - pos[j].y;
			const f = Math.sqrt(dx * dx + dy * dy) / k;
			disp[i].x -= dx * f;
			disp[i].y -= dy * f;
			disp[j].x += dx * f;
			disp[j].y += dy * f;
		}
		const t = 0.1 * (1 - iter / iters) + 1e-3;
		for (let i = 0; i < n; i++) {
			const d = Math.hypot(disp[i].x, disp[i].y) + 1e-9;
			const s = Math.min(d, t) / d;
			pos[i].x = clamp(pos[i].x + disp[i].x * s);
			pos[i].y = clamp(pos[i].y + disp[i].y * s);
		}
	}
	return normalize(pos);
}

function fastGraphLayout(n: number, adjacency: number[][], components: number[][]): Point[] {
	const cols = Math.ceil(Math.sqrt(components.length));
	const rows = Math.ceil(components.length / cols);
	const pos = Array.from({ length: n }, () => ({ x: 0.5, y: 0.5 }));
	const cellScale = 0.82 / Math.max(cols, rows);

	for (let c = 0; c < components.length; c++) {
		const nodes = components[c];
		const local = componentLayout(nodes, adjacency);
		const col = c % cols;
		const row = Math.floor(c / cols);
		const cx = (col + 0.5) / cols;
		const cy = (row + 0.5) / rows;
		for (const node of nodes) {
			const p = local.get(node) ?? { x: 0.5, y: 0.5 };
			pos[node] = {
				x: cx + (p.x - 0.5) * cellScale,
				y: cy + (p.y - 0.5) * cellScale
			};
		}
	}
	return normalize(pos);
}

/** Tidy tree layout for radial (or near-radial) graphs: depth grows along x
 * from the root, and each subtree occupies its own contiguous band of leaf
 * slots along y, so the drawing is planar by construction. The root is a
 * hinted source bus when one exists, else a BFS diameter endpoint, and a
 * node's slot equals the start of its slot interval, so the deepest chain of
 * first children renders as a straight trunk. Deterministic: every ordering
 * below is total, and there is no RNG. */
function treeLayout(
	n: number,
	adjacency: number[][],
	components: number[][],
	roots: Set<number>
): Point[] {
	const pos: Point[] = Array.from({ length: n }, () => ({ x: 0, y: 0 }));
	let slotCursor = 0;
	for (const comp of components) {
		const hinted = comp.filter((node) => roots.has(node));
		const root = hinted.length > 0 ? Math.min(...hinted) : diameterEndpoint(comp[0], adjacency);

		// BFS spanning tree from the root; chords of a near-tree are simply not
		// tree edges and draw between wherever their endpoints land.
		const parent = new Map<number, number>();
		const depth = new Map<number, number>();
		const order: number[] = [root];
		parent.set(root, -1);
		depth.set(root, 0);
		for (let head = 0; head < order.length; head++) {
			const node = order[head];
			for (const next of adjacency[node]) {
				if (depth.has(next)) continue;
				parent.set(next, node);
				depth.set(next, depth.get(node)! + 1);
				order.push(next);
			}
		}
		const children = new Map<number, number[]>(order.map((node) => [node, []]));
		for (const node of order) {
			const p = parent.get(node)!;
			if (p >= 0) children.get(p)!.push(node);
		}

		// Subtree metrics, children before parents via the reversed BFS order.
		const leaves = new Map<number, number>();
		const height = new Map<number, number>();
		for (let i = order.length - 1; i >= 0; i--) {
			const node = order[i];
			const kids = children.get(node)!;
			let leafCount = 0;
			let maxHeight = -1;
			for (const kid of kids) {
				leafCount += leaves.get(kid)!;
				maxHeight = Math.max(maxHeight, height.get(kid)!);
			}
			leaves.set(node, kids.length === 0 ? 1 : leafCount);
			height.set(node, maxHeight + 1);
		}

		// Tallest subtree first, so the trunk continues straight and laterals
		// hang off it in size order.
		for (const node of order) {
			children
				.get(node)!
				.sort(
					(a, b) =>
						height.get(b)! - height.get(a)! || leaves.get(b)! - leaves.get(a)! || a - b
				);
		}

		// Pre-order slot intervals: a node sits at the start of its interval,
		// which its first child inherits, so trunk chains share one slot.
		const stack: [number, number][] = [[root, slotCursor]];
		while (stack.length > 0) {
			const [node, lo] = stack.pop()!;
			pos[node] = { x: depth.get(node)!, y: lo + 0.5 };
			const kids = children.get(node)!;
			let start = lo;
			const starts = kids.map((kid) => {
				const s = start;
				start += leaves.get(kid)!;
				return s;
			});
			for (let i = kids.length - 1; i >= 0; i--) stack.push([kids[i], starts[i]]);
		}
		slotCursor += leaves.get(root)! + 1;
	}
	return normalize(pos);
}

/** The far endpoint of a BFS from `start`: the deepest node, smallest id on
 * ties. One end of (an approximation of) the graph diameter, the natural trunk
 * tip when no source bus is hinted. */
function diameterEndpoint(start: number, adjacency: number[][]): number {
	const depth = new Map<number, number>([[start, 0]]);
	const queue = [start];
	let best = start;
	for (let head = 0; head < queue.length; head++) {
		const node = queue[head];
		const d = depth.get(node)!;
		if (d > depth.get(best)! || (d === depth.get(best)! && node < best)) best = node;
		for (const next of adjacency[node]) {
			if (depth.has(next)) continue;
			depth.set(next, d + 1);
			queue.push(next);
		}
	}
	return best;
}

function graph(n: number, edges: [number, number][]) {
	const adjacency = Array.from({ length: n }, () => [] as number[]);
	const seen = new Set<number>();
	for (const [from, to] of edges) {
		if (from < 0 || to < 0 || from >= n || to >= n || from === to) continue;
		const a = Math.min(from, to);
		const b = Math.max(from, to);
		const key = a * n + b;
		if (seen.has(key)) continue;
		seen.add(key);
		adjacency[a].push(b);
		adjacency[b].push(a);
	}
	for (const neighbors of adjacency) neighbors.sort((a, b) => a - b);
	return adjacency;
}

function connectedComponents(adjacency: number[][]): number[][] {
	const seen = new Uint8Array(adjacency.length);
	const components: number[][] = [];
	for (let start = 0; start < adjacency.length; start++) {
		if (seen[start]) continue;
		const nodes: number[] = [];
		const stack = [start];
		seen[start] = 1;
		while (stack.length > 0) {
			const node = stack.pop()!;
			nodes.push(node);
			for (const next of adjacency[node]) {
				if (seen[next]) continue;
				seen[next] = 1;
				stack.push(next);
			}
		}
		nodes.sort((a, b) => a - b);
		components.push(nodes);
	}
	components.sort((a, b) => b.length - a.length || a[0] - b[0]);
	return components;
}

function componentLayout(nodes: number[], adjacency: number[][]): Map<number, Point> {
	if (nodes.length === 1) return new Map([[nodes[0], { x: 0.5, y: 0.5 }]]);

	const nodeToLocal = new Map(nodes.map((node, i) => [node, i]));
	const seed = nodes.reduce((best, node) =>
		adjacency[node].length > adjacency[best].length ||
		(adjacency[node].length === adjacency[best].length && node < best)
			? node
			: best
	);
	const order = bfsOrder(seed, adjacency);
	const golden = Math.PI * (3 - Math.sqrt(5));
	const pos = Array.from({ length: nodes.length }, () => ({ x: 0.5, y: 0.5 }));
	const initial = Array.from({ length: nodes.length }, () => ({ x: 0.5, y: 0.5 }));

	for (let rank = 0; rank < order.length; rank++) {
		const local = nodeToLocal.get(order[rank]);
		if (local === undefined) continue;
		const r = 0.47 * Math.sqrt((rank + 0.5) / nodes.length);
		const theta = rank * golden;
		const p = {
			x: 0.5 + r * Math.cos(theta),
			y: 0.5 + r * Math.sin(theta)
		};
		pos[local] = { ...p };
		initial[local] = p;
	}

	const localEdges: [number, number][] = [];
	for (const node of nodes) {
		const a = nodeToLocal.get(node)!;
		for (const next of adjacency[node]) {
			if (next <= node) continue;
			const b = nodeToLocal.get(next);
			if (b !== undefined) localEdges.push([a, b]);
		}
	}
	relaxEdges(pos, initial, localEdges);

	const out = new Map<number, Point>();
	const normalized = normalize(pos);
	for (const [i, node] of nodes.entries()) out.set(node, normalized[i]);
	return out;
}

function bfsOrder(seed: number, adjacency: number[][]): number[] {
	const seen = new Uint8Array(adjacency.length);
	const order: number[] = [];
	const queue = [seed];
	seen[seed] = 1;
	for (let head = 0; head < queue.length; head++) {
		const node = queue[head];
		order.push(node);
		for (const next of adjacency[node]) {
			if (seen[next]) continue;
			seen[next] = 1;
			queue.push(next);
		}
	}
	return order;
}

function relaxEdges(pos: Point[], initial: Point[], edges: [number, number][]) {
	const n = pos.length;
	const disp = Array.from({ length: n }, () => ({ x: 0, y: 0 }));
	const rest = Math.max(0.012, Math.min(0.06, 1.4 / Math.sqrt(n)));
	const iters = n <= 2000 ? 72 : n <= 10000 ? 42 : 24;
	for (let iter = 0; iter < iters; iter++) {
		for (const d of disp) {
			d.x = 0;
			d.y = 0;
		}
		for (const [i, j] of edges) {
			const dx = pos[j].x - pos[i].x;
			const dy = pos[j].y - pos[i].y;
			const dist = Math.hypot(dx, dy) + 1e-9;
			const f = (dist - rest) * 0.045;
			const fx = (dx / dist) * f;
			const fy = (dy / dist) * f;
			disp[i].x += fx;
			disp[i].y += fy;
			disp[j].x -= fx;
			disp[j].y -= fy;
		}
		for (let i = 0; i < n; i++) {
			disp[i].x += (initial[i].x - pos[i].x) * 0.012;
			disp[i].y += (initial[i].y - pos[i].y) * 0.012;
		}
		const t = 0.055 * (1 - iter / iters) + 0.002;
		for (let i = 0; i < n; i++) {
			const d = Math.hypot(disp[i].x, disp[i].y) + 1e-9;
			const s = Math.min(d, t) / d;
			pos[i].x = clamp(pos[i].x + disp[i].x * s);
			pos[i].y = clamp(pos[i].y + disp[i].y * s);
		}
	}
}

/** Fit into the unit box with one uniform scale, centered, so the layout keeps
 * its aspect ratio: a long feeder stays long instead of being stretched to a
 * square. */
function normalize(pos: Point[]): Point[] {
	const { min: minX, max: maxX } = extent(pos.map((p) => p.x));
	const { min: minY, max: maxY } = extent(pos.map((p) => p.y));
	const s = 0.92 / Math.max(maxX - minX || 1, maxY - minY || 1);
	const cx = (minX + maxX) / 2;
	const cy = (minY + maxY) / 2;
	return pos.map((p) => ({
		x: 0.5 + (p.x - cx) * s,
		y: 0.5 + (p.y - cy) * s
	}));
}

function clamp(v: number) {
	return Math.max(0, Math.min(1, v));
}
