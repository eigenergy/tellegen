import { readFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';
import {
	CASE3_COORDS,
	CASE3_PLANNING,
	callTool,
	congestCase,
	installWebMcpHarness
} from './fixtures/planning-case.js';

test.beforeEach(async ({ request }) => {
	const expectedVersion = JSON.parse(
		await readFile(new URL('../build/_app/version.json', import.meta.url), 'utf8')
	);
	const servedVersion = await request.get('/_app/version.json');
	expect(await servedVersion.json(), 'Preview must serve the current isolated build').toEqual(
		expectedVersion
	);
	const currentHtml = await readFile(new URL('../build/index.html', import.meta.url), 'utf8');
	const servedHtml = await (await request.get('/')).text();
	const entries = (html: string) =>
		[...html.matchAll(/_app\/immutable\/entry\/(?:start|app)\.[A-Za-z0-9_-]+\.js/g)]
			.map((match) => match[0])
			.sort();
	expect(entries(servedHtml), 'Restart the preview after rebuilding its assets').toEqual(
		entries(currentHtml)
	);
});

test('WebMCP switches cases without cursor input and keeps saved work', async ({ page }) => {
	test.setTimeout(180_000);
	await installWebMcpHarness(page);
	const first = await congestCase(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Study name').fill('Saved operating point');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Saved operating point' })).toBeVisible();

	const baseline = await callTool(page, 'inspect_case', {});
	expect(baseline.ok, JSON.stringify(baseline)).toBe(true);
	if (!baseline.ok) throw new Error(baseline.error.message);
	const edited = await callTool(page, 'edit_demand', {
		study_id: baseline.data.study_id,
		expected_revision: baseline.data.study_revision,
		operation: {
			kind: 'edit_demand',
			state: baseline.data.state_id,
			goal: null,
			changes: [{ bus: 2, delta_mw: 5 }],
			rationale: 'Compare a 5 MW demand increase at bus 2'
		}
	});
	expect(edited.ok, JSON.stringify(edited)).toBe(true);
	const demand = await callTool(page, 'query_network', {
		case_id: first.caseId,
		element_kind: 'bus',
		element_ids: ['2'],
		limit: 1
	});
	expect(demand).toMatchObject({
		ok: true,
		data: { elements: [{ legacy_id: 2, demand_mw: 95, base_demand_mw: 90 }] }
	});

	const displayed = await callTool(page, 'inspect_case', {});
	expect(displayed.ok, JSON.stringify(displayed)).toBe(true);
	if (!displayed.ok) throw new Error(displayed.error.message);
	expect(displayed.data.state_id).toEqual(expect.any(String));
	const prices = await callTool(page, 'query_network', {
		case_id: first.caseId,
		element_kind: 'bus',
		sort_by: 'price',
		direction: 'desc',
		limit: 1
	});
	expect(prices.ok, JSON.stringify(prices)).toBe(true);
	if (!prices.ok) throw new Error(prices.error.message);
	expect(prices.data.state_id).toBe(displayed.data.state_id);
	const bus = (
		prices.data.elements as Array<{
			element_id: string;
			legacy_id: number;
			demand_mw: number;
			price: number;
		}>
	)[0];
	expect(
		await callTool(page, 'focus_network', {
			case_id: first.caseId,
			target: { kind: 'bus', element_id: bus.element_id }
		})
	).toMatchObject({
		ok: true,
		data: {
			state_id: displayed.data.state_id,
			focused: { element_id: bus.element_id },
			sensitivity_loaded: false
		}
	});
	const details = page.locator('section[aria-label="Saved network details"]');
	await expect(
		details.getByRole('heading', { name: new RegExp(`^Bus ${bus.legacy_id}`) })
	).toBeVisible();
	const demandRow = details
		.locator('dl > div')
		.filter({ has: page.locator('dt', { hasText: 'Current demand' }) });
	await expect(demandRow.locator('dd')).toHaveText(`${bus.demand_mw} MW`);
	const lmpRow = details
		.locator('dl > div')
		.filter({ has: page.locator('dt', { hasText: /^LMP$/ }) });
	const shownPrice = Number.parseFloat(
		(await lmpRow.locator('dd').innerText()).replaceAll(',', '')
	);
	expect(shownPrice).toBeCloseTo(bus.price, 1);
	await expect(page.locator('input[type="range"]')).toHaveCount(0);
	const listed = await callTool(page, 'list_cases', {});
	expect(listed).toMatchObject({ ok: true, data: { cases: [{ case_id: first.caseId }] } });
	const selected = await callTool(page, 'select_case', { case_id: first.caseId });
	expect(selected).toMatchObject({
		ok: true,
		data: { case_id: first.caseId, state_id: null, selected: true }
	});

	expect(
		await callTool(page, 'query_network', {
			case_id: first.caseId,
			element_kind: 'bus',
			element_ids: ['2'],
			limit: 1
		})
	).toMatchObject({
		ok: true,
		data: { state_id: null, elements: [{ legacy_id: 2, demand_mw: 90 }] }
	});

	await page.locator('input[type="file"][accept*=".m,"]').setInputFiles([
		{ name: 'second-coords.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE3_COORDS) },
		{
			name: 'second.m',
			mimeType: 'text/plain',
			buffer: Buffer.from(
				CASE3_PLANNING.replace('case3test', 'second').replace('2 1 90 30', '2 1 70 30')
			)
		}
	]);
	await expect
		.poll(async () => {
			const result = await callTool(page, 'list_cases', {});
			return result.ok ? result.data.total : 0;
		})
		.toBe(2);
	const cases = await callTool(page, 'list_cases', {});
	if (!cases.ok) throw new Error(cases.error.message);
	const second = (cases.data.cases as Array<{ case_id: string }>).find(
		(c) => c.case_id !== first.caseId
	)!;
	expect(await callTool(page, 'select_case', { case_id: second.case_id })).toMatchObject({
		ok: true,
		data: { case_id: second.case_id }
	});
	expect(await callTool(page, 'select_case', { case_id: first.caseId })).toMatchObject({
		ok: true,
		data: { case_id: first.caseId }
	});
	await expect(
		page.getByLabel('Saved study').locator('option', { hasText: 'Saved operating point' })
	).toHaveCount(1);
});

test('unavailable configured cases remain visible and cannot replace the displayed case', async ({
	page
}) => {
	await installWebMcpHarness(page);
	const unavailable = {
		id: 'unavailable-demo',
		name: 'Unavailable demo',
		n_bus: 0,
		n_branch: 0,
		n_gen: 0,
		unavailable_reason: 'Case data is missing'
	};
	await page.route('**/api/cases', (route) => route.fulfill({ json: [unavailable] }));
	await page.goto('/');
	await expect(
		page.getByRole('button', { name: 'Unavailable demo Unavailable', exact: true })
	).toBeDisabled();
	await expect(
		page.getByRole('button', { name: 'Unavailable demo Unavailable', exact: true })
	).toHaveAttribute('title', unavailable.unavailable_reason);
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
	const catalogue = await callTool(page, 'list_cases', {});
	expect(catalogue).toMatchObject({
		ok: true,
		data: {
			cases: [
				{
					case_id: unavailable.id,
					availability: 'unavailable',
					reason: unavailable.unavailable_reason,
					selected: false
				}
			]
		}
	});
	expect(await callTool(page, 'select_case', { case_id: unavailable.id })).toMatchObject({
		ok: false,
		error: { code: 'CASE_UNAVAILABLE' }
	});
	expect(await callTool(page, 'inspect_case', {})).toMatchObject({
		ok: true,
		data: { active: false }
	});
});
