import { expect, test } from './fixtures/page-errors.js';
import type { Route } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { CASE3_COORDS, CASE3_PLANNING } from './fixtures/planning-case.js';
import { mockDataRoutes } from './fixtures/backend-case.js';

for (const status of [404, 200]) {
	test(`AC OPF stays unavailable when its asset is ${status === 404 ? 'missing' : 'invalid'}`, async ({
		page
	}) => {
		let probeClosed = false;
		page.on('worker', (worker) => {
			if (worker.url().includes('acopf-worker'))
				worker.on('close', () => {
					probeClosed = true;
				});
		});
		await page
			.context()
			.route('**/experimental-acopf/tellegen_acopf_wasi.wasm', (route) =>
				route.fulfill({ status, contentType: 'application/wasm', body: 'invalid WASM' })
			);
		await mockDataRoutes(page);
		await page.goto('/');
		await expect.poll(() => probeClosed).toBe(true);
		const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
		await expect(calculation).toHaveValue('dcopf');
		await expect(calculation).toBeEnabled();
		await expect(calculation.locator('option[value="acopf"]')).toHaveJSProperty('disabled', true);
		await expect(page.getByRole('complementary', { name: 'Network', exact: true })).toContainText(
			'1,234.5'
		);
		await expect(page.locator('.error')).toHaveCount(0);
	});
}

for (const mode of ['preview', 'development']) {
	test(`the default ${mode} app runs AC OPF in the isolated WASI worker`, async ({
		page
	}, testInfo) => {
		test.setTimeout(180_000);
		const workers: { closed: boolean }[] = [];
		page.on('worker', (worker) => {
			if (!worker.url().includes('acopf-worker')) return;
			const state = { closed: false };
			workers.push(state);
			worker.on('close', () => {
				state.closed = true;
			});
		});
		await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
		await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
		const keyedPort = Number(
			process.env.TELLEGEN_KEYED_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 2
		);
		await page.goto(mode === 'preview' ? '/' : `http://127.0.0.1:${keyedPort}/`);
		await expect(page.getByText('no default cases loaded')).toBeVisible();
		await page.locator('input[type="file"][multiple]').setInputFiles([
			{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
			{ name: 'case3opf.m', mimeType: 'text/plain', buffer: Buffer.from(CASE3_PLANNING) }
		]);
		const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
		await expect(calculation).toBeEnabled({ timeout: 60_000 });
		await expect(calculation).toHaveValue('dcopf');
		await expect(calculation.locator('option[value="acopf"]')).toHaveJSProperty('disabled', false);
		await calculation.selectOption('acopf');
		await expect(page.locator('.solvecard')).toContainText('AC OPF');
		await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
		await expect(calculation).toBeEnabled();
		await expect(page.getByRole('button', { name: '|V|', exact: true })).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		await expect(page.getByRole('button', { name: 'LMP', exact: true })).toHaveCount(0);
		await expect(page.locator('.error')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Save case', exact: true })).toBeDisabled();
		await expect(page.getByRole('button', { name: 'Export as', exact: true })).toBeDisabled();
		await expect(
			page.getByText('Saving and exporting AC OPF cases is not supported yet.', { exact: true })
		).toBeVisible();
		await page.screenshot({ path: testInfo.outputPath('transmission-acopf.png') });
		await expect.poll(() => workers.length).toBe(2); // probe plus first solve
		await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);

		// A second solve must use and dispose a new worker, not retain its WASI memory.
		await calculation.selectOption('dcopf');
		await expect(calculation).toBeEnabled();
		await calculation.selectOption('acopf');
		await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
		await expect.poll(() => workers.length).toBe(3);
		await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);

		// Hold the next real worker's fetch so removal deterministically happens
		// while it is pending, without relying on how fast a three-bus solve runs.
		await calculation.selectOption('dcopf');
		await expect(calculation).toBeEnabled();
		let held: Route | undefined;
		await page.context().route('**/experimental-acopf/tellegen_acopf_wasi.wasm', (route) => {
			held = route;
		});
		await calculation.selectOption('acopf');
		await expect.poll(() => !!held).toBe(true);
		await expect.poll(() => workers.length).toBe(4);
		await page.locator('.case-chip.local .case-remove').click();
		await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);
		await held!.abort();
		await expect(page.locator('.solvecard')).toHaveCount(0);
		await expect(page.locator('.error')).toHaveCount(0);
	});
}

test('a backend transmission case switches from cached DC OPF to browser AC OPF', async ({
	page
}, testInfo) => {
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/acopf.html`);
	await page.waitForFunction(() => typeof window.prepareBackendTransmissionCase === 'function');
	const fixture = await page.evaluate(
		(text) => window.prepareBackendTransmissionCase(text),
		CASE3_PLANNING
	);
	const { network, solution, moduleJson, nGen } = fixture;
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) =>
		route.fulfill({
			json: [
				{
					id: network.id,
					name: network.name,
					n_bus: network.buses.length,
					n_branch: network.branches.length,
					n_gen: nGen
				}
			]
		})
	);
	await page.route('**/api/cases/transmission/network', (route) =>
		route.fulfill({ json: network })
	);
	await page.route('**/api/cases/transmission/solution', (route) =>
		route.fulfill({ json: solution })
	);
	let moduleLoads = 0;
	await page.route('**/api/cases/transmission/case', (route) => {
		moduleLoads++;
		return route.fulfill({ contentType: 'application/json', body: moduleJson });
	});
	let serverSolves = 0;
	await page.route('**/api/cases/transmission/solve**', (route) => {
		serverSolves++;
		return route.abort();
	});
	await page.goto('/');
	const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
	await expect(calculation).toHaveValue('dcopf');
	await expect(calculation.locator('option[value="acopf"]')).toHaveJSProperty('disabled', false);
	await calculation.selectOption('acopf');
	await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
	await expect(calculation).toBeEnabled();
	await expect(calculation).toHaveValue('acopf');
	expect(moduleLoads).toBeGreaterThan(0);
	expect(serverSolves).toBe(0);
	await expect(page.locator('.error')).toHaveCount(0);
	await page.screenshot({ path: testInfo.outputPath('backend-transmission-acopf.png') });
});

test('imports and solves a retained canonical AC OPF instance', async ({ page }) => {
	test.setTimeout(180_000);
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/acopf.html`);
	await page.waitForFunction(() => typeof window.prepareAcOpfInstance === 'function');
	const moduleJson = await page.evaluate(
		(text) => window.prepareAcOpfInstance(text),
		CASE3_PLANNING
	);
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type="file"][multiple]').setInputFiles([
		{ name: 'canonical.pio.json', mimeType: 'application/json', buffer: Buffer.from(moduleJson) },
		{ name: 'case3-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) }
	]);
	const calculation = page.getByRole('combobox', { name: 'Calculation', exact: true });
	await expect(calculation).toHaveValue('acopf');
	await expect(page.locator('.solvecard')).toContainText('NLP iterations', { timeout: 120_000 });
	await expect(calculation.locator('option[value="dcopf"]')).toHaveJSProperty('disabled', true);
	await expect(page.locator('.error')).toHaveCount(0);
});

for (const phase of ['probe', 'solve'] as const) {
	test(`unmounting the provider terminates a pending ${phase} worker`, async ({ page }) => {
		const port = Number(
			process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
		);
		const workers: { closed: boolean }[] = [];
		page.on('worker', (worker) => {
			if (!worker.url().includes('acopf-worker')) return;
			const state = { closed: false };
			workers.push(state);
			worker.on('close', () => {
				state.closed = true;
			});
		});
		let held: Route | undefined;
		const holdAsset = () =>
			page.context().route('**/tellegen_acopf_wasi.wasm*', (route) => {
				if (route.request().resourceType() !== 'fetch') return route.continue();
				held = route;
			});
		if (phase === 'probe') await holdAsset();
		await page.goto(`http://127.0.0.1:${port}/acopf-lifecycle.html`);
		await page.waitForFunction(() => !!window.acOpfLifecycle);
		if (phase === 'solve') {
			await expect.poll(() => page.evaluate(() => window.acOpfLifecycle.available())).toBe(true);
			await holdAsset();
			await page.evaluate((text) => window.acOpfLifecycle.start(text), CASE3_PLANNING);
		}
		await expect.poll(() => !!held).toBe(true);
		await page.evaluate(() => window.unmountAcOpfProvider());
		await expect.poll(() => workers.length).toBe(phase === 'probe' ? 1 : 2);
		await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);
		await held!.abort();
		expect(await page.evaluate(() => window.acOpfLifecycle.solved())).toBe(false);
	});
}

// Opt in with a checksum-keyed independent oracle, as acopf_benchmark does.
// Large external datasets remain outside ordinary CI and the repository.
const corpus = process.env.TELLEGEN_ACOPF_CORPUS;
const referencePath = process.env.TELLEGEN_ACOPF_REFERENCE;
for (const name of (
	process.env.TELLEGEN_ACOPF_CASES ??
	'pglib_opf_case14_ieee,pglib_opf_case30_ieee,pglib_opf_case300_ieee'
).split(',')) {
	test(`released POUNCE artifact agrees with the independent oracle: ${name}`, async ({
		page,
		request
	}, testInfo) => {
		test.skip(!corpus || !referencePath, 'requires external cases and checksum-keyed oracle');
		test.setTimeout(600_000);
		const bytes = readFileSync(join(corpus!, `${name}.m`));
		const inputHash = createHash('sha256').update(bytes).digest('hex');
		const reference = JSON.parse(readFileSync(referencePath!, 'utf8')) as {
			provenance: string;
			objectives: Record<string, number>;
		};
		expect(reference.provenance.trim()).not.toBe('');
		const objective = reference.objectives[inputHash];
		expect(Number.isFinite(objective), `missing independent reference for ${inputHash}`).toBe(true);
		const previewPort = Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173);
		const port = Number(process.env.TELLEGEN_MC_PREVIEW_PORT ?? previewPort + 1);
		const wasmUrl = `http://127.0.0.1:${previewPort}/experimental-acopf/tellegen_acopf_wasi.wasm`;
		const manifestResponse = await request.get(
			wasmUrl.replace('tellegen_acopf_wasi.wasm', 'POUNCE-SOURCE.json')
		);
		expect(manifestResponse.ok()).toBe(true);
		const artifact = await manifestResponse.json();
		const served = await request.get(wasmUrl);
		expect(served.ok()).toBe(true);
		expect(
			createHash('sha256')
				.update(await served.body())
				.digest('hex')
		).toBe(artifact.artifact_sha256);
		const workers: { closed: boolean }[] = [];
		page.on('worker', (worker) => {
			if (!worker.url().includes('acopf-worker')) return;
			const state = { closed: false };
			workers.push(state);
			worker.on('close', () => {
				state.closed = true;
			});
		});
		await page.goto(`http://127.0.0.1:${port}/acopf.html`);
		await page.waitForFunction(() => typeof window.solveAcOpfCase === 'function');
		const started = Date.now();
		const response = await page.evaluate(({ text, url }) => window.solveAcOpfCase(text, url), {
			text: bytes.toString('utf8'),
			url: wasmUrl
		});
		const elapsedMs = Date.now() - started;
		expect(response.formulation).toBe('acopf');
		expect(response.status).toBe('feasible');
		expect(
			Math.abs(response.objective! - objective) / Math.max(1, Math.abs(objective))
		).toBeLessThan(1e-4);
		const iterations = response.iterations;
		if (!iterations || Array.isArray(iterations) || !('nlp_iterations' in iterations))
			throw new Error('missing nonlinear solve diagnostics');
		// Physical mismatch is now MW/MVAr; the oracle cases use a 100 MVA base.
		expect(iterations.primal_residual).toBeLessThan(1e-4);
		expect(iterations.residuals?.active_balance_mw).toBeLessThan(1e-4);
		expect(iterations.residuals?.reactive_balance_mvar).toBeLessThan(1e-4);
		expect(iterations.residuals?.objective_relative_error).toBeLessThan(1e-6);
		expect(iterations.base_restoration_calls).toBeGreaterThanOrEqual(0);
		expect(iterations.base_solver_status).toBeTruthy();
		expect(iterations.total_nlp_iterations).toBeGreaterThanOrEqual(iterations.nlp_iterations);
		expect(iterations.restoration_calls).toBeGreaterThanOrEqual(0);
		await expect.poll(() => workers.length).toBe(1);
		await expect.poll(() => workers.every((worker) => worker.closed)).toBe(true);
		await testInfo.attach(`${name}.json`, {
			body: JSON.stringify({
				inputHash,
				artifact,
				provenance: reference.provenance,
				objective,
				elapsedMs,
				response
			}),
			contentType: 'application/json'
		});
	});
}
