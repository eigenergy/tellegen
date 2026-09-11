import { expect, test } from './fixtures/page-errors.js';

const POWER_MODELS_TWO_BUS = JSON.stringify({
	name: 'json-sidecar',
	baseMVA: 100,
	per_unit: true,
	bus: {
		'1': {
			index: 1,
			bus_i: 1,
			bus_type: 3,
			vm: 1,
			va: 0,
			base_kv: 230,
			vmax: 1.1,
			vmin: 0.9
		},
		'2': {
			index: 2,
			bus_i: 2,
			bus_type: 1,
			vm: 1,
			va: 0,
			base_kv: 230,
			vmax: 1.1,
			vmin: 0.9
		}
	},
	gen: {
		'1': {
			index: 1,
			gen_bus: 1,
			pg: 0.5,
			qg: 0,
			pmax: 2,
			pmin: 0,
			qmax: 1,
			qmin: -1,
			vg: 1,
			mbase: 100,
			gen_status: 1,
			model: 2,
			ncost: 3,
			cost: [0, 10, 0]
		}
	},
	load: { '1': { index: 1, load_bus: 2, pd: 0.5, qd: 0.1, status: 1 } },
	branch: {
		'1': {
			index: 1,
			f_bus: 1,
			t_bus: 2,
			br_r: 0.01,
			br_x: 0.1,
			b_fr: 0,
			b_to: 0,
			g_fr: 0,
			g_to: 0,
			tap: 1,
			shift: 0,
			br_status: 1,
			rate_a: 2,
			angmin: -0.5,
			angmax: 0.5,
			transformer: false
		}
	},
	shunt: {},
	storage: {},
	switch: {},
	dcline: {}
});

const BMOPF_TWO_BUS = JSON.stringify({
	name: 'micro-bmopf',
	base_frequency: 60,
	bus: {
		src: {
			terminal_names: ['1', '2', '3', '4'],
			perfectly_grounded_terminals: ['4'],
			longitude: -83.92,
			latitude: 35.96
		},
		load_bus: {
			terminal_names: ['1', '2', '3', '4'],
			perfectly_grounded_terminals: ['4'],
			longitude: -83.9,
			latitude: 35.95
		}
	},
	line: {
		l1: {
			bus_from: 'src',
			bus_to: 'load_bus',
			terminal_map_from: ['1', '2', '3'],
			terminal_map_to: ['1', '2', '3'],
			linecode: 'lc1',
			length: 100
		}
	},
	voltage_source: {
		vs: {
			bus: 'src',
			terminal_map: ['1', '2', '3', '4'],
			v_magnitude: [7200, 7200, 7200, 0],
			v_angle: [0, -2.0944, 2.0944, 0]
		}
	},
	load: {
		ld1: {
			bus: 'load_bus',
			terminal_map: ['1', '2', '3', '4'],
			configuration: 'WYE',
			p_nom: [50000, 50000, 50000],
			q_nom: [10000, 10000, 10000]
		}
	}
});

// The Rust byte classifier must leave an unknown JSON object unrouted. This guards
// that it falls through to the geo sidecar path and its precise error, instead of
// landing as a phantom empty multiconductor case.
test('a stray JSON object is not swallowed as an empty multiconductor case', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a
	// drop fired earlier is lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'stray.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify({ foo: 'bar', notes: [1, 2, 3] }))
		}
	]);

	await expect(page.locator('p.error')).toContainText('no bus coordinates', {
		timeout: 30_000
	});
	await expect(page.getByRole('heading', { name: /stray/i })).toHaveCount(0);
});

test('a balanced JSON case consumes a co-dropped geographic sidecar', async ({ page }) => {
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});

	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();

	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'json-sidecar.json',
			mimeType: 'application/json',
			buffer: Buffer.from(POWER_MODELS_TWO_BUS)
		},
		{
			name: 'json-sidecar-coords.csv',
			mimeType: 'text/csv',
			buffer: Buffer.from('bus_i,lat,lon\n1,37.77,-122.42\n2,37.78,-122.41\n')
		}
	]);

	await expect(page.getByRole('heading', { name: /json-sidecar/i })).toBeVisible({
		timeout: 30_000
	});
	await expect(
		page.getByText('coordinates: geographic file data from json-sidecar-coords.csv')
	).toBeVisible();
	await expect(page.getByText('click the map to place the topology layout')).toHaveCount(0);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });
});

test('a BMOPF distribution case uses the same four-pane information hierarchy', async ({
	page
}) => {
	await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"]').setInputFiles({
		name: 'micro-bmopf.json',
		mimeType: 'application/json',
		buffer: Buffer.from(BMOPF_TWO_BUS)
	});

	await expect(page.getByRole('heading', { name: /micro-bmopf/i })).toBeVisible({
		timeout: 30_000
	});
	for (const name of ['Case overview', 'Analysis', 'Element inspector', 'Map display']) {
		await expect(page.getByRole('button', { name: new RegExp(name, 'i') })).toHaveAttribute(
			'aria-expanded',
			'true'
		);
	}
	await expect(page.locator('[data-pane="analysis"]')).toContainText('viewing only');
	await expect(page.locator('[data-pane="map-display"]')).toContainText('conductors');
	await expect(page.locator('[data-pane="map-display"]')).toContainText('kW, max(load');
});
