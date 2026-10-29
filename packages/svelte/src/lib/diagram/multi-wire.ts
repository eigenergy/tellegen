import type { DistGraph } from '@tellegen/engine';
import type { ResultSnapshot } from './snapshot.js';

/** One bus drawn as a stack of terminal rails. */
export interface WireBus {
	id: string;
	x: number;
	terminals: { name: string; y: number; grounded: boolean; attachments: string[] }[];
}

/** One conductor segment between two points; `terminal` names the end it belongs to. */
export interface Wire {
	x1: number;
	y1: number;
	x2: number;
	y2: number;
	/** Terminal at the bus end of the segment, used for phase colour. */
	terminal: string;
	amps: number | null;
}

/** A bounded multi-wire drawing of one branch and its two buses, in drawing units. */
export interface MultiWireView {
	width: number;
	height: number;
	buses: [WireBus, WireBus];
	wires: Wire[];
	/** Winding box for transformers: conductors meet it from each side and do not pass through. */
	winding: { x: number; y: number; width: number; height: number } | null;
	kind: string;
	id: string;
	closed: boolean;
}

const ROW = 22;
const TOP = 26;

/** Conductor-level view of branch `k`: every conductor pair from the from-terminal rail to the
 * to-terminal rail, so crossed terminal maps show as crossing wires. A transformer is drawn as a
 * winding box with no conductor continuity across it. Currents come from the snapshot's ends. */
export function multiWireBranch(
	graph: DistGraph,
	k: number,
	snapshot: ResultSnapshot | null,
	width = 320
): MultiWireView | null {
	const edge = graph.edges[k];
	const index = new Map(graph.buses.map((b, i) => [b.id.toLowerCase(), i]));
	const from = graph.buses[index.get(edge?.from.toLowerCase() ?? '') ?? -1];
	const to = graph.buses[index.get(edge?.to.toLowerCase() ?? '') ?? -1];
	if (!edge || !from || !to) return null;
	const rails = (bus: typeof from, x: number): WireBus => ({
		id: bus.id,
		x,
		terminals: bus.terminals.map((name, i) => ({
			name,
			y: TOP + i * ROW,
			grounded: bus.grounded.includes(name),
			attachments: (bus.terminal_attachments?.[name] ?? []).map((a) => `${a.kind} ${a.id}`)
		}))
	});
	const left = rails(from, 56);
	const right = rails(to, width - 56);
	const rowOf = (bus: WireBus, name: string) => bus.terminals.find((t) => t.name === name)?.y;
	const ends = snapshot?.branches[k]?.ends ?? [null, null];
	const amps = (side: 0 | 1, terminal: string) =>
		ends[side]?.currents.find((c) => c.terminal === terminal)?.amps ?? null;
	const rowsUsed = Math.max(left.terminals.length, right.terminals.length, 1);
	const height = TOP + rowsUsed * ROW + 10;
	const transformer = edge.kind === 'transformer';
	const winding = transformer
		? { x: width / 2 - 22, y: TOP - 10, width: 44, height: (rowsUsed - 1) * ROW + 20 }
		: null;
	const wires: Wire[] = [];
	for (const [a, b] of edge.conductors) {
		const y1 = rowOf(left, a);
		const y2 = rowOf(right, b);
		if (y1 === undefined || y2 === undefined) continue;
		if (winding) {
			wires.push({ x1: left.x, y1, x2: winding.x, y2: y1, terminal: a, amps: amps(0, a) });
			wires.push({
				x1: winding.x + winding.width,
				y1: y2,
				x2: right.x,
				y2,
				terminal: b,
				amps: amps(1, b)
			});
		} else {
			wires.push({ x1: left.x, y1, x2: right.x, y2, terminal: a, amps: amps(0, a) });
		}
	}
	return {
		width,
		height,
		buses: [left, right],
		wires,
		winding,
		kind: edge.kind,
		id: edge.id,
		closed: edge.closed
	};
}
