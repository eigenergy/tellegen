import { readFile } from 'node:fs/promises';
import type { Locator, Page } from '@playwright/test';

import { expect, test } from './fixtures/page-errors.js';

// These are local browser/WASM tests. Make startup deterministic and wait for
// hydration before dispatching file-input events, including under parallel CI.
test.beforeEach(async ({ page }) => {
	await page.route('**/api/compute', (route) => route.fulfill({ json: { enabled: false } }));
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
});

/** Open the app and wait until it has hydrated: the prerendered file input exists before hydration
 * attaches its listener, so a file set earlier is lost. The empty-cases panel renders only after load. */
async function openApp(page: Page) {
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
}

const typedCase = 'tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json';
const loadModelsCase = 'tests/fixtures/mc-pf-browser/public/mc-pf-load-models.bmopf.json';

async function noNeutralTypedCase(): Promise<string> {
	const text = await readFile(typedCase, 'utf8');
	const document = JSON.parse(text) as {
		value: {
			data: {
				network: {
					buses: Array<{ terminals: string[]; grounded: string[] }>;
					linecodes: Array<Record<string, unknown>>;
					lines: Array<{ terminal_map_from: string[]; terminal_map_to: string[] }>;
					loads: unknown[];
					shunts: unknown[];
					sources: Array<{
						terminal_map: string[];
						v_magnitude: number[];
						v_angle: number[];
					}>;
					extras: Record<string, unknown>;
				};
			};
		};
	};
	const network = document.value.data.network;
	for (const bus of network.buses) {
		bus.terminals = bus.terminals.slice(0, 3);
		bus.grounded = bus.grounded.filter((terminal) => bus.terminals.includes(terminal));
	}
	for (const line of network.lines) {
		line.terminal_map_from = line.terminal_map_from.slice(0, 3);
		line.terminal_map_to = line.terminal_map_to.slice(0, 3);
	}
	const matrixFields = ['r_series', 'x_series', 'g_from', 'b_from', 'g_to', 'b_to'];
	for (const code of network.linecodes) {
		code.n_conductors = 3;
		for (const field of matrixFields) {
			const matrix = code[field];
			if (Array.isArray(matrix)) {
				code[field] = matrix.slice(0, 3).map((row) => (Array.isArray(row) ? row.slice(0, 3) : row));
			}
		}
	}
	for (const source of network.sources) {
		source.terminal_map = source.terminal_map.slice(0, 3);
		source.v_magnitude = source.v_magnitude.slice(0, 3);
		source.v_angle = source.v_angle.slice(0, 3);
	}
	// Remove all injections and the explicit convention so this is a supported
	// three-wire network with no neutral reference to render.
	network.loads = [];
	network.shunts = [];
	const conventions = network.extras.bmopf_terminal_conventions;
	if (conventions && typeof conventions === 'object') {
		delete (conventions as { neutral?: unknown }).neutral;
	}
	return JSON.stringify(document);
}

function savedDistributionOptions(scope: Locator | Page) {
	return scope.getByLabel('Saved study').locator('option').filter({ hasText: 'distribution' });
}

function voltageModels(moduleText: string): string[] {
	const found: string[] = [];
	const visit = (value: unknown) => {
		if (Array.isArray(value)) {
			value.forEach(visit);
			return;
		}
		if (!value || typeof value !== 'object') return;
		const record = value as Record<string, unknown>;
		const model = record.voltage_model;
		if (
			model &&
			typeof model === 'object' &&
			typeof (model as { model?: unknown }).model === 'string'
		)
			found.push((model as { model: string }).model);
		Object.values(record).forEach(visit);
	};
	visit(JSON.parse(moduleText));
	return found.sort();
}

type LoadModelSpec = {
	p: number;
	q: number;
	model: string;
	terminal: string;
	alpha?: [number, number, number];
	beta?: [number, number, number];
	gamma?: [number, number];
};

const loadModelSpecs: Record<string, LoadModelSpec> = {
	ld1: { p: 12000, q: 4000, model: 'constant_current', terminal: 'a' },
	ld2: { p: 10000, q: 3000, model: 'constant_impedance', terminal: 'b' },
	ld3: {
		p: 15000,
		q: 5000,
		model: 'zip',
		terminal: 'c',
		alpha: [0.2, 0.3, 0.5],
		beta: [0.1, 0.2, 0.7]
	},
	ld4: { p: 8000, q: 3000, model: 'exponential', terminal: 'a', gamma: [1.5, 2] }
};

function expectedLoadPower(spec: LoadModelSpec, ratio: number) {
	// Above the default 1.15 pu ceiling, non-impedance models use the
	// boundary's constant-power equivalent admittance.
	if (spec.model !== 'constant_impedance' && ratio > 1.15)
		return { p: spec.p * (ratio / 1.15) ** 2, q: spec.q * (ratio / 1.15) ** 2 };
	if (spec.model === 'constant_current') return { p: spec.p * ratio, q: spec.q * ratio };
	if (spec.model === 'constant_impedance')
		return { p: spec.p * ratio ** 2, q: spec.q * ratio ** 2 };
	if (spec.model === 'zip') {
		const [az, ai, ap] = spec.alpha!;
		const [bz, bi, bp] = spec.beta!;
		return {
			p: spec.p * (az * ratio ** 2 + ai * ratio + ap),
			q: spec.q * (bz * ratio ** 2 + bi * ratio + bp)
		};
	}
	return { p: spec.p * ratio ** spec.gamma![0], q: spec.q * ratio ** spec.gamma![1] };
}

test('typed distribution case runs inside Studies and survives saved reopen', async ({ page }) => {
	test.setTimeout(120_000);
	await openApp(page);
	const headerFileInput = page.locator('header input[type="file"][accept*=".dss"]');
	await expect(headerFileInput).toHaveCount(1);
	await headerFileInput.setInputFiles(typedCase);
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible({
		timeout: 60_000
	});
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const study = page.getByRole('region', { name: 'Study workspace' });
	await expect(study.getByText('Distribution power flow', { exact: true })).toBeVisible();
	await expect(study.getByRole('button', { name: 'Run power flow' })).toBeEnabled({
		timeout: 60_000
	});
	await study.getByRole('button', { name: 'Run power flow' }).click();
	await expect(study.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
	await expect(study.getByRole('button', { name: 'Save result' })).toBeEnabled();
	await study.getByRole('button', { name: 'Save result' }).click();
	const downloadWait = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const exportPath = await (await downloadWait).path();
	if (!exportPath) throw new Error('saved distribution result has no export');
	const snapshot = JSON.parse(await readFile(exportPath, 'utf8'));
	const result = snapshot.result;
	expect(result.converged).toBe(true);
	expect(result.factorization_count).toBe(1);
	// The default voltage envelope changes this fixture's operating point.
	// Compare the rendered phasors to the saved result; the worker oracle test
	// independently checks the original load equations against OpenDSS.
	const source = result.source_reactions.reduce(
		(
			sum: { re: number; im: number },
			entry: { power_into_network: { re: number; im: number } }
		) => ({
			re: sum.re + entry.power_into_network.re,
			im: sum.im + entry.power_into_network.im
		}),
		{ re: 0, im: 0 }
	);
	await expect(
		study.locator('dt').filter({ hasText: 'Source P / Q' }).locator('..').locator('dd')
	).toHaveText(`${(source.re / 1000).toFixed(3)} / ${(source.im / 1000).toFixed(3)} kW / kvar`);
	await study.getByLabel('Bus result').selectOption('lb');
	await study.getByText('Voltages to neutral', { exact: true }).click();
	type Terminal = { bus: string; terminal: string; voltage: { re: number; im: number } };
	const terminals = (result.terminals as Terminal[]).filter((row) => row.bus === 'lb');
	expect(terminals.map((row) => row.terminal).sort()).toEqual(['a', 'b', 'c', 'n']);
	const neutral = terminals.find((row) => row.terminal === 'n')!.voltage;
	const formatted = (voltage: { re: number; im: number }) => [
		Math.hypot(voltage.re, voltage.im).toFixed(3),
		Math.hypot(voltage.re, voltage.im) === 0
			? '-'
			: ((Math.atan2(voltage.im, voltage.re) * 180) / Math.PI).toFixed(2)
	];
	for (const row of terminals) {
		const relative = { re: row.voltage.re - neutral.re, im: row.voltage.im - neutral.im };
		expect(
			(
				await study
					.getByRole('table', { name: 'Terminal results', exact: true })
					.locator('tbody tr')
					.filter({ hasText: new RegExp(`^${row.terminal}`) })
					.locator('td')
					.allTextContents()
			).slice(0, 3)
		).toEqual([row.terminal, ...formatted(row.voltage)]);
		await expect(
			study
				.getByRole('table', { name: 'Neutral voltage results', exact: true })
				.locator('tbody tr')
				.filter({ hasText: new RegExp(`^${row.terminal}`) })
				.locator('td')
		).toHaveText([row.terminal, ...formatted(relative)]);
	}
	await expect(savedDistributionOptions(page)).toHaveCount(1);
	const firstSavedValue = await savedDistributionOptions(page).nth(0).getAttribute('value');
	if (!firstSavedValue) throw new Error('first saved distribution option has no value');
	// Reload before creating the second case: ephemeral case counters restart,
	// so durable MC snapshot ids must still remain distinct.
	// Load the app afresh without the saved study's link, which would reopen it by itself.
	await openApp(page);
	await expect(page.locator('header input[type="file"][accept*=".dss"]')).toHaveCount(1);
	await page.locator('header input[type="file"][accept*=".dss"]').setInputFiles(typedCase);
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible({
		timeout: 60_000
	});
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const secondStudy = page.getByRole('region', { name: 'Study workspace' });
	await expect(secondStudy.getByRole('button', { name: 'Run power flow' })).toBeEnabled({
		timeout: 60_000
	});
	await secondStudy.getByRole('button', { name: 'Run power flow' }).click();
	await expect(secondStudy.getByText('Converged', { exact: true })).toBeVisible({
		timeout: 60_000
	});
	await secondStudy.getByRole('button', { name: 'Save result' }).click();
	await expect(savedDistributionOptions(page)).toHaveCount(2);
	const savedValues = await savedDistributionOptions(page).evaluateAll((options) =>
		options.map((option) => option.getAttribute('value'))
	);
	expect(new Set(savedValues).size).toBe(2);
	expect(savedValues).toContain(firstSavedValue);

	// Load the app afresh without the saved study's link, which would reopen it by itself.
	await openApp(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const reopened = page.getByRole('region', { name: 'Study workspace' });
	const reopenedSaved = page.getByLabel('Saved study');
	await expect(savedDistributionOptions(page)).toHaveCount(2);
	await reopenedSaved.selectOption(firstSavedValue);
	await expect(reopened.getByText('Distribution power flow', { exact: true })).toBeVisible();
	await expect(reopened.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
	await expect(reopened.getByText('Goal', { exact: true })).toHaveCount(0);
});

test('built-in four-wire demo solves and a three-wire case omits neutral-relative columns', async ({
	page
}) => {
	test.setTimeout(120_000);
	await openApp(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const demoStudy = page.getByRole('region', { name: 'Study workspace' });
	await demoStudy.getByRole('button', { name: 'Load 4-conductor example' }).click();
	await expect(demoStudy.getByText('Distribution power flow', { exact: true })).toBeVisible({
		timeout: 60_000
	});
	await expect(demoStudy.getByRole('button', { name: 'Run power flow' })).toBeEnabled({
		timeout: 60_000
	});
	await demoStudy.getByRole('button', { name: 'Run power flow' }).click();
	await expect(demoStudy.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });

	// Load the app afresh without the saved study's link, which would reopen it by itself.
	await openApp(page);
	const noNeutral = await noNeutralTypedCase();
	await page.locator('header input[type="file"][accept*=".dss"]').setInputFiles({
		name: 'three-wire-no-neutral.pio.json',
		mimeType: 'application/json',
		buffer: Buffer.from(noNeutral)
	});
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible({
		timeout: 60_000
	});
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const threeWireStudy = page.getByRole('region', { name: 'Study workspace' });
	await expect(threeWireStudy.getByRole('button', { name: 'Run power flow' })).toBeEnabled({
		timeout: 60_000
	});
	await threeWireStudy.getByRole('button', { name: 'Run power flow' }).click();
	await expect(threeWireStudy.getByText('Converged', { exact: true })).toBeVisible({
		timeout: 60_000
	});
	await threeWireStudy.getByLabel('Bus result').selectOption('lb');
	await expect(threeWireStudy.getByText('Voltages to neutral', { exact: true })).toHaveCount(0);
});

for (const nominalVoltage of [200, 240]) {
	test(`raw BMOPF load models solve and replay at ${nominalVoltage} V nominal`, async ({
		page
	}) => {
		test.setTimeout(180_000);
		const rawText = await readFile(loadModelsCase, 'utf8');
		const raw = JSON.parse(rawText) as {
			load: Record<string, Record<string, unknown>>;
		};
		expect(Object.values(raw.load).map((load) => load.model)).toEqual(
			expect.arrayContaining(['constant_current', 'constant_impedance', 'zip', 'exponential'])
		);
		for (const load of Object.values(raw.load)) load.v_nom = [nominalVoltage];
		const constantPower = structuredClone(raw);
		for (const load of Object.values(constantPower.load)) {
			load.model = 'constant_power';
			for (const field of [
				'alpha_z',
				'alpha_i',
				'alpha_p',
				'beta_z',
				'beta_i',
				'beta_p',
				'gamma_p',
				'gamma_q'
			])
				delete load[field];
		}

		await openApp(page);
		const headerFileInput = page.locator('header input[type="file"][accept*=".dss"]');
		await expect(headerFileInput).toHaveCount(1);
		await headerFileInput.setInputFiles({
			name: 'load-models.bmopf.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify(raw))
		});
		await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible({
			timeout: 60_000
		});
		await page.getByRole('button', { name: 'Studies', exact: true }).click();
		const study = page.getByRole('region', { name: 'Study workspace' });
		await expect(study.getByRole('button', { name: 'Run power flow' })).toBeEnabled({
			timeout: 60_000
		});
		await study.getByRole('button', { name: 'Run power flow' }).click();
		await expect(study.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
		const source = study
			.locator('dt')
			.filter({ hasText: 'Source P / Q' })
			.locator('..')
			.locator('dd');
		const loss = study
			.locator('dt')
			.filter({ hasText: 'Passive loss' })
			.locator('..')
			.locator('dd');
		const modelSource = (await source.textContent())?.trim();
		const modelLoss = (await loss.textContent())?.trim();
		if (!modelSource || !modelLoss) throw new Error('voltage-dependent result summary is missing');
		// This ideal-source fixture gives an independent analytical total, both
		// inside the normal voltage band and above the default 1.15 pu ceiling.
		const ratio = 239.6003617136947 / nominalVoltage;
		const expectedTotal = Object.values(loadModelSpecs).reduce(
			(sum, spec) => {
				const power = expectedLoadPower(spec, ratio);
				return { p: sum.p + power.p, q: sum.q + power.q };
			},
			{ p: 0, q: 0 }
		);
		await expect(source).toHaveText(
			`${(expectedTotal.p / 1000).toFixed(3)} / ${(expectedTotal.q / 1000).toFixed(3)} kW / kvar`
		);
		await study.getByRole('button', { name: 'Save result' }).click();
		await expect(savedDistributionOptions(page)).toHaveCount(1);
		const downloadWait = page.waitForEvent('download');
		await page.getByRole('button', { name: 'Export', exact: true }).click();
		const download = await downloadWait;
		const exportPath = await download.path();
		if (!exportPath) throw new Error('distribution snapshot export has no local path');
		const snapshot = JSON.parse(await readFile(exportPath, 'utf8')) as {
			input_module: string;
			solution_module: string | null;
			result: {
				terminals: Array<{ bus: string; terminal: string; voltage: { re: number; im: number } }>;
				element_ports: Array<{
					element: string;
					kind: string;
					terminal: string;
					current_into_element: { re: number; im: number };
					power_into_element: { re: number; im: number };
				}>;
			};
		};
		const expectedModels = ['constant_current', 'constant_impedance', 'exponential', 'zip'];
		expect(voltageModels(snapshot.input_module)).toEqual(expectedModels);
		if (nominalVoltage === 240) {
			expect(snapshot.solution_module).not.toBeNull();
			expect(voltageModels(snapshot.solution_module!)).toEqual(expectedModels);
		} else {
			expect(snapshot.solution_module).toBeNull();
		}
		for (const [element, spec] of Object.entries(loadModelSpecs)) {
			const port = snapshot.result.element_ports.find(
				(candidate) =>
					candidate.element === element &&
					candidate.kind === 'load' &&
					candidate.terminal === spec.terminal
			);
			if (!port) throw new Error(`exported snapshot has no phase port for ${element}`);
			const phase = snapshot.result.terminals.find(
				(candidate) => candidate.bus === 'src' && candidate.terminal === spec.terminal
			);
			const neutral = snapshot.result.terminals.find(
				(candidate) => candidate.bus === 'src' && candidate.terminal === 'n'
			);
			if (!phase || !neutral)
				throw new Error(`exported snapshot has no branch voltage for ${element}`);
			const voltage = {
				re: phase.voltage.re - neutral.voltage.re,
				im: phase.voltage.im - neutral.voltage.im
			};
			const ratio = Math.hypot(voltage.re, voltage.im) / nominalVoltage;
			const expected = expectedLoadPower(spec, ratio);
			const denominator = voltage.re ** 2 + voltage.im ** 2;
			const quotient = {
				re: (expected.p * voltage.re + expected.q * voltage.im) / denominator,
				im: (expected.q * voltage.re - expected.p * voltage.im) / denominator
			};
			expect(port.power_into_element.re).toBeCloseTo(expected.p, 6);
			expect(port.power_into_element.im).toBeCloseTo(expected.q, 6);
			expect(port.current_into_element.re).toBeCloseTo(quotient.re, 6);
			expect(port.current_into_element.im).toBeCloseTo(-quotient.im, 6);
		}
		await page.locator('label.file-button input[type="file"]').setInputFiles({
			name: 'exported-mc-pf-study.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify(snapshot))
		});
		await expect(study.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
		await expect(
			study.locator('dt').filter({ hasText: 'Source P / Q' }).locator('..').locator('dd')
		).toHaveText(modelSource);
		await expect(
			study.locator('dt').filter({ hasText: 'Passive loss' }).locator('..').locator('dd')
		).toHaveText(modelLoss);

		// Load the app afresh without the saved study's link, which would reopen it by itself.
		await openApp(page);
		await page.getByRole('button', { name: 'Studies', exact: true }).click();
		const reopened = page.getByRole('region', { name: 'Study workspace' });
		await page.getByLabel('Saved study').selectOption({ index: 1 });
		await expect(reopened.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
		await expect(
			reopened.locator('dt').filter({ hasText: 'Source P / Q' }).locator('..').locator('dd')
		).toHaveText(modelSource);
		await expect(
			reopened.locator('dt').filter({ hasText: 'Passive loss' }).locator('..').locator('dd')
		).toHaveText(modelLoss);

		// Load the app afresh without the saved study's link, which would reopen it by itself.
		await openApp(page);
		await page.locator('header input[type="file"][accept*=".dss"]').setInputFiles({
			name: 'mc-pf-load-models-constant-power.bmopf.json',
			mimeType: 'application/json',
			buffer: Buffer.from(JSON.stringify(constantPower))
		});
		await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible({
			timeout: 60_000
		});
		await page.getByRole('button', { name: 'Studies', exact: true }).click();
		const baseline = page.getByRole('region', { name: 'Study workspace' });
		await baseline.getByRole('button', { name: 'Run power flow' }).click();
		await expect(baseline.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
		const baselineSource = (
			await baseline
				.locator('dt')
				.filter({ hasText: 'Source P / Q' })
				.locator('..')
				.locator('dd')
				.textContent()
		)?.trim();
		if (!baselineSource) throw new Error('constant-power baseline summary is missing');
		const baselineRatio = 239.6003617136947 / nominalVoltage;
		const baselineScale = baselineRatio > 1.15 ? (baselineRatio / 1.15) ** 2 : 1;
		expect(baselineSource).toBe(
			`${(45 * baselineScale).toFixed(3)} / ${(15 * baselineScale).toFixed(3)} kW / kvar`
		);
	});
}

test('settings reruns qualify modified results and preserve provenance through export and reopen', async ({
	page
}, testInfo) => {
	test.setTimeout(120_000);
	await openApp(page);
	await page.locator('header input[type="file"][accept*=".dss"]').setInputFiles(typedCase);
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const study = page.getByRole('region', { name: 'Study workspace' });
	await study.getByText('Power flow settings', { exact: true }).click();
	await expect(study.getByLabel('Vmin (pu)', { exact: true })).toHaveValue('0.85');
	await expect(study.getByLabel('Vmax (pu)', { exact: true })).toHaveValue('1.15');
	await study.getByLabel('Vmin (pu)', { exact: true }).fill('0.4');
	await expect(study.getByRole('button', { name: 'Run power flow', exact: true })).toBeDisabled();
	await expect(study.getByRole('alert').filter({ hasText: 'Vlow' })).toBeVisible();
	// Put every non-impedance load outside the normal band without changing the fixture.
	await study.getByLabel('Vmin (pu)', { exact: true }).fill('1.01');
	await study.getByLabel('Maximum iterations', { exact: true }).fill('200');
	await study.getByRole('button', { name: 'Run power flow', exact: true }).click();
	const results = study.getByRole('region', { name: 'AC power flow results' });
	await expect(
		results.getByText('Modified load model — non-canonical result', { exact: true })
	).toBeVisible();
	await expect(results.getByText('Converged', { exact: true })).toBeVisible();
	await expect(
		study
			.getByRole('region', { name: 'Operating point overview' })
			.getByText('Violations', { exact: true })
	).toBeInViewport();
	await study.getByRole('button', { name: 'Save result' }).click();
	const downloadWait = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const exportedPath = await (await downloadWait).path();
	if (!exportedPath) throw new Error('missing exported snapshot');
	const saved = JSON.parse(await readFile(exportedPath, 'utf8'));
	expect(saved.version).toBe(2);
	expect(saved.solution_module).toBeNull();
	expect(saved.result.load_model.status).toBe('modified');
	expect(saved.result.load_model.modified_branch_count).toBeGreaterThan(0);
	expect(saved.result.load_model_changes.length).toBe(
		saved.result.load_model.modified_branch_count
	);
	expect(saved.options.v_min_pu).toBe(1.01);
	expect(saved.result.load_model.options).toEqual(saved.options);
	const csvWait = page.waitForEvent('download');
	await study.getByRole('button', { name: 'Bus CSV', exact: true }).click();
	const csvPath = await (await csvWait).path();
	if (!csvPath) throw new Error('missing CSV export');
	const csv = await readFile(csvPath, 'utf8');
	expect(csv).toContain('Modified load model');
	expect(csv).toContain('v_min_pu');
	expect(csv).toContain('1.01');

	await study.getByText('Power flow settings', { exact: true }).scrollIntoViewIfNeeded();
	await page.screenshot({
		path: testInfo.outputPath('modified-model-settings.png'),
		fullPage: true
	});

	// A true numerical failure keeps the previous, visibly qualified operating point.
	await study.getByLabel('Maximum iterations', { exact: true }).fill('1');
	await study.getByRole('button', { name: 'Run power flow', exact: true }).click();
	await expect(study.getByRole('alert').filter({ hasText: 'did not converge' })).toBeVisible();
	await expect(
		results.getByText('Modified load model — non-canonical result', { exact: true })
	).toBeVisible();

	await openApp(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Saved study').selectOption(`mc:${saved.id}`);
	await expect(
		results.getByText('Modified load model — non-canonical result', { exact: true })
	).toBeVisible();
	await study.getByText('Power flow settings', { exact: true }).click();
	await expect(study.getByLabel('Vmin (pu)', { exact: true })).toHaveValue('1.01');
	await expect(study.getByLabel('Maximum iterations', { exact: true })).toHaveValue('200');
	await study.getByRole('button', { name: 'Run power flow', exact: true }).click();
	await expect(study.getByRole('button', { name: 'Run power flow', exact: true })).toBeEnabled();
	await expect(
		results.getByText('Modified load model — non-canonical result', { exact: true })
	).toBeVisible();

	await page.setViewportSize({ width: 390, height: 844 });
	await study.getByText('Power flow settings', { exact: true }).click();
	await expect(study.getByRole('button', { name: 'Run power flow', exact: true })).toBeVisible();
	await study.getByText('Power flow settings', { exact: true }).scrollIntoViewIfNeeded();
	await page.screenshot({
		path: testInfo.outputPath('modified-model-settings-mobile.png'),
		fullPage: true
	});
});

test('energized-network selection retains excluded islands through solve, CSV and saved reopen', async ({
	page
}) => {
	test.setTimeout(120_000);
	const fixture = JSON.parse(await readFile(loadModelsCase, 'utf8'));
	fixture.name = 'Explicit island selection';
	fixture.bus.unpowered = structuredClone(fixture.bus.src);
	fixture.load.unsupplied = { ...structuredClone(fixture.load.ld1), bus: 'unpowered' };
	await openApp(page);
	await page.locator('header input[type="file"][accept*=".dss"]').setInputFiles({
		name: 'islands.bmopf.json',
		mimeType: 'application/json',
		buffer: Buffer.from(JSON.stringify(fixture))
	});
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'Solve energized network', exact: true })
	).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Run power flow', exact: true })).toHaveCount(0);
	await expect(page.getByText('Unpowered islands detected', { exact: true })).toBeVisible();
	await page.getByText('Islands and equipment to exclude', { exact: true }).click();
	await expect(page.getByText('load: unsupplied', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const study = page.getByRole('region', { name: 'Study workspace' });
	await expect(study.getByRole('button', { name: 'Run power flow', exact: true })).toHaveCount(0);
	await expect(
		study.getByRole('button', { name: 'Solve energized network', exact: true })
	).toHaveCount(1);
	await expect(study.getByText('Unpowered islands detected', { exact: true })).toBeVisible();
	await study.getByText('Islands and equipment to exclude', { exact: true }).click();
	await expect(study.getByText('load: unsupplied', { exact: true })).toBeVisible();
	await expect(study.getByRole('button', { name: 'Save result', exact: true })).toBeDisabled();
	await study.getByText('Power flow settings', { exact: true }).click();
	await expect(study.getByLabel('Maximum iterations', { exact: true })).toHaveValue('100');
	await study.getByLabel('Maximum iterations', { exact: true }).fill('0');
	await expect(
		study.getByRole('button', { name: 'Solve energized network', exact: true })
	).toBeDisabled();
	await expect(study.getByRole('button', { name: 'Apply settings and run' })).toHaveCount(0);
	await study.getByLabel('Maximum iterations', { exact: true }).fill('500');
	await study.getByText('Advanced tolerances', { exact: true }).click();
	await expect(study.getByLabel('Voltage-change tolerance (V)', { exact: true })).toHaveValue(
		'0.0001'
	);
	await expect(study.getByLabel('Absolute KCL tolerance (A)', { exact: true })).toHaveValue(
		'0.0001'
	);
	await expect(study.getByLabel('Relative KCL tolerance', { exact: true })).toHaveValue('0.00001');
	await study.getByLabel('Voltage-change tolerance (V)', { exact: true }).fill('1e-7');
	await study.getByLabel('Absolute KCL tolerance (A)', { exact: true }).fill('1e-5');
	// The one solve action must use the shared settings.
	await study.getByRole('button', { name: 'Solve energized network', exact: true }).click();
	await expect(study.getByText('Converged', { exact: true })).toBeVisible();
	await expect(
		study
			.getByRole('region', { name: 'Operating point overview' })
			.getByText('Violations', { exact: true })
	).toBeInViewport();
	await expect(
		study.getByText('1 load has no voltage source and is absent from solved totals.').first()
	).toBeVisible();
	const results = study.getByRole('region', { name: 'AC power flow results' });
	await results.getByLabel('Bus result').selectOption('unpowered');
	await expect(results.getByText('0 terminals at bus unpowered', { exact: true })).toBeVisible();
	await expect(
		results.getByRole('table', { name: 'Terminal results', exact: true }).locator('tbody tr')
	).toHaveCount(0);
	await study.getByRole('button', { name: 'Save result', exact: true }).click();
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const path = await (await download).path();
	if (!path) throw new Error('missing study export');
	const snapshot = JSON.parse(await readFile(path, 'utf8'));
	expect(snapshot.options.tolerance).toBe(1e-7);
	expect(snapshot.options.absolute_kcl_tolerance).toBe(1e-5);
	expect(snapshot.result.load_model.options).toEqual(snapshot.options);
	expect(snapshot.options.max_iterations).toBe(500);
	expect(snapshot.result.network_scope).toEqual({
		excluded_island_count: 1,
		excluded_bus_count: 1,
		excluded_equipment_count: 1,
		excluded_load_count: 1
	});
	expect(snapshot.result.excluded_islands).toEqual([
		{ buses: ['unpowered'], equipment: [{ kind: 'load', name: 'unsupplied' }] }
	]);
	expect(snapshot.input_module).toContain('unsupplied');
	expect(snapshot.input_module).toContain('tellegen.energized_network');
	expect(snapshot.result.terminals.every((t: { bus: string }) => t.bus !== 'unpowered')).toBe(true);
	const csvWait = page.waitForEvent('download');
	await study.getByRole('button', { name: 'Bus CSV', exact: true }).click();
	const csvPath = await (await csvWait).path();
	if (!csvPath) throw new Error('missing CSV export');
	const csv = await readFile(csvPath, 'utf8');
	expect(csv).toContain('Energized network only');
	expect(csv).toContain('excluded_islands');
	expect(csv).toContain('unsupplied');
	const savedId = await savedDistributionOptions(page).first().getAttribute('value');
	await openApp(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Saved study').selectOption(savedId!);
	await expect(study.getByText('Converged', { exact: true })).toBeVisible();
	await expect(results.getByLabel('Bus result').locator('option[value="unpowered"]')).toHaveCount(
		1
	);
	await study.getByRole('button', { name: 'Solve energized network', exact: true }).click();
	await expect(
		study.getByRole('button', { name: 'Solve energized network', exact: true })
	).toBeEnabled();
	await expect(study.getByText('Converged', { exact: true })).toBeVisible();
	await results.getByText('Excluded islands and equipment', { exact: true }).click();
	await expect(results.getByText('load: unsupplied', { exact: true })).toBeVisible();
	await page.screenshot({ path: 'test-results/mc-energized-network.png', fullPage: true });
});

test('a case without voltage sources remains inspectable with solving disabled', async ({
	page
}) => {
	await openApp(page);
	const fixture = JSON.parse(await readFile(loadModelsCase, 'utf8'));
	fixture.voltage_source = {};
	await page.locator('header input[type="file"][accept*=".dss"]').setInputFiles({
		name: 'no-sources.bmopf.json',
		mimeType: 'application/json',
		buffer: Buffer.from(JSON.stringify(fixture))
	});
	await expect(page.getByText('Multiconductor', { exact: true }).first()).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'Solve energized network', exact: true })
	).toBeDisabled();
	await expect(page.getByText(/No energized network: no bus island/)).toBeVisible();
	await page.getByText('Islands and equipment to exclude', { exact: true }).click();
	await expect(page.getByText('load: ld1', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const study = page.getByRole('region', { name: 'Study workspace' });
	await expect(
		study.getByRole('button', { name: 'Solve energized network', exact: true })
	).toBeDisabled();
	await expect(study.getByRole('button', { name: 'Run power flow', exact: true })).toHaveCount(0);
	await expect(study.getByText(/No energized network: no bus island/)).toBeVisible();
});
