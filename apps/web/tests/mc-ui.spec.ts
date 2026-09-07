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
	await expect(table.locator('tbody tr')).toHaveCount(8);
	await expect(table).toContainText('Voltage (V)');
	await expect(table).toContainText('Net current (A)');
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
	await page.locator('input[type=file]').setInputFiles({
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
	await expect(
		page.getByRole('button', { name: 'Solve AC power flow', exact: true })
	).toBeVisible();
	await page.screenshot({ path: testInfo.outputPath('mc-results-mobile.png') });
});
