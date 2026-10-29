import { describe, expect, it } from 'vitest';
import type { DistGraph, DistWindingDetail } from '@tellegen/engine';
import {
	schematicEquipment,
	sourceLayout,
	displayTree,
	terminalLabel,
	terminalConductors,
	transformerGlyph,
	transformerInfo
} from '../src/lib/diagram/index.js';

const winding = (
	bus: string,
	connection: 'wye' | 'delta',
	terminals: string[],
	v_ref: number,
	s_rating = 500e3
): DistWindingDetail => ({ bus, connection, terminals, v_ref, s_rating });

const bus = (id: string, extra: Partial<DistGraph['buses'][number]> = {}) => ({
	id,
	terminals: ['1', '2', '3', '4'],
	grounded: [],
	neutral_terminal: '4',
	load_kw: 0,
	gen_kw: 0,
	has_source: false,
	...extra
});

const transformer = (
	id: string,
	from: string,
	to: string,
	n_phases: number,
	windings: DistWindingDetail[]
) => ({
	kind: 'transformer' as const,
	id,
	from,
	to,
	conductors: [] as [string, string][],
	closed: true,
	n_phases,
	windings
});

describe('transformer kinds and labels', () => {
	const dyn = [
		winding('HV', 'delta', ['1', '2', '3'], 11_000),
		winding('LV', 'wye', ['1', '2', '3', '4'], 415)
	];
	const ct = [
		winding('M', 'wye', ['1', '4'], 7_200, 25e3),
		winding('S', 'wye', ['1', '4'], 120, 25e3),
		winding('S', 'wye', ['4', '2'], 120, 25e3)
	];
	const g: DistGraph = {
		buses: [bus('HV'), bus('LV'), bus('M'), bus('S'), bus('A'), bus('B')],
		edges: [
			transformer('T1', 'HV', 'LV', 3, dyn),
			transformer('CT', 'M', 'S', 1, ct),
			transformer('CT', 'M', 'S', 1, ct),
			transformer('CT', 'S', 'S', 1, ct),
			transformer('T2', 'A', 'B', 1, [
				winding('A', 'wye', ['1', '4'], 6_350, 50e3),
				winding('B', 'wye', ['1', '4'], 240, 50e3)
			])
		]
	};

	it('names a polyphase unit by its vector group, high side first, and its rating', () => {
		expect(transformerInfo(g, 0)).toMatchObject({ kind: 'poly', label: 'Dyn · 500 kVA' });
	});

	it('tells a centre tap from a plain single-phase unit', () => {
		expect(transformerInfo(g, 1)).toMatchObject({ kind: 'split', label: '1φ split · 25 kVA' });
		expect(transformerInfo(g, 4)).toMatchObject({ kind: 'single', label: '1φ · 50 kVA' });
	});

	it('marks windings inside a polyphase symbol, a tap on a split one, nothing on a single one', () => {
		const plain = transformerGlyph(null, 1).length;
		expect(transformerGlyph(transformerInfo(g, 0), 3).length).toBe(plain + 1 + 4);
		expect(transformerGlyph(transformerInfo(g, 1), 1).length).toBe(plain + 2);
		expect(transformerGlyph(transformerInfo(g, 4), 1).length).toBe(plain);
	});

	it('draws one symbol per transformer, not one per winding pair', () => {
		const placed = g.buses.map((b, i) => ({ id: b.id, lon: i * 10, lat: 0 }));
		const geometry = sourceLayout(g, placed, false, displayTree(g));
		const eq = schematicEquipment(g, geometry);
		const ids = eq.items.filter((i) => i.kind === 'transformer').map((i) => i.ids[0]);
		expect(ids.filter((id) => id === 'CT')).toHaveLength(1);
		expect(eq.routeGroups).toEqual([]);
	});
});

describe('attachment connections', () => {
	it('labels terminals compactly, with the connection in front', () => {
		expect(terminalLabel(['2', '4'], 'single_phase')).toBe('2–4');
		expect(terminalLabel(['1', '2', '3', '4'], 'wye')).toBe('Y 1234');
		expect(terminalLabel(['a', 'b', 'c'], 'delta')).toBe('Δ abc');
		expect(terminalLabel(['L1', 'L2', 'L3', 'N'], 'wye')).toBe('Y L1·L2·L3·N');
		expect(terminalConductors(['1', '4'], '4')).toEqual({ phases: 1, neutral: true });
		expect(terminalConductors(['1', '2', '3'], '4')).toEqual({ phases: 3, neutral: false });
	});

	it('splits a kind by how it connects and marks each drop with its conductors', () => {
		const g: DistGraph = {
			buses: [
				bus('S', { has_source: true }),
				bus('L', {
					attachments: [
						{ kind: 'load', id: 'a', terminals: ['1', '4'], connection: 'single_phase' },
						{ kind: 'load', id: 'b', terminals: ['2', '4'], connection: 'single_phase' },
						{ kind: 'load', id: 'c', terminals: ['2', '4'], connection: 'single_phase' },
						{ kind: 'load', id: 'd', terminals: ['1', '2', '3', '4'], connection: 'wye' }
					]
				})
			],
			edges: [
				{
					kind: 'line',
					id: 'l',
					from: 'S',
					to: 'L',
					conductors: [],
					closed: true,
					n_phases: 3
				}
			]
		};
		const placed = g.buses.map((b, i) => ({ id: b.id, lon: i * 10, lat: 0 }));
		const eq = schematicEquipment(g, sourceLayout(g, placed, false, displayTree(g)));
		const loads = eq.items.filter((i) => i.kind === 'load');
		expect(loads.map((l) => [l.ids, l.terminals?.join('')])).toEqual([
			[['a'], '14'],
			[['b', 'c'], '24'],
			[['d'], '1234']
		]);
		// Side by side along the slot, each on its own drop.
		expect(new Set(loads.map((l) => l.y)).size).toBe(1);
		expect(loads[1].x).toBeGreaterThan(loads[0].x);
		// Conductor marks on the drops, owned by the load bus (bus 1, owner −2); a neutral's mark is a
		// slash and a small circle, so there are more strokes than conductors.
		const marks = [...eq.ticks.edge].filter((o) => o < 0);
		expect(marks.length).toBeGreaterThanOrEqual(8);
		expect(new Set(marks)).toEqual(new Set([-2]));
	});
});
