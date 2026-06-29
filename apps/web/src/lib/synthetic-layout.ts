import type { NetworkBranch, NetworkBus } from './api.js';
import { extent } from './format.js';
import type { Topology } from './wasm.js';

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

export function placeSyntheticTopology(topology: Topology, center: PlacementCenter): PlacedNetwork {
	const buses = [...topology.buses].sort((a, b) => a.id - b.id);
	const index = new Map(buses.map((bus, i) => [bus.id, i]));
	const edges = topology.branches
		.filter((branch) => branch.status !== 0 && index.has(branch.from) && index.has(branch.to))
		.map((branch) => [index.get(branch.from)!, index.get(branch.to)!] as [number, number])
		.filter(([from, to]) => from !== to);
	const unit = forceLayout(buses.length, edges);
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
			return { id: bus.id, lon, lat, demand_mw: bus.demand_mw, gen_mw: bus.gen_mw };
		}),
		branches: topology.branches
			.filter((branch) => coords.has(branch.from) && coords.has(branch.to))
			.map((branch) => ({
				id: branch.id,
				from: branch.from,
				to: branch.to,
				rate_mw: branch.rate_mw,
				status: branch.status,
				path: [coords.get(branch.from)!, coords.get(branch.to)!]
			}))
	};
}

function forceLayout(n: number, edges: [number, number][]): Point[] {
	if (n === 0) return [];
	if (n === 1) return [{ x: 0.5, y: 0.5 }];
	if (n > ALL_PAIRS_LIMIT) return fastGraphLayout(n, edges);

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

function fastGraphLayout(n: number, edges: [number, number][]): Point[] {
	const adjacency = graph(n, edges);
	const components = connectedComponents(adjacency);
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

function normalize(pos: Point[]): Point[] {
	const { min: minX, max: maxX } = extent(pos.map((p) => p.x));
	const { min: minY, max: maxY } = extent(pos.map((p) => p.y));
	const sx = maxX - minX || 1;
	const sy = maxY - minY || 1;
	return pos.map((p) => ({
		x: 0.04 + ((p.x - minX) / sx) * 0.92,
		y: 0.04 + ((p.y - minY) / sy) * 0.92
	}));
}

function clamp(v: number) {
	return Math.max(0, Math.min(1, v));
}
