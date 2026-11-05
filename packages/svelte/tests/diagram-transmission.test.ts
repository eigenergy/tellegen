import { describe, expect, it } from 'vitest';
import type { Network } from '@tellegen/engine';
import {
	branchAnchor,
	edgeAnchors,
	flowDirections,
	fromTxLayout,
	txLayoutInput,
	substationCentres,
	txBranchText,
	txBusText,
	txLandmarks,
	typicalBranchLength,
	txGraph,
	txTier,
	voltageLegend,
	txSourceLayout
} from '../src/lib/diagram/index.js';

/** 1 (345 kV) =T= 2 (138 kV) at one substation, a 138 kV line to 3, and 3 -- 4 out of service. */
const network = (grouped = true): Network => ({
	id: 't',
	name: 't',
	base_mva: 100,
	synthetic_coords: false,
	buses: [
		{ id: 1, lon: -97, lat: 31, demand_mw: 0, gen_mw: 500, base_kv: 345, substation: 0 },
		{ id: 2, lon: -97.001, lat: 31, demand_mw: 50, gen_mw: 0, base_kv: 138, substation: 0 },
		{ id: 3, lon: -96.5, lat: 31.2, demand_mw: 80, gen_mw: 0, base_kv: 138, substation: 1 },
		{ id: 4, lon: -96.4, lat: 31.3, demand_mw: 10, gen_mw: 0, base_kv: 138, substation: 2 }
	].map((b) => (grouped ? b : { ...b, substation: undefined, base_kv: b.base_kv })),
	branches: [
		{ id: 1, from: 1, to: 2, rate_mw: 400, status: 1, path: [], transformer: true },
		{
			id: 2,
			from: 2,
			to: 3,
			rate_mw: 200,
			status: 1,
			// A routed line with one bend.
			path: [
				[-97.001, 31],
				[-96.8, 31.15],
				[-96.5, 31.2]
			]
		},
		{ id: 3, from: 3, to: 4, rate_mw: 100, status: 0, path: [] }
	].map((br) => (grouped ? br : { ...br, transformer: undefined })),
	...(grouped
		? {
				substations: [
					{ key: 'sub:1', name: 'ALPHA', source: 'declared' as const, kv: [345, 138] },
					{ key: 'sub:2', name: null, source: 'declared' as const, kv: [138] },
					{ key: 'sub:3', name: null, source: 'declared' as const, kv: [138] }
				]
			}
		: {})
});

describe('transmission graph', () => {
	it('gives each voltage level its own legend entry within the budget', () => {
		const legend = voltageLegend(
			new Map([
				[345, 10],
				[138, 40],
				[13.8, 5],
				[0, 1]
			])
		);
		expect(legend.entries.map((e) => e.label)).toEqual([
			'345 kV',
			'138 kV',
			'13.8 kV',
			'voltage not given'
		]);
		expect(legend.entries.map((e) => e.tier)).toEqual(['ehv', 'hv', 'low', 'unknown']);
		expect(legend.entryOf(138)).toBe(1);
		expect(legend.entryOf(0)).toBe(3);
	});

	it('merges close low levels first and keeps high levels distinct longest', () => {
		const levels = new Map([765, 500, 345, 230, 138, 115, 69].map((kv) => [kv, 1]));
		const six = voltageLegend(levels, 6);
		expect(six.entries.map((e) => e.label)).toEqual([
			'765 kV',
			'500 kV',
			'345 kV',
			'230 kV',
			'115–138 kV',
			'69 kV'
		]);
		expect(six.entryOf(115)).toBe(4);
		expect(six.entryOf(138)).toBe(4);
		const four = voltageLegend(levels, 4);
		expect(four.entries.map((e) => e.label)).toEqual([
			'765 kV',
			'345–500 kV',
			'115–230 kV',
			'69 kV'
		]);
		// Levels that differ only by rounding are one level.
		expect(
			voltageLegend(
				new Map([
					[138, 1],
					[138.0001, 2]
				])
			).entries
		).toHaveLength(1);
	});

	it('shades levels within a tier and widens higher entries', () => {
		const legend = voltageLegend(new Map([345, 230, 138, 115].map((kv) => [kv, 1])));
		const hv = legend.entries.filter((e) => e.tier === 'hv');
		expect(hv.map((e) => [e.inTier, e.tierSize])).toEqual([
			[0, 3],
			[1, 3],
			[2, 3]
		]);
		expect(txTier(765)).toBe('ehv');
		expect(txTier(69)).toBe('sub');
		expect(txTier(0)).toBe('unknown');
	});

	it('reads the engine grouping, kV and transformer flags as columns', () => {
		const g = txGraph(network());
		expect(g.grouped).toBe(true);
		expect([...g.substation]).toEqual([0, 0, 1, 2]);
		expect([...g.from]).toEqual([0, 1, 2]);
		expect([...g.to]).toEqual([1, 2, 3]);
		expect([...g.transformer]).toEqual([1, 0, 0]);
		expect([...g.inService]).toEqual([1, 1, 0]);
		expect([...g.branchKv]).toEqual([345, 138, 138]);
	});

	it('falls back to one substation per bus and a voltage test on older payloads', () => {
		const g = txGraph(network(false));
		expect(g.grouped).toBe(false);
		expect([...g.substation]).toEqual([0, 1, 2, 3]);
		expect(g.substations[0]).toEqual({ key: 'bus:1', name: null, source: 'single', kv: [345] });
		// 345 to 138 kV reads as a transformer without the flag.
		expect([...g.transformer]).toEqual([1, 0, 0]);
	});

	it('projects geographic positions around their centroid and follows routed paths', () => {
		const net = network();
		const g = txGraph(net);
		const geo = txSourceLayout(g, net);
		expect(geo.mercatorOrigin).toBeDefined();
		// Positions are centred: they sum to (about) zero.
		let sx = 0;
		for (let i = 0; i < 4; i++) sx += geo.positions[i * 3];
		expect(Math.abs(sx)).toBeLessThan(1);
		// East is +x and north is -y (y down).
		expect(geo.positions[2 * 3]).toBeGreaterThan(geo.positions[0]);
		expect(geo.positions[2 * 3 + 1]).toBeLessThan(geo.positions[1]);
		// One segment for the transformer, two for the routed line, one for the open branch.
		expect([...geo.segmentEdge]).toEqual([0, 1, 1, 2]);
		expect([...geo.role]).toEqual([0, 0, 2]);
		// The route's last point is bus 3.
		expect(geo.targets[2 * 3]).toBeCloseTo(geo.positions[2 * 3], 0);
	});

	it('keeps drawing coordinates as given', () => {
		const net = { ...network(), coordinate_space: 'diagram' as const };
		const geo = txSourceLayout(txGraph(net), net);
		expect(geo.mercatorOrigin).toBeUndefined();
		expect(geo.positions[2 * 3] - geo.positions[0]).toBeCloseTo(0.5, 5);
	});

	it('centres each substation on its buses', () => {
		const net = network();
		const g = txGraph(net);
		const geo = txSourceLayout(g, net);
		const c = substationCentres(g, geo);
		expect(c[0]).toBeCloseTo((geo.positions[0] + geo.positions[3]) / 2, 3);
		expect(c[2]).toBeCloseTo(geo.positions[6], 3);
	});

	it('ranks landmarks: congested transformers, generators by size, then quiet transformers', () => {
		const net = network();
		const g = txGraph(net);
		const geo = txSourceLayout(g, net);
		const quiet = txLandmarks(g, geo);
		expect(quiet.map((l) => l.kind)).toEqual(['generator', 'transformer']);
		expect(quiet[0].weight).toBe(500);
		const hot = txLandmarks(g, geo, (k) => (k === 0 ? 'violation' : 'normal'));
		expect(hot.map((l) => [l.kind, l.tier])).toEqual([
			['transformer', 1],
			['generator', 2]
		]);
	});

	it('anchors a branch symbol midway along its ends, pointing from one to the other', () => {
		const net = network();
		const g = txGraph(net);
		const geo = txSourceLayout(g, net);
		const a = branchAnchor(g, geo, 1)!;
		expect(a.x).toBeCloseTo((geo.positions[3] + geo.positions[6]) / 2, 3);
		expect(Math.hypot(a.cos, a.sin)).toBeCloseTo(1, 9);
		expect(typicalBranchLength(g, geo)).toBeGreaterThan(0);
	});

	it('describes buses and branches for hover cards', () => {
		const net = network();
		const g = txGraph(net);
		expect(txBusText(g, net, 0)).toBe('bus 1\n345 kV · substation ALPHA\ngeneration up to 500 MW');
		expect(txBusText(g, net, 2, { label: 'LMP', text: '12.00 $/MWh' })).toContain('LMP 12.00');
		expect(txBranchText(g, net, 0, { mw: 120, loading: 0.3 })).toBe(
			'Transformer 1, bus 1 to bus 2\n345 kV / 138 kV · rating 400 MW\nflow 120 MW · 30 % of rating'
		);
		expect(txBranchText(g, net, 2)).toContain('out of service');
	});

	it('hands the engine the graph with the shown positions as seeds', () => {
		const net = network();
		const g = txGraph(net);
		const geo = txSourceLayout(g, net);
		const input = txLayoutInput(g, geo);
		expect(input.bus_substation).toEqual([0, 0, 1, 2]);
		expect(input.from).toEqual([0, 1, 2]);
		expect(input.in_service).toEqual([1, 1, 0]);
		expect(input.xy[4]).toBeCloseTo(geo.positions[6], 3);
	});

	it("turns the engine layout into drawable geometry in the seeds' frame", () => {
		const { geometry, parts } = fromTxLayout(
			{
				version: 1,
				positions: [0, 0, 10, 0],
				busbars: [0, -1, 0, 1, 10, -1, 10, 1],
				boxes: [-1, -2, 1, 2, 9, -2, 11, 2],
				segments: [0, 0, 5, 0, 5, 0, 10, 0],
				segment_edge: [0, 0],
				role: [0],
				unit: 0.5,
				scale: [1, 1],
				bounds: [-1, -2, 11, 2]
			},
			[100, 200]
		);
		expect([...geometry.positions]).toEqual([0, 0, 0, 10, 0, 0]);
		expect(geometry.mercatorOrigin).toEqual([100, 200]);
		expect([...geometry.segmentEdge]).toEqual([0, 0]);
		expect(parts.unit).toBe(0.5);
		expect(parts.boxes.length).toBe(8);
	});

	it('anchors each branch halfway along its drawn path', () => {
		const geometry = {
			positions: new Float32Array(9),
			// Branch 0: an L, 0,0 → 10,0 → 10,10 (length 20, halfway at the corner).
			// Branch 1: not drawn.
			sources: Float32Array.from([0, 0, 0, 10, 0, 0]),
			targets: Float32Array.from([10, 0, 0, 10, 10, 0]),
			segmentEdge: Int32Array.from([0, 0]),
			role: new Uint8Array(2),
			bounds: [0, 0, 10, 10] as [number, number, number, number]
		};
		const { anchors, lengths } = edgeAnchors(geometry, 2);
		expect([...anchors.subarray(0, 4)]).toEqual([10, 0, 1, 0]);
		expect(lengths[0]).toBe(20);
		expect(Number.isNaN(anchors[4])).toBe(true);
	});

	it('points flow arrows the way power flows along the drawn path', () => {
		const net = network();
		const g = txGraph(net);
		const geo = txSourceLayout(g, net);
		const { anchors } = edgeAnchors(geo, 3);
		// Branch 0 flows 1 → 2, branch 1 flows 3 → 2 (negative), branch 2 has no flow.
		const flows = [100, -40, null];
		const { edges, dir } = flowDirections(g, geo, anchors, (k) => flows[k]);
		expect(edges).toEqual([0, 1]);
		// Both paths run from → to in the payload, so the sign follows the flow.
		expect([...dir]).toEqual([1, -1]);
	});
	it.each([false, true])(
		'keeps arrows aligned on a doubling-back route (reversed: %s)',
		(reverse) => {
			const net = network();
			net.coordinate_space = 'diagram';
			net.buses[1].lon = 0;
			net.buses[1].lat = 0;
			net.buses[2].lon = 10;
			net.buses[2].lat = 4;
			const path: [number, number][] = [
				[0, 0],
				[20, 0],
				[20, 2],
				[-10, 2],
				[-10, 4],
				[10, 4]
			];
			net.branches[1].path = reverse ? path.reverse() : path;
			const g = txGraph(net);
			const geometry = txSourceLayout(g, net);
			const { anchors } = edgeAnchors(geometry, net.branches.length);
			// Its middle leg runs left, although the to bus is right of the from bus.
			expect(anchors[6]).toBeLessThan(0);
			expect([...flowDirections(g, geometry, anchors, (k) => (k === 1 ? 10 : null)).dir]).toEqual([
				1
			]);
			expect([...flowDirections(g, geometry, anchors, (k) => (k === 1 ? -10 : null)).dir]).toEqual([
				-1
			]);
			const first = [...geometry.segmentEdge].indexOf(1);
			expect(geometry.sources[first * 3]).toBeCloseTo(geometry.positions[3]);
			expect(geometry.sources[first * 3 + 1]).toBeCloseTo(geometry.positions[4]);
		}
	);
});
