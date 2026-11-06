import { describe, expect, it } from 'vitest';
import type { Network, NetworkShunt } from '@tellegen/engine';
import {
	equipmentDrawing,
	equipmentGlyph,
	equipmentItems,
	equipmentTreatment,
	shuntKind
} from '../src/lib/diagram/transmission-equipment.js';
import { txGraph, txSourceLayout } from '../src/lib/diagram/transmission.js';
import { placeSyntheticTopology } from '../src/lib/synthetic-layout.js';
const shunt = (b: number, extra: Partial<NetworkShunt> = {}): NetworkShunt => ({
	id: 1,
	uid: null,
	bus: 1,
	in_service: true,
	g_mw: 0,
	b_mvar: b,
	section_count: null,
	control_mode: null,
	blocks: [],
	...extra
});
const network: Network = {
	id: 'equipment',
	name: 'Equipment',
	base_mva: 100,
	synthetic_coords: false,
	coordinate_space: 'diagram',
	buses: [1, 2].map((id) => ({
		id,
		lon: id * 100,
		lat: 0,
		demand_mw: 0,
		gen_mw: 0,
		base_kv: 138
	})),
	branches: [
		{
			id: 1,
			from: 1,
			to: 2,
			status: 1,
			transformer: true,
			rate_mw: 100,
			path: [
				[100, 0],
				[200, 0]
			]
		}
	],
	equipment: {
		transformers: [
			{
				branch: 1,
				uid: null,
				phase_shifter: true,
				tap_ratio: 1,
				phase_shift_deg: 0,
				control_mode: 'active_flow',
				control_enabled: true,
				controlled_bus: null,
				setting_min: -20,
				setting_max: 20
			}
		],
		shunts: [shunt(25), shunt(-25, { id: 2, in_service: false })],
		hvdc: [
			{
				id: 1,
				uid: null,
				from: 1,
				to: 2,
				in_service: true,
				pf_mw: -90,
				pt_mw: -95,
				nominal_kv: 320,
				from_converter: 'vsc',
				to_converter: 'vsc'
			}
		],
		detailed_dc: { nodes: 0, lines: 0, converters: 0 }
	}
};

describe('transmission equipment semantics', () => {
	it('classifies pure banks but preserves unknown, conductive and mixed devices', () => {
		expect(shuntKind(shunt(25))).toBe('capacitor');
		expect(shuntKind(shunt(-25))).toBe('reactor');
		expect(shuntKind(shunt(0))).toBe('shunt');
		expect(shuntKind(shunt(25, { g_mw: 1 }))).toBe('shunt');
		expect(shuntKind(shunt(0, { blocks: [{ steps: 2, g_mw: 0, b_mvar: 10 }] }))).toBe('capacitor');
		expect(shuntKind(shunt(25, { blocks: [{ steps: 2, g_mw: 0, b_mvar: -10 }] }))).toBe('shunt');
	});
	it('keeps zero-angle PSTs, individual banks and signed HVDC input powers', () => {
		const items = equipmentItems(network);
		expect(items).toHaveLength(4);
		expect(items[0].title).toContain('PST');
		expect(items[0].facts.join(' ')).toContain('Phase setting 0°');
		expect(items[2].state).toBe('Out of service');
		expect(items[3].facts).toContain('Net input loss: 5 MW');
		expect(items[3].explanation).toContain('not solved DC voltages');
	});
	it('retains metadata through synthetic placement without making HVDC an AC edge', () => {
		const placed = placeSyntheticTopology(network, { lon: 0, lat: 0 });
		expect(placed.equipment).toBe(network.equipment);
		expect(placed.branches).toHaveLength(1);
	});
	it('draws distinct devices and selectable DC labels independently of AC edges', () => {
		const graph = txGraph(network),
			geometry = txSourceLayout(graph, network);
		const drawing = equipmentDrawing(
			network,
			graph,
			geometry,
			null,
			new Float32Array([0, 0, 1, 0]),
			5,
			true
		);
		expect(drawing.labels.map((l) => l.text)).toEqual(['S1', 'OFF · S2', 'PST 0°', 'DC 1 · input']);
		expect(drawing.strokes.every((s) => [...s.from, ...s.to].every(Number.isFinite))).toBe(true);
		expect(graph.from.length).toBe(1);
		expect(equipmentGlyph('capacitor')).not.toEqual(equipmentGlyph('reactor'));
	});
	it('keeps switch state and formulation capability independent of device identity', () => {
		expect(equipmentGlyph('switch', true)).not.toEqual(equipmentGlyph('switch', false));
		expect(equipmentTreatment('hvdc', 'socwr')).toContain('unsupported');
		expect(equipmentTreatment('hvdc', 'acpf')).toContain('Fixed terminal injections');
		expect(equipmentTreatment('capacitor', 'dcpf')).toContain('susceptance is omitted');
		expect(equipmentTreatment('capacitor', 'acpf')).toContain('voltage squared');
	});
	it('omits drawing paths with unavailable terminals, retaining the inventory', () => {
		const missing = { ...network, buses: network.buses.slice(0, 1) };
		const graph = txGraph(missing),
			geometry = txSourceLayout(graph, missing);
		const drawing = equipmentDrawing(missing, graph, geometry, null, new Float32Array(), 5, false);
		expect(drawing.strokes).toHaveLength(0);
		expect(equipmentItems(missing).some((i) => i.kind === 'hvdc')).toBe(true);
	});
});
