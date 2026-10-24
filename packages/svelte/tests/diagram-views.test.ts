import { describe, expect, it } from 'vitest';
import { priceColor, sensColor } from '../src/lib/colors.js';
import {
	bandColour,
	bandPosition,
	branchLabel,
	buildSearchIndex,
	busLabel,
	chordsInto,
	colourFamily,
	conductorBars,
	decodeCamera,
	ease,
	encodeCamera,
	entityKey,
	lerpInto,
	loadingColour,
	pairBar,
	placeLabels,
	queryTable,
	screenAnchored,
	search,
	tableCsv,
	tableModel,
	titleBlock,
	viewportSvg,
	withView,
	worstBand,
	TITLE_BLOCK_HEIGHT,
	type PairVoltage,
	type ResultSnapshot,
	type SearchEntity
} from '../src/lib/diagram/index.js';

const pn = (t: string, volts: number, min = 200, max = 260): PairVoltage => ({
	pair: [t, 'n'],
	zone: 'phase–neutral',
	volts,
	min,
	max
});

describe('colour modes', () => {
	it('places voltage in the declared band and keeps the pair nearest a limit', () => {
		expect(bandPosition(pn('1', 200))).toBe(-1);
		expect(bandPosition(pn('1', 230))).toBe(0);
		expect(
			bandPosition({ pair: ['1', 'n'], zone: 'phase–neutral', volts: 230, min: null, max: null })
		).toBeNull();
		expect(worstBand([pn('1', 230), pn('2', 258), pn('3', 203)])).toBe(bandPosition(pn('2', 258)));
		expect(worstBand([])).toBeNull();
	});

	it('uses the application ramps directly, clamped at the limits', () => {
		expect(loadingColour(0)).toEqual(priceColor(0.15));
		expect(loadingColour(180)).toEqual(priceColor(1));
		expect(bandColour(-5)).toEqual(sensColor(-1));
		expect(bandColour(0)).toEqual(sensColor(0));
		expect(colourFamily('voltage-band')).toBe('voltage');
		expect(colourFamily('none')).toBe('none');
	});
});

describe('search', () => {
	const entities: SearchEntity[] = [
		{ kind: 'bus', index: 0, id: 'MV1', family: 'bus', source: 'Feeder', qualified: 's/MV1' },
		{ kind: 'bus', index: 1, id: 'LV1', family: 'bus', source: 'Feeder', qualified: 's/LV1' },
		{ kind: 'bus', index: 2, id: 'LV10', family: 'bus', source: 'Feeder', qualified: 's/LV10' },
		{ kind: 'bus', index: 3, id: 'LV1', family: 'bus', source: 'Other', qualified: 't/LV1' },
		{ kind: 'branch', index: 0, id: 'LV1', family: 'switch', qualified: 's/LV1' },
		{ kind: 'branch', index: 1, id: 'T1', family: 'transformer / 2w', qualified: 's/T1' }
	];
	const index = buildSearchIndex(entities);

	it('ranks exact, qualified, prefix and substring matches; buses first on ties', () => {
		const lv1 = search(index, 'lv1');
		expect(lv1.matches.slice(0, 3).map((m) => [m.kind, m.qualified])).toEqual([
			['bus', 's/LV1'],
			['bus', 't/LV1'],
			['branch', 's/LV1']
		]);
		expect(lv1.matches[0].exact).toBe(true);
		expect(lv1.total).toBe(4);
		expect(search(index, 't/lv1').matches[0]).toMatchObject({ qualified: 't/LV1', exact: true });
		expect(search(index, '  ').total).toBe(0);
		expect(search(index, '1', { kinds: ['branch'] }).matches.map((m) => m.id)).toEqual([
			'T1',
			'LV1'
		]);
	});

	it('builds readable, unique keys even when raw IDs repeat', () => {
		const keys = entities.map(entityKey);
		expect(new Set(keys).size).toBe(entities.length);
		expect(keys.slice(4)).toEqual(['branch:switch:s/LV1', 'branch:transformer.2w:s/T1']);
		expect(index.byKey.get('bus:t/LV1')?.index).toBe(3);
	});
});

const snapshot: ResultSnapshot = {
	kind: 'solved',
	label: 'test',
	iterations: 1,
	buses: [
		{ pairs: [pn('1', 230), pn('2', 231), pn('3', 229)], unbalance: null },
		{ pairs: [pn('1', 195), pn('2', 230), pn('3', 240)], unbalance: null },
		{ pairs: [], unbalance: null }
	],
	branches: [
		{
			ends: [
				{ p: 100, q: 10, currents: [] },
				{ p: -98, q: -9, currents: [] }
			],
			loading: 95,
			basis: null,
			primary: true,
			loss: { p: 2, q: 1 }
		},
		{
			ends: [
				{ p: -50, q: 5, currents: [] },
				{ p: 50.5, q: -4, currents: [] }
			],
			loading: 40,
			basis: null,
			primary: true,
			loss: { p: 0.5, q: 1 }
		},
		{ ends: [null, null], loading: null, basis: null, primary: true, loss: null }
	]
};
const names = {
	buses: [{ id: 'B1' }, { id: 'B2' }, { id: '=cmd' }],
	branches: [
		{ id: 'L1', family: 'line', from: 0, to: 1, inService: true },
		{ id: 'T1', family: 'transformer / 2w', from: 1, to: 2, inService: true },
		{ id: 'S1', family: 'switch', from: 0, to: 2, inService: false }
	]
};

describe('results table', () => {
	it('builds rows with pair voltages, band, unbalance basis and status', () => {
		const row = tableModel(snapshot, names, 'bus').row(1);
		expect([row.id, row.pairs, row.reference, row.v0, row.state, row.unbalanceBasis]).toEqual([
			'B2',
			'1–n 2–n 3–n',
			'phase–neutral',
			195,
			'violation',
			'IEEE PVUR'
		]);
		const branch = tableModel(snapshot, names, 'branch');
		expect([branch.row(0).loss, branch.row(0).state, branch.row(2).state]).toEqual([
			2,
			'warning',
			'inactive'
		]);
	});

	it('filters, sorts with missing values last both ways, and exports safe full-precision CSV', () => {
		const branch = tableModel(snapshot, names, 'branch');
		expect([...queryTable(branch, { text: 't1' })]).toEqual([1]);
		expect([...queryTable(branch, { issuesOnly: true })]).toEqual([0]);
		expect([...queryTable(branch, { sortKey: 'loading' })]).toEqual([1, 0, 2]);
		expect([...queryTable(branch, { sortKey: 'loading', descending: true })]).toEqual([0, 1, 2]);
		const bus = tableModel(snapshot, names, 'bus');
		expect([...queryTable(bus, { sortKey: 'band', descending: true })]).toEqual([1, 0, 2]);
		const csv = tableCsv(bus, [0, 2], ['case: test']).split('\r\n');
		expect(csv[0]).toBe('# case: test');
		expect(csv[1]).toContain('V1 (V)');
		expect(csv[3].startsWith(`"'=cmd"`)).toBe(true);
		expect(tableCsv(branch, [0]).split('\r\n')[1]).toContain(',-98,');
	});
});

describe('value labels', () => {
	const c = (
		id: string,
		x: number,
		y: number,
		state: 'normal' | 'violation' = 'normal',
		priority = 0,
		selected = false
	) => ({
		id,
		text: id,
		screen: [x, y] as [number, number],
		state,
		priority,
		selected
	});

	it('places the selection, then severity, then magnitude, rejecting overlaps', () => {
		const out = placeLabels(
			[
				c('low', 100, 100, 'normal', 1),
				c('high', 105, 100, 'normal', 9),
				c('bad', 300, 100, 'violation'),
				c('sel', 102, 101, 'normal', 0, true)
			],
			{ width: 800, height: 600 }
		);
		expect(out.map((o) => o.id)).toEqual(['sel', 'bad']);
		expect(placeLabels([c('edge', 790, 300)], { width: 800, height: 600 })).toEqual([]);
	});

	it('labels the pair nearest a limit and the branch value for the view family', () => {
		expect(busLabel(snapshot, 1)).toMatchObject({ text: '195 V', state: 'violation' });
		expect(busLabel(snapshot, 2)).toBeNull();
		expect(branchLabel(snapshot, 0, 'loading')).toMatchObject({ text: '95.0 %', state: 'warning' });
		expect(branchLabel(snapshot, 1, 'power')?.text).toBe('50 kW');
		expect(branchLabel(snapshot, 2, 'loading')).toBeNull();
	});
});

describe('svg export', () => {
	const set = (segments: number[][], colors: number[][], widths: Float32Array | null = null) => ({
		sources: Float32Array.from(segments.flatMap((s) => [s[0], s[1], 0])),
		targets: Float32Array.from(segments.flatMap((s) => [s[2], s[3], 0])),
		colors: Uint8Array.from(colors.flat()),
		widths,
		count: segments.length
	});
	const base = {
		bounds: [0, 0, 100, 50] as [number, number, number, number],
		scale: 2,
		width: 200,
		height: 100,
		background: '#fff',
		ink: '#000',
		muted: '#666',
		title: { left: ['Feeder <A>'], right: ['Saved'] }
	};

	it('keeps visible segments, groups by colour and width, and escapes text', () => {
		const svg = viewportSvg({
			...base,
			lineSets: [
				set(
					[
						[0, 0, 10, 0],
						[20, 0, 30, 0],
						[200, 200, 300, 300],
						[90, 40, 150, 40]
					],
					[
						[255, 0, 0, 255],
						[255, 0, 0, 255],
						[255, 0, 0, 255],
						[0, 0, 255, 255]
					]
				)
			],
			texts: [{ position: [5, 5], text: '<b>&' }]
		});
		expect(svg.match(/<path /g)).toHaveLength(2);
		expect(svg).toContain('M0 0L20 0M40 0L60 0');
		expect(svg).toContain('&lt;b&gt;&amp;');
		expect(svg).toContain('Feeder &lt;A&gt;');
		expect(svg).toContain(`height="${100 + TITLE_BLOCK_HEIGHT}"`);
	});

	it('rotates glyphs clockwise and refuses oversized views', () => {
		const svg = viewportSvg({
			...base,
			glyphs: [{ position: [10, 10], text: '▸', angle: 90, color: 'teal' }]
		});
		expect(svg).toContain('rotate(-90 20 20)');
		const n = 200_001;
		const many = {
			sources: new Float32Array(n * 3),
			targets: new Float32Array(n * 3).fill(1),
			colors: new Uint8Array(n * 4).fill(255),
			count: n
		};
		expect(() => viewportSvg({ ...base, lineSets: [many] })).toThrow(/zoom in/);
	});

	it('title blocks carry the result revision and flag stress data', () => {
		const camera = { target: [1, 2], zoom: 1 };
		const t = titleBlock({
			dataset: 'French',
			layout: 'Tidy tree',
			view: 'Loading',
			camera,
			result: { kind: 'solved', label: 'PF', iterations: 16, revision: 'abcdef0123456789abcdef' },
			exported: 'now'
		});
		expect(t.right[0]).toBe('PF · 16 iterations');
		expect(t.right[1]).toBe('Result revision abcdef0123456789…');
		expect(t.left[3]).toContain('2.00×');
		expect(
			titleBlock({
				dataset: 'S',
				layout: 'L',
				view: 'V',
				camera,
				result: { kind: 'stress', label: '', iterations: null, revision: 'x' },
				exported: 't'
			}).right[0]
		).toContain('NOT A SOLUTION');
	});
});

describe('phase bars and morph', () => {
	it('draws pair bars on a band ±25 % track and conductor bars on one shared scale', () => {
		const bar = pairBar(pn('1', 230));
		expect(bar.band?.left).toBeCloseTo(100 / 6, 10);
		expect(bar.band?.width).toBeCloseTo(200 / 3, 10);
		expect([bar.marker, bar.state]).toEqual([50, 'normal']);
		expect(pairBar(pn('1', 1e9)).marker).toBe(100);
		expect(
			pairBar({ pair: ['1', 'n'], zone: 'phase–neutral', volts: 230, min: null, max: null })
		).toMatchObject({
			band: null,
			state: 'unbounded'
		});
		const end = (a: number, nA: number) => ({
			p: 10,
			q: 2,
			currents: [
				{ terminal: '1', amps: a },
				{ terminal: '2', amps: 50 },
				{ terminal: 'n', amps: nA }
			]
		});
		const bars = conductorBars([end(100, 40), end(99, 40)]);
		expect(bars?.scaleAmps).toBe(100);
		expect(bars?.ends[0]?.bars.map((b) => [b.fill, b.neutral])).toEqual([
			[100, false],
			[50, false],
			[40, true]
		]);
		expect(bars?.neutralShare).toBe(40);
		expect(conductorBars([null, null])).toBeNull();
	});

	it('anchors morph start positions to the current screen, interpolates and draws chords', () => {
		const start = screenAnchored(
			Float32Array.of(10, 20, 0, 30, 40, 0),
			{ target: [10, 20], zoom: 1 },
			{ target: [100, 200], zoom: 0 }
		);
		expect([...start]).toEqual([100, 200, 0, 140, 240, 0]);
		const out = lerpInto(new Float32Array(6), start, new Float32Array(6), 0.5);
		expect([...out]).toEqual([50, 100, 0, 70, 120, 0]);
		const s = new Float32Array(3);
		const t = new Float32Array(3);
		chordsInto(s, t, Uint32Array.of(1, 0), out);
		expect([...s, ...t]).toEqual([70, 120, 0, 50, 100, 0]);
		expect([ease(0), ease(0.5), ease(1)]).toEqual([0, 0.5, 1]);
	});
});

describe('view state', () => {
	it('round-trips cameras, rejects malformed ones and merges URL state', () => {
		expect(decodeCamera(encodeCamera({ target: [12.345, -6.78, 0], zoom: 1.234 }))).toEqual({
			target: [12.3, -6.8, 0],
			zoom: 1.23
		});
		for (const bad of ['', '1,2', '1,2,x', '1,2,9', null]) expect(decodeCamera(bad)).toBeNull();
		expect(
			withView('http://x/p/?dataset=a&keep=1&sel=bus:s/1', { sel: null, view: '1,2,0' }).search
		).toBe('?dataset=a&keep=1&view=1%2C2%2C0');
	});
});
