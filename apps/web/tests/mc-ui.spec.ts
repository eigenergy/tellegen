import { readFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';
import { installWebMcpHarness, callTool } from './fixtures/planning-case.js';
import { noticeDetails } from './fixtures/notices.js';

const fixture = new URL(
	'./fixtures/mc-pf-browser/public/mc-pf-three-phase.bmopf.json',
	import.meta.url
);
function coordinates(space: 'diagram' | 'geographic') {
	const points =
		space === 'diagram'
			? [
					[1000, 2000],
					[1050, 2020],
					[1100, 2050]
				]
			: [
					[-83, 35],
					[-82.95, 35.02],
					[-82.9, 35.05]
				];
	return JSON.stringify({
		type: 'FeatureCollection',
		powerio_geo: { space },
		features: [
			{
				type: 'Feature',
				properties: { bus: 'src' },
				geometry: { type: 'Point', coordinates: points[0] }
			},
			{
				type: 'Feature',
				properties: { bus: 'lb' },
				geometry: { type: 'Point', coordinates: points[2] }
			},
			{
				type: 'Feature',
				properties: { bus_from: 'src', bus_to: 'lb' },
				geometry: { type: 'LineString', coordinates: points }
			}
		]
	});
}
test('multiconductor UI solves, exposes terminal values to agents, and retains results after coordinate attachment', async ({
	page
}, testInfo) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles([
		{ name: 'private-feeder.json', mimeType: 'application/json', buffer: await readFile(fixture) },
		{
			name: 'private-drawing.geojson',
			mimeType: 'application/json',
			buffer: Buffer.from(coordinates('diagram'))
		}
	]);
	await expect(page.getByRole('button', { name: 'Solve AC power flow', exact: true })).toBeEnabled({
		timeout: 60_000
	});
	await expect(
		page.getByRole('application', { name: 'Network diagram', exact: true })
	).toBeVisible();
	await expect(page.locator('.maplibregl-ctrl-attrib')).toHaveCount(0);
	await expect(page.locator('[data-bus-id="0"] circle')).toHaveAttribute('cx', /1000|1100/);
	const path = page.locator('[data-branch-id="0"]');
	expect((await path.getAttribute('d'))?.split(' L')).toHaveLength(3);
	await page.getByRole('button', { name: 'Solve AC power flow', exact: true }).click();
	const table = page.getByRole('table', { name: 'Terminal results', exact: true });
	await expect(table).toBeVisible({ timeout: 60_000 });
	await expect(table.locator('tbody tr')).toHaveCount(4);
	await expect(table.getByRole('columnheader', { name: 'To ground V' })).toBeVisible();
	await expect(table.getByRole('columnheader', { name: 'Net current A' })).toBeVisible();
	const before = await table.innerText();
	const inspected = await callTool(page, 'inspect_case', {});
	expect(inspected.ok).toBe(true);
	if (!inspected.ok) throw new Error(inspected.error.message);
	const caseId = String(inspected.data.case_id);
	const solved = await callTool(page, 'solve_multiconductor_pf', {
		case_id: caseId,
		expected_revision: inspected.data.revision
	});
	expect(solved).toMatchObject({ ok: true, data: { converged: true, terminal_count: 8 } });
	const query = await callTool(page, 'query_network', {
		case_id: caseId,
		element_kind: 'bus',
		sort_by: 'voltage_v',
		limit: 10
	});
	expect(query.ok).toBe(true);
	if (!query.ok) throw new Error(query.error.message);
	expect(JSON.stringify(query.data)).toContain('terminal_values');
	expect(JSON.stringify(query.data)).toContain('current_a');
	expect(query.data.units).toMatchObject({ lmp: null });
	await page.getByText('Calculation details', { exact: true }).click();
	await expect(page.getByRole('region', { name: 'AC power flow results' })).toContainText(
		'Maximum KCL residual'
	);
	await page.screenshot({ path: testInfo.outputPath('mc-results-desktop.png') });
	await page.locator('header input[type=file]').setInputFiles({
		name: 'private-map.geojson',
		mimeType: 'application/json',
		buffer: Buffer.from(coordinates('geographic'))
	});
	await expect(page.getByRole('application', { name: 'Network diagram', exact: true })).toHaveCount(
		0
	);
	await expect.poll(() => table.innerText()).toBe(before);
	await expect(await noticeDetails(page)).toContainText(
		'2 buses, 1 routes matched; 0 unmatched objects'
	);
	await page
		.getByRole('region', { name: 'Notifications' })
		.getByRole('button', { name: 'Details', exact: true })
		.click();
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.getByRole('button', { name: 'Run power flow', exact: true })).toBeVisible();
	await page.screenshot({ path: testInfo.outputPath('mc-results-mobile.png') });
	await page.getByRole('button', { name: 'Save result', exact: true }).click();
	await expect(page.getByLabel('Saved study')).not.toHaveValue('');
	const downloadPromise = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const stream = await (await downloadPromise).createReadStream();
	if (!stream) throw new Error('Export returned no data');
	const chunks: Buffer[] = [];
	for await (const chunk of stream) chunks.push(Buffer.from(chunk));
	const exported = Buffer.concat(chunks);
	const snapshot = JSON.parse(exported.toString());
	expect(snapshot.schema).toBe('tellegen-mc-pf-study');
	expect(snapshot.result.terminals).toHaveLength(8);
	expect(snapshot.input_module).toContain('-82.9');
	expect(snapshot.solution_module).toContain('-82.9');
	const fresh = await page.context().browser()!.newContext();
	try {
		const imported = await fresh.newPage();
		await imported.route('**/api/cases', (route) => route.fulfill({ json: [] }));
		await imported.goto(new URL('/', page.url()).href);
		await imported.getByRole('button', { name: 'Studies', exact: true }).click();
		await imported
			.getByText('Import', { exact: true })
			.locator('input')
			.setInputFiles({ name: 'saved.json', mimeType: 'application/json', buffer: exported });
		const resultTable = imported.getByRole('table', { name: 'Terminal results', exact: true });
		await expect(resultTable).toBeVisible({ timeout: 60_000 });
		await expect.poll(() => resultTable.innerText()).toBe(before);
		await expect(imported.getByRole('button', { name: 'Save result', exact: true })).toBeDisabled();
		await expect(
			imported.getByRole('application', { name: 'Network diagram', exact: true })
		).toHaveCount(0);
		await imported
			.getByText('Import', { exact: true })
			.locator('input')
			.setInputFiles({ name: 'same.json', mimeType: 'application/json', buffer: exported });
		await expect(await noticeDetails(imported)).toContainText('This study is already saved');
		await expect(imported.getByLabel('Saved study').locator('option[value^="mc:"]')).toHaveCount(1);
	} finally {
		await fresh.close();
	}
});
