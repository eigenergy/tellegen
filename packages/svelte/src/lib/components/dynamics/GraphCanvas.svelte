<script lang="ts">
	/** The block-diagram canvas of a controller graph: a block palette (drag
	 * blocks onto the canvas), the canvas (move blocks, click an output port
	 * and then an input port to wire them) and a form for the selected block.
	 * Shows each block's output value when `values` are given (live run or
	 * playback). The engine checks the graph as it changes. Used inline in the
	 * unit popup on the map and diagram, and full screen. */
	import { checkGraph, GRAPH_PORTS, type BlockGraph, type GraphBlock, type GraphNode } from '@tellegen/engine';

	let {
		graph = $bindable(),
		check = $bindable(null),
		values = null,
		compact = false
	}: {
		graph: BlockGraph;
		check?: { ok: true; states: string[] } | { ok: false; error: string } | null;
		/** Output of each block by node id at the shown instant. */
		values?: Record<string, number> | null;
		compact?: boolean;
	} = $props();

	type BlockType = GraphBlock['type'];
	const PALETTE: { type: BlockType; label: string; short: string; group: string }[] = [
		{ type: 'input', label: 'Input port', short: 'in', group: 'signals' },
		{ type: 'const', label: 'Constant', short: 'c', group: 'signals' },
		{ type: 'gain', label: 'Gain K', short: 'K', group: 'math' },
		{ type: 'sum', label: 'Sum ±', short: 'Σ', group: 'math' },
		{ type: 'product', label: 'Product', short: '×', group: 'math' },
		{ type: 'divide', label: 'Divide', short: '÷', group: 'math' },
		{ type: 'min', label: 'Min', short: 'min', group: 'math' },
		{ type: 'max', label: 'Max', short: 'max', group: 'math' },
		{ type: 'clamp', label: 'Limiter', short: 'lim', group: 'limits' },
		{ type: 'clamp_var', label: 'Limiter (signals)', short: 'lim~', group: 'limits' },
		{ type: 'gate', label: 'Voltage gate', short: 'gate', group: 'limits' },
		{ type: 'rectifier', label: 'Rectifier FEX', short: 'FEX', group: 'limits' },
		{ type: 'sat_quadratic', label: 'Saturation Se', short: 'Se', group: 'limits' },
		{ type: 'lag', label: 'Lag K/(1+sT)', short: 'lag', group: 'dynamics' },
		{ type: 'lag_limited', label: 'Lag, non-windup', short: 'lag⊓', group: 'dynamics' },
		{ type: 'lead_lag', label: 'Lead-lag', short: 'LL', group: 'dynamics' },
		{ type: 'washout', label: 'Washout sK/(1+sT)', short: 'wo', group: 'dynamics' },
		{ type: 'integrator', label: 'Integrator K/s', short: '∫', group: 'dynamics' },
		{ type: 'pi_limited', label: 'PI, non-windup', short: 'PI', group: 'dynamics' },
		{ type: 'lag2', label: '2nd-order lag', short: 'lag²', group: 'dynamics' },
		{ type: 'lead_lag2', label: '2nd-order lead-lag', short: 'LL²', group: 'dynamics' }
	];
	const GROUPS = ['signals', 'math', 'limits', 'dynamics'];

	function defaults(type: BlockType): GraphBlock {
		switch (type) {
			case 'input':
				return { type, port: GRAPH_PORTS[graph.slot][0] };
			case 'const':
				return { type, value: 0 };
			case 'gain':
				return { type, k: 1 };
			case 'sum':
				return { type, signs: [1, -1] };
			case 'clamp':
				return { type, min: -1, max: 1 };
			case 'gate':
				return { type, lo: 0, hi: 0 };
			case 'sat_quadratic':
				return { type, a: 0, b: 0 };
			case 'lag':
				return { type, k: 1, t: 0.1 };
			case 'lag_limited':
				return { type, k: 1, t: 0.1, min: -1, max: 1 };
			case 'lead_lag':
				return { type, k: 1, t1: 0.1, t2: 0.2 };
			case 'washout':
				return { type, k: 1, t: 1 };
			case 'integrator':
				return { type, k: 1 };
			case 'pi_limited':
				return { type, kp: 1, ki: 1, min: -1, max: 1 };
			case 'lag2':
				return { type, k: 1, t1: 0.1, t2: 0.01 };
			case 'lead_lag2':
				return { type, t1: 0.1, t2: 0.01, t3: 0.1, t4: 0.01 };
			default:
				return { type } as GraphBlock;
		}
	}

	/** Inputs a block takes; null: two or more, set in the form. */
	function arity(n: GraphNode): number | null {
		switch (n.type) {
			case 'input':
			case 'const':
				return 0;
			case 'sum':
				return n.signs.length;
			case 'product':
			case 'min':
			case 'max':
				return null;
			case 'divide':
			case 'gate':
				return 2;
			case 'clamp_var':
				return 3;
			default:
				return 1;
		}
	}

	const PORT_NAMES: Partial<Record<BlockType, string[]>> = {
		divide: ['u', '÷'],
		gate: ['u', 'v'],
		clamp_var: ['u', 'lo', 'hi']
	};

	// ----- notation and geometry -----
	// Blocks drawn in the notation of IEEE Std 421.5: transfer
	// functions as fractions in boxes, summing points as circles with their
	// signs, gains as triangles, limits written above and below their block,
	// signals as arrows. Positions are each block's top-left corner.
	const fmt = (v: number) =>
		!Number.isFinite(v)
			? '–'
			: v === 0
				? '0'
				: Math.abs(v) >= 1e4 || Math.abs(v) < 1e-3
					? v.toExponential(1)
					: `${+v.toPrecision(3)}`;
	const sp = (t: number, s: string) => (t === 1 ? s : `${fmt(t)}${s}`);
	/** `1 + Ts` (or just `1` without a time constant). */
	const poly1 = (t: number) => (t === 0 ? '1' : `1 + ${sp(t, 's')}`);
	const poly2 = (t1: number, t2: number) =>
		[`1`, t1 !== 0 ? sp(t1, 's') : '', t2 !== 0 ? sp(t2, 's²') : ''].filter(Boolean).join(' + ');

	type Shape =
		| { kind: 'port'; label: string }
		| { kind: 'junction'; symbol: string }
		| { kind: 'gain'; label: string }
		| { kind: 'fraction'; num: string; den: string }
		| { kind: 'text'; label: string; sat?: boolean };
	function shape(n: GraphNode): Shape {
		switch (n.type) {
			case 'input':
				return { kind: 'port', label: n.port };
			case 'const':
				return { kind: 'text', label: fmt(n.value) };
			case 'gain':
				return { kind: 'gain', label: fmt(n.k) };
			case 'sum':
				return { kind: 'junction', symbol: 'Σ' };
			case 'product':
				return { kind: 'junction', symbol: '×' };
			case 'divide':
				return { kind: 'junction', symbol: '÷' };
			case 'lag':
			case 'lag_limited':
				return n.t === 0 ? { kind: 'text', label: fmt(n.k) } : { kind: 'fraction', num: fmt(n.k), den: poly1(n.t) };
			case 'lead_lag':
				return {
					kind: 'fraction',
					num: n.k === 1 ? poly1(n.t1) : n.t1 === 0 ? fmt(n.k) : `${fmt(n.k)}(${poly1(n.t1)})`,
					den: poly1(n.t2)
				};
			case 'washout':
				return { kind: 'fraction', num: sp(n.k, 's'), den: poly1(n.t) };
			case 'integrator':
				return { kind: 'fraction', num: fmt(n.k), den: 's' };
			case 'pi_limited':
				return { kind: 'text', label: `${fmt(n.kp)} + ${fmt(n.ki)}/s` };
			case 'lag2':
				return { kind: 'fraction', num: fmt(n.k), den: poly2(n.t1, n.t2) };
			case 'lead_lag2':
				return { kind: 'fraction', num: poly2(n.t3, n.t4), den: poly2(n.t1, n.t2) };
			case 'clamp':
				return { kind: 'text', label: '', sat: true };
			case 'clamp_var':
				return { kind: 'text', label: 'LIMIT', sat: true };
			case 'min':
				return { kind: 'text', label: 'MIN' };
			case 'max':
				return { kind: 'text', label: 'MAX' };
			case 'gate':
				return { kind: 'text', label: 'V-GATE' };
			case 'rectifier':
				return { kind: 'text', label: 'FEX(Iₙ)' };
			case 'sat_quadratic':
				return { kind: 'text', label: 'Sₑ(u)·' };
			default:
				return { kind: 'text', label: (n as GraphNode).type };
		}
	}
	/** Limits written at the block (non-windup: on the block's edge). */
	function limits(n: GraphNode): [string, string] | null {
		switch (n.type) {
			case 'clamp':
			case 'lag_limited':
			case 'pi_limited':
				return [fmt(n.max), fmt(n.min)];
			default:
				return null;
		}
	}
	const textW = (t: string) => t.length * 6.4 + 18;
	type Geo = { w: number; h: number; ins: [number, number][]; out: [number, number] };
	function geo(n: GraphNode): Geo {
		const sh = shape(n);
		const k = n.inputs.length;
		if (sh.kind === 'port') {
			const w = textW(sh.label) + 10;
			return { w, h: 22, ins: [], out: [w, 11] };
		}
		if (sh.kind === 'junction') {
			const d = 30;
			const ins = n.inputs.map((_, i): [number, number] => {
				const a = ((120 + (120 * (i + 0.5)) / Math.max(1, k)) * Math.PI) / 180;
				return [15 + 15 * Math.cos(a), 15 + 15 * Math.sin(a)];
			});
			return { w: d, h: d, ins, out: [d, 15] };
		}
		if (sh.kind === 'gain') {
			const w = Math.max(40, textW(sh.label) + 12);
			return { w, h: 34, ins: [[0, 17]], out: [w, 17] };
		}
		const w =
			sh.kind === 'fraction' ? Math.max(64, textW(sh.num), textW(sh.den)) : Math.max(sh.sat ? 52 : 44, textW(sh.label));
		const h = Math.max(sh.kind === 'fraction' ? 46 : 32, 10 + k * 14);
		return { w, h, ins: n.inputs.map((_, i) => [0, (h * (i + 1)) / (k + 1)]), out: [w, h / 2] };
	}
	const geos = $derived(new Map(graph.nodes.map((n) => [n.id, geo(n)])));
	const g0: Geo = { w: 60, h: 30, ins: [], out: [60, 15] };

	// Place blocks without a position as a layered flow diagram (Sugiyama):
	// feedback edges found by depth-first search are set aside, every block
	// goes to the column of its longest forward path from the inputs, blocks
	// within a column are ordered by the rows of their predecessors, and the
	// rows follow the main path, so the signal runs left to right and
	// feedback returns underneath.
	$effect.pre(() => {
		if (graph.nodes.every((n) => n.pos)) return;
		const ids = graph.nodes.map((n) => n.id);
		const byId = new Map(graph.nodes.map((n) => [n.id, n]));
		const succ = new Map<string, string[]>(ids.map((id) => [id, []]));
		for (const n of graph.nodes) for (const s of n.inputs) if (byId.has(s)) succ.get(s)!.push(n.id);
		// Back edges: depth-first from the sources (inputs, constants), then
		// from anything left.
		const back = new Set<string>();
		const state = new Map<string, number>();
		const visit = (id: string) => {
			state.set(id, 1);
			for (const t of succ.get(id) ?? []) {
				const st = state.get(t) ?? 0;
				if (st === 1) back.add(`${id}>${t}`);
				else if (st === 0) visit(t);
			}
			state.set(id, 2);
		};
		for (const n of graph.nodes) if (n.inputs.length === 0 && !state.get(n.id)) visit(n.id);
		for (const id of ids) if (!state.get(id)) visit(id);
		// Columns: longest forward path.
		const layer = new Map<string, number>();
		const preds = (id: string) =>
			(byId.get(id)?.inputs ?? []).filter((s) => byId.has(s) && !back.has(`${s}>${id}`));
		const layerOf = (id: string, seen = new Set<string>()): number => {
			if (layer.has(id)) return layer.get(id)!;
			if (seen.has(id)) return 0;
			seen.add(id);
			const p = preds(id);
			const l = p.length ? 1 + Math.max(...p.map((q) => layerOf(q, seen))) : 0;
			layer.set(id, l);
			return l;
		};
		for (const id of ids) layerOf(id);
		// Inputs sit in the first column; everything else at least in the second.
		for (const n of graph.nodes) if (n.type !== 'input' && layer.get(n.id) === 0) layer.set(n.id, 1);
		const nl = Math.max(0, ...layer.values()) + 1;
		const cols: string[][] = Array.from({ length: nl }, () => []);
		for (const id of ids) cols[layer.get(id)!].push(id);
		// Rows: the output's chain first, then barycenter sweeps.
		const rank = new Map<string, number>();
		const chain = new Set<string>();
		let cur: string | undefined = graph.output;
		while (cur && byId.has(cur) && !chain.has(cur)) {
			chain.add(cur);
			const p = preds(cur);
			cur = p.find((q) => !byId.get(q)?.inputs.length) ? p[0] : p.sort((a, b) => (layer.get(b) ?? 0) - (layer.get(a) ?? 0))[0];
		}
		cols.forEach((c) => c.forEach((id, i) => rank.set(id, chain.has(id) ? -1 : i)));
		for (let sweep = 0; sweep < 4; sweep++) {
			for (const c of cols) {
				for (const id of c) {
					const p = preds(id);
					if (!chain.has(id) && p.length) rank.set(id, p.reduce((a, q) => a + (rank.get(q) ?? 0), 0) / p.length + 0.01 * c.indexOf(id));
				}
				c.sort((a, b) => (rank.get(a) ?? 0) - (rank.get(b) ?? 0));
				c.forEach((id, i) => rank.set(id, chain.has(id) ? -1 : i));
			}
		}
		const widths = cols.map((c) => Math.max(0, ...c.map((id) => geo(byId.get(id)!).w)));
		let x = 30;
		const xs = widths.map((w) => {
			const at = x;
			x += w + 58;
			return at;
		});
		const ROW = 78;
		cols.forEach((c, l) => {
			let y = 40;
			for (const id of c) {
				const n = byId.get(id)!;
				const gm = geo(n);
				// Rows line up with the predecessors where there is room.
				const p = preds(id).map((q) => byId.get(q)!).filter((q) => q.pos);
				const want = p.length ? p.reduce((a, q) => a + q.pos![1] + geo(q).out[1], 0) / p.length - gm.out[1] : y;
				const top = Math.max(y, Math.round(want));
				if (!n.pos) n.pos = [xs[l] + (widths[l] - gm.w) / 2, top];
				y = top + Math.max(gm.h, 26) + (ROW - 30);
			}
		});
	});
	let selected = $state<string | null>(null);
	// Zoom: `fit` scales the diagram to the canvas width (never above 1).
	let canvasWidth = $state(0);
	let fit = $state(true);
	let zoomLevel = $state(1);
	const sel = $derived(graph.nodes.find((n) => n.id === selected) ?? null);
	const OUT_NAME: Record<string, string> = { exciter: 'Efd', governor: 'τm', stabilizer: 'Vs' };
	const extent = $derived.by(() => {
		let w = compact ? 520 : 760;
		let h = compact ? 260 : 380;
		for (const n of graph.nodes) {
			const gm = geos.get(n.id) ?? g0;
			const [x, y] = n.pos ?? [0, 0];
			w = Math.max(w, x + gm.w + (graph.output === n.id ? 110 : 50));
			h = Math.max(h, y + gm.h + 70);
		}
		return { w, h: h + 40 };
	});
	const zoom = $derived(fit && canvasWidth > 0 ? Math.min(1, (canvasWidth - 8) / extent.w) : zoomLevel);
	function setZoom(z: number) {
		fit = false;
		zoomLevel = Math.max(0.3, Math.min(2, z));
	}

	function nodeAt(id: string) {
		return graph.nodes.find((n) => n.id === id);
	}
	function outPort(n: GraphNode): [number, number] {
		const [x, y] = n.pos ?? [0, 0];
		const o = (geos.get(n.id) ?? g0).out;
		return [x + o[0], y + o[1]];
	}
	function inPort(n: GraphNode, i: number): [number, number] {
		const [x, y] = n.pos ?? [0, 0];
		const p = (geos.get(n.id) ?? g0).ins[i] ?? [0, 15];
		return [x + p[0], y + p[1]];
	}
	function bottom(n: GraphNode) {
		return (n.pos?.[1] ?? 0) + (geos.get(n.id) ?? g0).h + (limits(n) ? 12 : 0);
	}
	/** Wires routed orthogonally around the blocks: A* on an 8 px grid with
	 * a cost for every bend and for cells another signal already uses
	 * (wires of the same signal share them freely, so a fan-out grows as one
	 * trunk). Each wire leaves its output to the right and enters its input
	 * from the left. */
	const GRID = 8;
	function routeAll(): { d: string; key: string; from: string; to: string }[] {
		const gw = Math.ceil(extent.w / GRID) + 2;
		const gh = Math.ceil(extent.h / GRID) + 2;
		const blocked = new Uint8Array(gw * gh);
		for (const n of graph.nodes) {
			const gm = geos.get(n.id) ?? g0;
			const [x, y] = n.pos ?? [0, 0];
			const x0 = Math.floor((x - 5) / GRID), x1 = Math.ceil((x + gm.w + 5) / GRID);
			const y0 = Math.floor((y - 5) / GRID), y1 = Math.ceil((y + gm.h + 5) / GRID);
			for (let cy = Math.max(0, y0); cy <= Math.min(gh - 1, y1); cy++)
				for (let cx = Math.max(0, x0); cx <= Math.min(gw - 1, x1); cx++) blocked[cy * gw + cx] = 1;
		}
		const owner = new Int32Array(gw * gh).fill(-1);
		const srcIndex = new Map(graph.nodes.map((n, i) => [n.id, i]));
		const edges: { key: string; from: string; to: string; i: number; a: [number, number]; b: [number, number] }[] = [];
		for (const n of graph.nodes)
			n.inputs.forEach((s, i) => {
				const src = nodeAt(s);
				if (src) edges.push({ key: `${s}>${n.id}:${i}`, from: s, to: n.id, i, a: outPort(src), b: inPort(n, i) });
			});
		edges.sort((p, q) => Math.abs(p.a[0] - p.b[0]) + Math.abs(p.a[1] - p.b[1]) - (Math.abs(q.a[0] - q.b[0]) + Math.abs(q.a[1] - q.b[1])));
		const DX = [1, 0, -1, 0];
		const DY = [0, 1, 0, -1];
		const out: { d: string; key: string; from: string; to: string }[] = [];
		for (const e of edges) {
			const me = srcIndex.get(e.from) ?? -2;
			const sx = Math.min(gw - 1, Math.round(e.a[0] / GRID) + 1), sy = Math.round(e.a[1] / GRID);
			const tx = Math.max(0, Math.round(e.b[0] / GRID) - 1), ty = Math.round(e.b[1] / GRID);
			const cells = astar(sx, sy, tx, ty);
			if (!cells) {
				const mx = (e.a[0] + e.b[0]) / 2;
				out.push({ key: e.key, from: e.from, to: e.to, d: `M${e.a[0]},${e.a[1]} H${mx} V${e.b[1]} H${e.b[0]}` });
				continue;
			}
			for (const c of cells) if (owner[c] < 0) owner[c] = me;
			// Corners of the cell path; the first and last horizontal runs sit
			// at the ports' exact heights.
			const pts: [number, number][] = [];
			for (let k = 0; k < cells.length; k++) {
				const cx = (cells[k] % gw) * GRID, cy = Math.floor(cells[k] / gw) * GRID;
				if (k > 0 && k < cells.length - 1) {
					const p = cells[k - 1], q = cells[k + 1];
					if ((p % gw === cells[k] % gw) === (q % gw === cells[k] % gw)) continue;
				}
				pts.push([cx, cy]);
			}
			const yFirst = pts[0][1];
			for (const pt of pts) if (pt[1] === yFirst) pt[1] = e.a[1];
			else break;
			const yLast = pts[pts.length - 1][1];
			for (let k = pts.length - 1; k >= 0 && pts[k][1] === yLast; k--) pts[k][1] = e.b[1];
			out.push({
				key: e.key,
				from: e.from,
				to: e.to,
				d: `M${e.a[0]},${e.a[1]} ` + pts.map(([x, y]) => `L${x},${y}`).join(' ') + ` L${e.b[0]},${e.b[1]}`
			});
			function astar(sx: number, sy: number, tx: number, ty: number): number[] | null {
				const N = gw * gh;
				const g = new Float64Array(N * 4).fill(Infinity);
				const from = new Int32Array(N * 4).fill(-1);
				const heap: [number, number][] = [];
				const push = (f: number, st: number) => {
					heap.push([f, st]);
					let i = heap.length - 1;
					while (i > 0) {
						const pi = (i - 1) >> 1;
						if (heap[pi][0] <= heap[i][0]) break;
						[heap[pi], heap[i]] = [heap[i], heap[pi]];
						i = pi;
					}
				};
				const pop = () => {
					const top = heap[0];
					const last = heap.pop()!;
					if (heap.length) {
						heap[0] = last;
						let i = 0;
						for (;;) {
							const l = 2 * i + 1, r = l + 1;
							let m = i;
							if (l < heap.length && heap[l][0] < heap[m][0]) m = l;
							if (r < heap.length && heap[r][0] < heap[m][0]) m = r;
							if (m === i) break;
							[heap[m], heap[i]] = [heap[i], heap[m]];
							i = m;
						}
					}
					return top;
				};
				const start = (sy * gw + sx) * 4; // heading east
				const goal = ty * gw + tx;
				g[start] = 0;
				push(Math.abs(tx - sx) + Math.abs(ty - sy), start);
				let pops = 0;
				while (heap.length && pops++ < 60000) {
					const [, st] = pop();
					const cell = st >> 2, dir = st & 3;
					if (cell === goal && dir === 0) {
						const path: number[] = [];
						for (let s2 = st; s2 >= 0; s2 = from[s2]) path.push(s2 >> 2);
						return path.reverse().filter((c, k, arr) => k === 0 || c !== arr[k - 1]);
					}
					const cx = cell % gw, cy = Math.floor(cell / gw);
					for (let d = 0; d < 4; d++) {
						if (d === (dir + 2) % 4) continue;
						const nx = cx + DX[d], ny = cy + DY[d];
						if (nx < 0 || ny < 0 || nx >= gw || ny >= gh) continue;
						const nc = ny * gw + nx;
						if (blocked[nc] && nc !== goal) continue;
						const shared = owner[nc] >= 0 && owner[nc] !== me;
						const cost = g[st] + 1 + (d !== dir ? 4 : 0) + (shared ? 6 : 0) + (owner[nc] === me ? -0.6 : 0);
						const ns = nc * 4 + d;
						if (cost < g[ns]) {
							g[ns] = cost;
							from[ns] = st;
							push(cost + Math.abs(tx - nx) + Math.abs(ty - ny), ns);
						}
					}
				}
				return null;
			}
		}
		return out;
	}
	const wires = $derived(routeAll());
	/** The selected block's wires and neighbours. */
	const linked = $derived.by(() => {
		const set = new Set<string>();
		if (!selected) return set;
		for (const w of wires) {
			if (w.from === selected) set.add(w.to);
			if (w.to === selected) set.add(w.from);
		}
		return set;
	});
	function signOf(n: GraphNode, i: number) {
		return n.type === 'sum' ? (n.signs[i] >= 0 ? '+' : '−') : n.type === 'divide' && i === 1 ? '÷' : '';
	}
	function summary(n: GraphNode): string {
		const sh = shape(n);
		const lim = limits(n);
		const body = sh.kind === 'fraction' ? `(${sh.num}) / (${sh.den})` : 'label' in sh ? sh.label : sh.kind === 'junction' ? sh.symbol : '';
		return `${n.id}: ${PALETTE.find((p) => p.type === n.type)?.label ?? n.type} ${body}${lim ? ` [${lim[1]}, ${lim[0]}]` : ''}`;
	}

	// ----- editing -----
	function freshId(type: string) {
		let k = 1;
		while (nodeAt(`${type}${k}`)) k++;
		return `${type}${k}`;
	}
	function addNode(type: BlockType, pos: [number, number]) {
		const n = { ...defaults(type), id: freshId(type), inputs: [], pos } as GraphNode;
		n.inputs = Array(arity(n) ?? 2).fill('');
		graph.nodes.push(n);
		selected = n.id;
	}
	function removeNode(id: string) {
		graph.nodes = graph.nodes.filter((n) => n.id !== id);
		for (const n of graph.nodes) n.inputs = n.inputs.map((s) => (s === id ? '' : s));
		if (graph.output === id) graph.output = '';
		selected = null;
	}
	function renameNode(n: GraphNode, id: string) {
		id = id.trim();
		if (!id || nodeAt(id)) return;
		const old = n.id;
		for (const m of graph.nodes) m.inputs = m.inputs.map((s) => (s === old ? id : s));
		if (graph.output === old) graph.output = id;
		n.id = id;
		selected = id;
	}
	function setInputs(n: GraphNode, count: number) {
		count = Math.max(1, Math.min(8, count));
		if (n.type === 'sum') n.signs = Array.from({ length: count }, (_, i) => n.signs[i] ?? 1);
		n.inputs = Array.from({ length: count }, (_, i) => n.inputs[i] ?? '');
	}
	function numberField(n: GraphNode, key: string): number {
		return (n as unknown as Record<string, number>)[key];
	}
	function setNumber(n: GraphNode, key: string, text: string) {
		const v = Number(text);
		if (text.trim() !== '' && Number.isFinite(v)) (n as unknown as Record<string, number>)[key] = v;
	}
	const PARAMS: Partial<Record<BlockType, string[]>> = {
		const: ['value'],
		gain: ['k'],
		integrator: ['k'],
		clamp: ['min', 'max'],
		gate: ['lo', 'hi'],
		lag: ['k', 't'],
		washout: ['k', 't'],
		lag_limited: ['k', 't', 'min', 'max'],
		lead_lag: ['k', 't1', 't2'],
		pi_limited: ['kp', 'ki', 'min', 'max'],
		lag2: ['k', 't1', 't2'],
		lead_lag2: ['t1', 't2', 't3', 't4'],
		sat_quadratic: ['a', 'b'],
		min: ['tie']
	};

	// Wiring: click an output port, then an input port.
	let pending = $state<string | null>(null);
	function clickOut(id: string, e: Event) {
		e.stopPropagation();
		pending = pending === id ? null : id;
	}
	function clickIn(n: GraphNode, i: number, e: Event) {
		e.stopPropagation();
		n.inputs[i] = pending ?? '';
		pending = null;
	}

	// Dragging blocks.
	let svg = $state<SVGSVGElement | null>(null);
	let drag: { id: string; dx: number; dy: number } | null = null;
	function point(e: { clientX: number; clientY: number }): [number, number] {
		const r = svg?.getBoundingClientRect();
		return r ? [(e.clientX - r.left) / zoom, (e.clientY - r.top) / zoom] : [0, 0];
	}
	function startDrag(n: GraphNode, e: PointerEvent) {
		const [x, y] = point(e);
		drag = { id: n.id, dx: x - (n.pos?.[0] ?? 0), dy: y - (n.pos?.[1] ?? 0) };
		selected = n.id;
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
	}
	function moveDrag(e: PointerEvent) {
		if (!drag) return;
		const n = nodeAt(drag.id);
		if (!n) return;
		const [x, y] = point(e);
		n.pos = [Math.max(0, Math.round(x - drag.dx)), Math.max(0, Math.round(y - drag.dy))];
	}
	function dropOnCanvas(e: DragEvent) {
		e.preventDefault();
		const type = e.dataTransfer?.getData('application/x-tellegen-block') as BlockType | '';
		if (!type) return;
		const [x, y] = point(e);
		addNode(type, [Math.max(0, x - 30), Math.max(0, y - 16)]);
	}

	// ----- engine check (debounced) -----
	let checkTimer: ReturnType<typeof setTimeout> | null = null;
	$effect(() => {
		const snapshot = JSON.parse(JSON.stringify(graph)) as BlockGraph;
		if (checkTimer) clearTimeout(checkTimer);
		checkTimer = setTimeout(() => {
			checkGraph(snapshot)
				.then((c) => (check = c))
				.catch((e) => (check = { ok: false, error: e instanceof Error ? e.message : String(e) }));
		}, 150);
	});

	function onkey(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
		if (e.key === 'Escape' && pending) {
			pending = null;
			e.stopPropagation();
		}
		if ((e.key === 'Delete' || e.key === 'Backspace') && selected) {
			removeNode(selected);
		}
	}
</script>

<svelte:window onkeydown={onkey} />

<div class="gc" class:compact class:bare={compact && !sel}>
	<aside class="palette">
		{#each GROUPS as group (group)}
			{#if !compact}<h4>{group}</h4>{/if}
			{#each PALETTE.filter((p) => p.group === group) as p (p.type)}
				<button
					class="block"
					draggable="true"
					ondragstart={(e) => e.dataTransfer?.setData('application/x-tellegen-block', p.type)}
					ondblclick={() => addNode(p.type, [24, extent.h - 90])}
					title={`${p.label}: drag onto the canvas (or double-click)`}>{compact ? p.short : p.label}</button
				>
			{/each}
		{/each}
	</aside>
	<div
		class="canvas"
		role="application"
		bind:clientWidth={canvasWidth}
		ondragover={(e) => e.preventDefault()}
		ondrop={dropOnCanvas}
	>
		<div class="zoom">
			<button title="zoom out" onclick={() => setZoom(zoom / 1.25)}>−</button>
			<button title="fit to width" class:on={fit} onclick={() => (fit = true)}>fit</button>
			<button title="zoom in" onclick={() => setZoom(zoom * 1.25)}>+</button>
		</div>
		<svg
			bind:this={svg}
			width={extent.w * zoom}
			height={extent.h * zoom}
			viewBox={`0 0 ${extent.w} ${extent.h}`}
			onpointermove={moveDrag}
			onpointerup={() => (drag = null)}
			onclick={() => {
				pending = null;
				selected = null;
			}}
			role="presentation"
		>
			<defs>
				<marker id="gc-arrow" viewBox="0 0 8 8" refX="7.5" refY="4" markerWidth="7" markerHeight="7" orient="auto">
					<path d="M0,0 L8,4 L0,8 z" class="arrowhead" />
				</marker>
				<marker id="gc-arrow-hl" viewBox="0 0 8 8" refX="7.5" refY="4" markerWidth="7" markerHeight="7" orient="auto">
					<path d="M0,0 L8,4 L0,8 z" class="arrowhead hl" />
				</marker>
			</defs>
			{#each wires as w (w.key)}
				<path
					class="wire"
					class:hl={!!selected && (w.from === selected || w.to === selected)}
					class:dim={!!selected && w.from !== selected && w.to !== selected}
					d={w.d}
					marker-end={!!selected && (w.from === selected || w.to === selected) ? 'url(#gc-arrow-hl)' : 'url(#gc-arrow)'}
				/>
			{/each}
			{#each graph.nodes as n (n.id)}
				{@const [x, y] = n.pos ?? [0, 0]}
				{@const gm = geos.get(n.id) ?? g0}
				{@const sh = shape(n)}
				{@const lim = limits(n)}
				{@const [ox, oy] = outPort(n)}
				{@const v = values?.[n.id]}
				<g
					class="node"
					class:selected={selected === n.id}
					class:linked={linked.has(n.id)}
					class:faded={!!selected && selected !== n.id && !linked.has(n.id)}
					class:pending={pending === n.id}
				>
					<title>{summary(n)}</title>
					{#if sh.kind !== 'port'}<text x={x + gm.w / 2} y={y - 6} class="sig">{n.id}</text>{/if}
					{#if sh.kind === 'port'}
						<path class="shape port" d={`M${x},${y} H${x + gm.w - 9} L${x + gm.w},${y + 11} L${x + gm.w - 9},${y + 22} H${x} z`} />
						<text x={x + 8} y={y + 15} class="lbl">{sh.label}</text>
					{:else if sh.kind === 'junction'}
						<circle class="shape" cx={x + 15} cy={y + 15} r="15" />
						<text x={x + 15} y={y + 20} class="sym">{sh.symbol}</text>
						{#each n.inputs as _, i (i)}
							{@const [px, py] = inPort(n, i)}
							{#if signOf(n, i)}<text x={px - 7} y={py - 4} class="sign">{signOf(n, i)}</text>{/if}
						{/each}
					{:else if sh.kind === 'gain'}
						<path class="shape" d={`M${x},${y} L${x + gm.w},${y + 17} L${x},${y + 34} z`} />
						<text x={x + gm.w * 0.36} y={y + 21} class="lbl">{sh.label}</text>
					{:else}
						<rect class="shape" {x} {y} width={gm.w} height={gm.h} rx="2" />
						{#if sh.kind === 'fraction'}
							<text x={x + gm.w / 2} y={y + gm.h / 2 - 5} class="tf">{sh.num}</text>
							<line x1={x + 8} x2={x + gm.w - 8} y1={y + gm.h / 2} y2={y + gm.h / 2} class="bar" />
							<text x={x + gm.w / 2} y={y + gm.h / 2 + 14} class="tf">{sh.den}</text>
						{:else if sh.sat}
							<path class="satsym" d={`M${x + gm.w / 2 - 14},${y + gm.h / 2 + 8} H${x + gm.w / 2 - 6} L${x + gm.w / 2 + 6},${y + gm.h / 2 - 8} H${x + gm.w / 2 + 14}`} />
							{#if sh.label}<text x={x + gm.w / 2} y={y + gm.h - 3} class="small">{sh.label}</text>{/if}
						{:else}
							<text x={x + gm.w / 2} y={y + gm.h / 2 + 4} class="tf">{sh.label}</text>
						{/if}
					{/if}
					{#if lim}
						<text x={x + gm.w - 2} y={y - 6} class="lim" text-anchor="end">{lim[0]}</text>
						<text x={x + gm.w - 2} y={y + gm.h + 11} class="lim" text-anchor="end">{lim[1]}</text>
						<line x1={x + gm.w - 14} x2={x + gm.w} y1={y} y2={y} class="limbar" />
						<line x1={x} x2={x + 14} y1={y + gm.h} y2={y + gm.h} class="limbar" />
					{/if}
					<rect
						class="hit"
						{x}
						{y}
						width={gm.w}
						height={gm.h}
						onpointerdown={(e) => startDrag(n, e)}
						onclick={(e) => e.stopPropagation()}
						onkeydown={(e) => e.key === 'Enter' && (selected = n.id)}
						role="button"
						tabindex="-1"
					/>
					{#if graph.output === n.id}
						<path class="wire" d={`M${ox},${oy} H${ox + 46}`} marker-end="url(#gc-arrow)" />
						<text x={ox + 50} y={oy + 4} class="outname">{OUT_NAME[graph.slot] ?? 'out'}</text>
					{/if}
					{#if v !== undefined}
						<text x={ox + 4} y={oy - 5} class="value">{fmt(v)}</text>
					{/if}
					{#each n.inputs as s, i (i)}
						{@const [px, py] = inPort(n, i)}
						<circle
							class="port in"
							class:open={!nodeAt(s)}
							class:target={!!pending}
							cx={px}
							cy={py}
							r="4"
							onclick={(e) => clickIn(n, i, e)}
							onkeydown={(e) => e.key === 'Enter' && clickIn(n, i, e)}
							role="button"
							tabindex="-1"
							><title>{(PORT_NAMES[n.type]?.[i] ?? `input ${i + 1}`) + (s ? ` ← ${s}` : ' (not connected)')}</title></circle
						>
					{/each}
					<circle
						class="port out"
						class:armed={pending === n.id}
						cx={ox}
						cy={oy}
						r="4"
						onclick={(e) => clickOut(n.id, e)}
						onkeydown={(e) => e.key === 'Enter' && clickOut(n.id, e)}
						role="button"
						tabindex="-1"><title>output of {n.id}: click, then an input</title></circle
					>
				</g>
			{/each}
		</svg>
	</div>
	<aside class="form" class:hidden={compact && !sel}>
		{#if sel}
			<label>id <input class="mono" value={sel.id} onchange={(e) => renameNode(sel, e.currentTarget.value)} /></label>
			<div class="dim">{PALETTE.find((p) => p.type === sel.type)?.label}</div>
			{#if sel.type === 'input'}
				<label
					>port
					<select bind:value={sel.port}>
						{#each GRAPH_PORTS[graph.slot] as p (p)}<option value={p}>{p}</option>{/each}
					</select>
				</label>
			{/if}
			{#each PARAMS[sel.type] ?? [] as key (key)}
				<label
					>{key}
					<input class="mono" value={numberField(sel, key)} oninput={(e) => setNumber(sel, key, e.currentTarget.value)} /></label
				>
			{/each}
			{#if sel.type === 'lag_limited'}
				<label class="check"
					><input type="checkbox" checked={sel.clamp_output !== false} onchange={(e) => (sel.clamp_output = e.currentTarget.checked)} />
					clamp the output</label
				>
			{/if}
			{#if sel.type === 'sum'}
				<label
					>signs
					<input
						class="mono"
						value={sel.signs.join(', ')}
						onchange={(e) => {
							const signs = e.currentTarget.value
								.split(',')
								.map((v) => Number(v.trim()))
								.filter((v) => Number.isFinite(v));
							if (signs.length) {
								sel.signs = signs;
								setInputs(sel, signs.length);
							}
						}}
					/>
				</label>
			{/if}
			{#if arity(sel) === null}
				<label
					>inputs <input
						type="number"
						min="1"
						max="8"
						value={sel.inputs.length}
						onchange={(e) => setInputs(sel, Number(e.currentTarget.value))}
					/></label
				>
			{/if}
			{#if values?.[sel.id] !== undefined}<div class="dim">output now <span class="mono">{fmt(values[sel.id])}</span></div>{/if}
			<div class="row">
				<button class="ghost" disabled={graph.output === sel.id} onclick={() => (graph.output = sel.id)}>Make output</button>
				<button class="ghost" onclick={() => removeNode(sel.id)}>Delete</button>
			</div>
		{:else}
			<p class="dim">
				Drag blocks onto the canvas. Wire them by clicking an output port (right), then an input port (left); click
				an input port to open it. A loop needs a lag or an integrator; a zero time constant makes a state algebraic.
			</p>
			{#if check?.ok}<p class="dim">States: <span class="mono">{check.states.join(', ') || '–'}</span></p>{/if}
		{/if}
	</aside>
</div>

<style>
	.gc {
		flex: 1;
		display: grid;
		grid-template-columns: 160px 1fr 220px;
		min-height: 0;
		font-size: var(--fs-control);
	}
	.gc.compact {
		grid-template-columns: 1fr 180px;
		grid-template-rows: auto 1fr;
	}
	.gc.compact.bare {
		grid-template-columns: 1fr;
	}
	.form.hidden {
		display: none;
	}
	.gc.compact .palette {
		grid-column: 1 / -1;
		flex-direction: row;
		flex-wrap: wrap;
		border-right: none;
		border-bottom: 1px solid var(--border-default);
		padding: 4px 8px;
	}
	h4 {
		margin: 8px 0 2px;
		font-size: var(--fs-control);
		color: var(--text-tertiary);
		font-weight: 500;
		text-transform: capitalize;
	}
	.mono {
		font-family: var(--font-mono);
	}
	.dim {
		color: var(--text-tertiary);
	}
	.palette,
	.form {
		overflow-y: auto;
		padding: 6px 10px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.palette {
		border-right: 1px solid var(--border-default);
	}
	.form {
		border-left: 1px solid var(--border-default);
	}
	.form label {
		display: flex;
		flex-direction: column;
		gap: 2px;
		color: var(--text-secondary);
	}
	.form label.check {
		flex-direction: row;
		align-items: center;
		gap: 6px;
	}
	.form input,
	.form select {
		font-size: var(--fs-control);
	}
	.row {
		display: flex;
		gap: 6px;
		margin-top: 6px;
	}
	button.block {
		text-align: left;
		cursor: grab;
		font-size: var(--fs-control);
	}
	.compact button.block {
		padding: 1px 6px;
		font-family: var(--font-mono);
	}
	.zoom {
		position: sticky;
		top: 6px;
		left: 6px;
		float: left;
		z-index: 1;
		display: flex;
		gap: 2px;
		margin: 6px;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: 6px;
		padding: 2px;
	}
	.zoom button {
		border: none;
		background: none;
		padding: 0 7px;
		font-size: var(--fs-control);
		cursor: pointer;
		color: var(--text-secondary);
	}
	.zoom button.on {
		color: var(--text-accent);
		font-weight: 600;
	}
	.canvas {
		position: relative;
		overflow: auto;
		min-width: 0;
		min-height: 0;
		background-image: radial-gradient(var(--border-default) 1px, transparent 1px);
		background-size: 16px 16px;
	}
	svg {
		display: block;
		user-select: none;
	}
	.wire {
		fill: none;
		stroke: var(--text-secondary);
		stroke-width: 1.3;
	}
	.wire.hl {
		stroke: var(--accent);
		stroke-width: 2.2;
	}
	.wire.dim {
		opacity: 0.25;
	}
	.arrowhead.hl {
		fill: var(--accent);
	}
	.node.linked .shape {
		stroke: var(--accent);
	}
	.node.faded {
		opacity: 0.45;
	}
	.arrowhead {
		fill: var(--text-secondary);
	}
	.node .shape {
		fill: var(--surface-menu);
		stroke: var(--text-primary);
		stroke-width: 1.2;
	}
	.node .shape.port {
		fill: var(--surface-control);
		stroke: var(--text-secondary);
	}
	.node.selected .shape {
		stroke: var(--accent);
		stroke-width: 2;
	}
	.node text {
		pointer-events: none;
		fill: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 11px;
	}
	.node text.tf,
	.node text.sym,
	.node text.small {
		text-anchor: middle;
	}
	.node text.sym {
		font-size: 15px;
		font-family: inherit;
	}
	.node text.small {
		font-size: 8px;
		fill: var(--text-tertiary);
	}
	.node text.sig {
		text-anchor: middle;
		font-size: 9px;
		fill: var(--text-tertiary);
	}
	.node text.sign {
		font-size: 11px;
		font-weight: 700;
		text-anchor: middle;
	}
	.node text.lim {
		font-size: 9px;
		fill: var(--text-secondary);
	}
	.node text.outname {
		font-weight: 600;
	}
	.node text.value {
		fill: var(--text-accent);
		font-size: 9px;
		font-weight: 600;
	}
	.bar,
	.limbar {
		stroke: var(--text-primary);
		stroke-width: 1;
	}
	.limbar {
		stroke-width: 2;
	}
	.satsym {
		fill: none;
		stroke: var(--text-primary);
		stroke-width: 1.4;
	}
	.hit {
		fill: transparent;
		cursor: move;
	}
	.port {
		fill: var(--surface-menu);
		stroke: var(--text-secondary);
		stroke-width: 1.2;
		cursor: crosshair;
		opacity: 0;
		transition: opacity 120ms;
	}
	.node:hover .port,
	.node.selected .port,
	.port.target,
	.port.armed,
	.port.open {
		opacity: 1;
	}
	.port.open {
		stroke: var(--text-danger);
	}
	.port.in.target {
		stroke: var(--accent);
	}
	.port.out.armed {
		fill: var(--accent);
		stroke: var(--accent);
	}
</style>
