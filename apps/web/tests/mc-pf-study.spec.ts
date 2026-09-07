import { readFile } from 'node:fs/promises';
import type { Locator, Page } from '@playwright/test';

import { expect, test } from './fixtures/page-errors.js';

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
	// A network without a declared neutral omits neutral-relative voltages.
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
	await page.goto('/');
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
	await expect(study.getByText(/49\.825 \/ 16\.508 kW \/ kvar/)).toBeVisible();
	await expect(study.getByText(/4\.825 kW/)).toBeVisible();
	await study.getByLabel('Bus result').selectOption('lb');
	await study.getByText('Voltages to neutral', { exact: true }).click();
	const terminalRow = (terminal: string) =>
		study
			.getByRole('table', { name: 'Terminal results', exact: true })
			.locator('tbody tr')
			.filter({ hasText: new RegExp(`^${terminal}`) });
	const neutralRow = (terminal: string) =>
		study
			.getByRole('table', { name: 'Neutral voltage results', exact: true })
			.locator('tbody tr')
			.filter({ hasText: new RegExp(`^${terminal}`) });
	for (const [terminal, ground, angle, neutral, relativeAngle] of [
		['a', '221.762', '0.31', '222.333', '-2.76'],
		['b', '228.616', '-120.10', '238.828', '-118.60'],
		['c', '213.351', '120.04', '202.976', '121.64'],
		['n', '11.898', '91.53', '0.000', '-']
	]) {
		expect((await terminalRow(terminal).locator('td').allTextContents()).slice(0, 3)).toEqual([
			terminal,
			ground,
			angle
		]);
		await expect(neutralRow(terminal).locator('td')).toHaveText([terminal, neutral, relativeAngle]);
	}
	await expect(study.getByRole('button', { name: 'Save result' })).toBeEnabled();
	await study.getByRole('button', { name: 'Save result' }).click();
	await expect(savedDistributionOptions(page)).toHaveCount(1);
	const firstSavedValue = await savedDistributionOptions(page).nth(0).getAttribute('value');
	if (!firstSavedValue) throw new Error('first saved distribution option has no value');
	// Durable snapshot IDs remain distinct across browser reloads.
	await page.reload();
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

	await page.reload();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const reopened = page.getByRole('region', { name: 'Study workspace' });
	const reopenedSaved = page.getByLabel('Saved study');
	await expect(savedDistributionOptions(page)).toHaveCount(2);
	await reopenedSaved.selectOption(firstSavedValue);
	await expect(reopened.getByText('Distribution power flow', { exact: true })).toBeVisible();
	await expect(reopened.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
	await expect(reopened.getByText('Goal', { exact: true })).toHaveCount(0);
});

test('built-in 4-conductor example solves and a 3-conductor case omits neutral-relative columns', async ({
	page
}) => {
	test.setTimeout(120_000);
	await page.goto('/');
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

	await page.reload();
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

test('raw BMOPF voltage-dependent loads solve and replay with their models', async ({ page }) => {
	test.setTimeout(180_000);
	const rawText = await readFile(loadModelsCase, 'utf8');
	const raw = JSON.parse(rawText) as {
		load: Record<string, Record<string, unknown>>;
	};
	expect(Object.values(raw.load).map((load) => load.model)).toEqual(
		expect.arrayContaining(['constant_current', 'constant_impedance', 'zip', 'exponential'])
	);
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

	await page.goto('/');
	const headerFileInput = page.locator('header input[type="file"][accept*=".dss"]');
	await expect(headerFileInput).toHaveCount(1);
	await headerFileInput.setInputFiles(loadModelsCase);
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
	const loss = study.locator('dt').filter({ hasText: 'Network loss' }).locator('..').locator('dd');
	const modelSource = (await source.textContent())?.trim();
	const modelLoss = (await loss.textContent())?.trim();
	if (!modelSource || !modelLoss) throw new Error('voltage-dependent result summary is missing');
	// The source is ideal and all four loads share its bus. At |V| =
	// 239.6003617136947 V and Vnom = 200 V, the four model laws consume
	// 56.414755 kW and 18.818863 kvar in aggregate.
	await expect(source).toHaveText('56.415 / 18.819 kW / kvar');
	await study.getByRole('button', { name: 'Save result' }).click();
	await expect(savedDistributionOptions(page)).toHaveCount(1);
	const downloadWait = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const download = await downloadWait;
	const exportPath = await download.path();
	if (!exportPath) throw new Error('distribution snapshot export has no local path');
	const snapshot = JSON.parse(await readFile(exportPath, 'utf8')) as {
		input_module: string;
		solution_module: string;
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
	expect(voltageModels(snapshot.solution_module)).toEqual(expectedModels);
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
		const ratio = Math.hypot(voltage.re, voltage.im) / 200;
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
		study.locator('dt').filter({ hasText: 'Network loss' }).locator('..').locator('dd')
	).toHaveText(modelLoss);

	await page.reload();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const reopened = page.getByRole('region', { name: 'Study workspace' });
	await page.getByLabel('Saved study').selectOption({ index: 1 });
	await expect(reopened.getByText('Converged', { exact: true })).toBeVisible({ timeout: 60_000 });
	await expect(
		reopened.locator('dt').filter({ hasText: 'Source P / Q' }).locator('..').locator('dd')
	).toHaveText(modelSource);
	await expect(
		reopened.locator('dt').filter({ hasText: 'Network loss' }).locator('..').locator('dd')
	).toHaveText(modelLoss);

	await page.reload();
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
	expect(baselineSource).toBe('45.000 / 15.000 kW / kvar');
});
