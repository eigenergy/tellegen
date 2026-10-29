import type { DistConnection, DistGraph, DistWindingDetail } from '@tellegen/engine';
import type { Glyph } from './schematic-equipment.js';

const ring = (cx: number, cy: number, r: number, steps = 20) =>
	Array.from({ length: steps + 1 }, (_, i): [number, number] => {
		const a = (2 * Math.PI * i) / steps;
		return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
	});

/** How a transformer is built, as the drawing tells it: single-phase, split-phase (a centre-tapped
 * secondary: PowerIO models its halves as two windings on one bus), or polyphase. */
export type TransformerKind = 'single' | 'split' | 'poly';

export interface TransformerInfo {
	kind: TransformerKind;
	/** The windings at the edge's from and to buses (the first, for a split secondary). */
	from: DistWindingDetail | null;
	to: DistWindingDetail | null;
	/** "Dyn · 500 kVA", "1φ split · 25 kVA": vector group without its clock number, and rating. */
	label: string;
}

const sameBus = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function formatVa(va: number): string {
	if (!(va > 0)) return '';
	return va >= 1e6
		? `${Number((va / 1e6).toPrecision(3))} MVA`
		: `${Number((va / 1e3).toPrecision(3))} kVA`;
}

/** A winding's IEC letter: D or Y, N when its neutral is brought out; lower case for the lower
 * voltage side. */
function windingLetters(w: DistWindingDetail, phases: number, high: boolean): string {
	const letter = w.connection === 'delta' ? 'D' : w.connection === 'wye' ? 'Y' : '?';
	const neutral = w.connection === 'wye' && w.terminals.length > phases ? 'N' : '';
	const out = letter + neutral;
	return high ? out : out.toLowerCase();
}

/** What kind of transformer edge `k` belongs to, and its windings at either end. Null when the
 * edge carries no winding detail (a graph from before connections were exported). */
export function transformerInfo(graph: DistGraph, k: number): TransformerInfo | null {
	const edge = graph.edges[k];
	const windings = edge.windings;
	if (edge.kind !== 'transformer' || !windings?.length) return null;
	const phases = edge.n_phases;
	const shared = windings.some((w, i) => windings.some((v, j) => j > i && sameBus(w.bus, v.bus)));
	const kind: TransformerKind = phases >= 2 ? 'poly' : shared ? 'split' : 'single';
	const from = windings.find((w) => sameBus(w.bus, edge.from)) ?? null;
	const to =
		windings.find((w) => sameBus(w.bus, edge.to) && w !== from) ??
		windings.find((w) => sameBus(w.bus, edge.to)) ??
		null;
	const rating = formatVa(Math.max(...windings.map((w) => w.s_rating)));
	let group: string;
	if (kind === 'poly') {
		const sorted = [...new Set([from, to].filter((w): w is DistWindingDetail => !!w))].sort(
			(a, b) => b.v_ref - a.v_ref
		);
		group = sorted.map((w, i) => windingLetters(w, phases, i === 0)).join('');
	} else group = kind === 'split' ? '1φ split' : '1φ';
	return { kind, from, to, label: rating ? `${group} · ${rating}` : group };
}

/** A winding connection mark centred on the origin, `r` across: Δ, Y, or Y with a neutral tail. */
export function connectionMark(w: DistWindingDetail, phases: number, r: number): Glyph {
	const h = 0.866 * r;
	if (w.connection === 'delta')
		return [
			[
				[0, -r],
				[h, r / 2],
				[-h, r / 2],
				[0, -r]
			]
		];
	if (w.connection !== 'wye') return [];
	const y: Glyph = [
		[
			[0, 0],
			[0, -r]
		],
		[
			[0, 0],
			[h, r / 2]
		],
		[
			[0, 0],
			[-h, r / 2]
		]
	];
	if (w.terminals.length > phases)
		y.push([
			[0, 0],
			[0, r * 1.15]
		]);
	return y;
}

/** The transformer symbol: two overlapping winding circles on the line. A polyphase unit carries
 * each winding's connection inside its circle; a split-phase unit a tap brought out below its
 * secondary; a single-phase unit plain circles. The from winding is drawn at −x. */
export function transformerGlyph(
	info: TransformerInfo | null,
	phases: number,
	leads = true
): Glyph {
	const circles: Glyph = [ring(-0.45, 0, 0.7), ring(0.45, 0, 0.7)];
	const out: Glyph = leads
		? [
				[
					[-1.4, 0],
					[-1.15, 0]
				],
				...circles,
				[
					[1.15, 0],
					[1.4, 0]
				]
			]
		: circles;
	if (!info) return out;
	const place = (glyph: Glyph, dx: number) =>
		glyph.map((line) => line.map(([x, y]): [number, number] => [x + dx, y]));
	if (info.kind === 'poly') {
		if (info.from) out.push(...place(connectionMark(info.from, phases, 0.3), -0.68));
		if (info.to) out.push(...place(connectionMark(info.to, phases, 0.3), 0.68));
	} else if (info.kind === 'split')
		out.push(
			[
				[0.45, 0.7],
				[0.45, 1.05]
			],
			ring(0.45, 1.13, 0.08, 8)
		);
	return out;
}

/** Terminals as a compact label: "b–n" across two terminals, else the names run together when all
 * are single characters ("123n") and dotted otherwise; a wye or delta connection leads ("Y 123n"). */
export function terminalLabel(terminals: string[], connection?: DistConnection): string {
	if (!terminals.length) return '';
	if (terminals.length === 2 && connection !== 'wye' && connection !== 'delta')
		return `${terminals[0]}–${terminals[1]}`;
	const names = terminals.every((t) => t.length === 1) ? terminals.join('') : terminals.join('·');
	const lead = connection === 'wye' ? 'Y ' : connection === 'delta' ? 'Δ ' : '';
	return lead + names;
}

/** Phase conductors and whether the neutral is among `terminals`, at a bus with `neutral`. */
export function terminalConductors(
	terminals: string[],
	neutral: string | null | undefined
): { phases: number; neutral: boolean } {
	const hasNeutral = !!neutral && terminals.includes(neutral);
	return { phases: terminals.length - (hasNeutral ? 1 : 0), neutral: hasNeutral };
}
