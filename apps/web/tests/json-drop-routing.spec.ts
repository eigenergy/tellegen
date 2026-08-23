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
