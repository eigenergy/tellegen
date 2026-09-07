import { readFile } from 'node:fs/promises';
import type { Locator, Page } from '@playwright/test';

import { expect, test } from './fixtures/page-errors.js';

const typedCase = 'tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json';

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
