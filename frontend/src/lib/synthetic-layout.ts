import type { NetworkBranch, NetworkBus } from './api';
import type { Topology } from './wasm';

export interface PlacedNetwork {
	buses: NetworkBus[];
	branches: NetworkBranch[];
}

export interface PlacementCenter {
	lon: number;
	lat: number;
}

type Point = { x: number; y: number };

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

function normalize(pos: Point[]): Point[] {
	const xs = pos.map((p) => p.x);
	const ys = pos.map((p) => p.y);
	const [minX, maxX] = [Math.min(...xs), Math.max(...xs)];
	const [minY, maxY] = [Math.min(...ys), Math.max(...ys)];
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
