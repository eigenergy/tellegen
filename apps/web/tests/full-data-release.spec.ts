import { readFile, writeFile } from 'node:fs/promises';
import type { Page } from '@playwright/test';
import type { StudyBundle } from '@tellegen/engine';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness, listTools } from './fixtures/planning-case.js';

test.skip(process.env.TELLEGEN_FULL_DATA !== '1', 'Requires the local four-case dataset server');
test.use({ trace: 'off' });

test.beforeEach(async ({ page, request }) => {
	const version = JSON.parse(
		await readFile(new URL('../build/_app/version.json', import.meta.url), 'utf8')
	);
	expect(await (await request.get('/_app/version.json')).json()).toEqual(version);
	const currentHtml = await readFile(new URL('../build/index.html', import.meta.url), 'utf8');
	const servedHtml = await (await request.get('/')).text();
	const entries = (html: string) =>
		[...html.matchAll(/_app\/immutable\/entry\/(?:start|app)\.[A-Za-z0-9_-]+\.js/g)]
			.map((match) => match[0])
			.sort();
	expect(entries(servedHtml)).toEqual(entries(currentHtml));
	await installWebMcpHarness(page);
	await page.addInitScript(() => {
		const operations: string[] = [];
		Object.defineProperty(window, '__releaseWorkerOps', { value: operations });
		const post = Worker.prototype.postMessage;
		Worker.prototype.postMessage = function (message: unknown, ...args: unknown[]) {
			const op = (message as { op?: string }).op;
			if (op) operations.push(op);
			return Reflect.apply(post, this, [message, ...args]);
		};
	});
});

async function storedSummary(page: Page, selectedId?: string, selectedBus?: string) {
	const menu = page.getByRole('combobox', { name: 'Saved study', exact: true });
	if (!selectedId) {
		await expect(menu).toBeEnabled({ timeout: 90_000 });
		await expect(menu).not.toHaveValue('', { timeout: 90_000 });
	}
	const id = selectedId ?? (await menu.inputValue({ timeout: 15_000 }));
	return page.evaluate(
		async ({ id, selectedBus }) => {
			const db = await new Promise<IDBDatabase>((resolve, reject) => {
				const request = indexedDB.open('tellegen-studies', 1);
				request.onsuccess = () => resolve(request.result);
				request.onerror = () => reject(request.error);
			});
			try {
				const bundle = await new Promise<StudyBundle>((resolve, reject) => {
					const request = db.transaction('studies', 'readonly').objectStore('studies').get(id);
					request.onsuccess = () => resolve(request.result);
					request.onerror = () => reject(request.error);
				});
				const document = bundle.document;
				const layer = JSON.parse(bundle.artifacts[document.display!.geography].text);
				return {
					id: document.id,
					display: document.display,
					state: document.inspected_state,
					has_solution: !!document.states[document.inspected_state!].solution,
					goal_count: Object.keys(document.goals).length,
					solve_count: Object.values(document.experiments).reduce(
						(count, activity) => count + activity.solve_count,
						0
					),
					sample: layer.features.find(
						(feature: { properties: { id?: string } }) => feature.properties.id === selectedBus
					)?.geometry.coordinates,
					points: layer.features.filter(
						(feature: { geometry: { type: string } }) => feature.geometry.type === 'Point'
					).length
				};
			} finally {
				db.close();
			}
		},
		{ id, selectedBus }
	);
}

async function workerOps(page: Page) {
	return page.evaluate(() => [
		...(window as unknown as { __releaseWorkerOps: string[] }).__releaseWorkerOps
	]);
}

test('all demo cases load and Texas7k saves, queries, differentiates and plans in the browser', async ({
	page,
	request
}, testInfo) => {
	test.setTimeout(300_000);
	const health = await request.get('/api/health');
	expect(health.ok()).toBe(true);
	expect(await health.json()).toMatchObject({ status: 'ok', unavailable: [] });
	await page.goto('/');
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
	const catalogue = await callTool(page, 'list_cases', {});
	expect(catalogue.ok, JSON.stringify(catalogue)).toBe(true);
	if (!catalogue.ok) throw new Error(catalogue.error.message);
	const cases = catalogue.data.cases as Array<{ case_id: string; availability: string }>;
	expect(cases.map((c) => c.case_id).sort()).toEqual(['case200', 'case500', 'case7000', 'cats']);
	expect(cases.some((c) => c.availability === 'unavailable')).toBe(false);
	for (const caseId of ['case200', 'case500', 'cats', 'case7000']) {
		const selected = await callTool(page, 'select_case', { case_id: caseId });
		expect(selected).toMatchObject({ ok: true, data: { case_id: caseId, state_id: null } });
		const inspected = await callTool(page, 'inspect_case', {});
		expect(inspected.ok, JSON.stringify(inspected)).toBe(true);
		if (!inspected.ok) throw new Error(inspected.error.message);
		expect(inspected.data.solution).toBeTruthy();
	}
	console.info('Full-data check: all four cases loaded');
	const native = await (await request.get('/api/cases/case7000/solution')).json();
	const highest = [...native.prices].sort((a, b) => b.value - a.value)[0];
	const query = {
		case_id: 'case7000',
		element_kind: 'bus',
		sort_by: 'price',
		direction: 'desc',
		limit: 1
	};
	const before = await callTool(page, 'query_network', query);
	expect(before.ok, JSON.stringify(before)).toBe(true);
	if (!before.ok) throw new Error(before.error.message);
	const peak = (
		before.data.elements as Array<{ element_id: string; legacy_id: number; price: number }>
	)[0];
	expect(peak.price).toBeCloseTo(highest.value, 6);
	const ops = await workerOps(page);
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	await page.getByLabel('Study name', { exact: true }).fill('Texas7k saved result');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(
		page.getByRole('heading', { name: 'Texas7k saved result', exact: true })
	).toBeVisible({ timeout: 90_000 });
	const saved = await storedSummary(page);
	console.info('Full-data check: cached Texas7k result saved');
	expect(saved.solve_count).toBe(0);
	expect(saved.goal_count).toBe(0);
	expect(saved.has_solution).toBe(true);
	const afterSaveOps = (await workerOps(page)).slice(ops.length);
	expect(afterSaveOps).not.toContain('study_new');
	expect(afterSaveOps).not.toContain('study_replace_edits');
	expect(saved.points).toBe(6717);
	expect(saved.display?.camera).toBeTruthy();
	const after = await callTool(page, 'query_network', query);
	expect(after).toMatchObject({
		ok: true,
		data: { state_id: saved.state, elements: [{ price: peak.price }] }
	});
	await page.getByRole('button', { name: 'New study', exact: true }).click();
	await page.getByLabel('Study name', { exact: true }).fill('Texas7k same view');
	await page.getByRole('button', { name: 'Save study', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Texas7k same view', exact: true })).toBeVisible({
		timeout: 90_000
	});
	const second = await storedSummary(page);
	console.info('Full-data check: repeated save retained camera and geography');
	expect(second.display?.camera).toEqual(saved.display?.camera);
	expect(second.display?.geography).toBe(saved.display?.geography);
	console.info('Full-data check: requesting Texas7k sensitivity');
	const derivative = await callTool(page, 'analyze_sensitivity', {
		case_id: 'case7000',
		target: { kind: 'bus', element_id: peak.element_id },
		limit: 3
	});
	expect(derivative.ok, JSON.stringify(derivative)).toBe(true);
	if (!derivative.ok) throw new Error(derivative.error.message);
	expect((derivative.data.responses as unknown[]).length).toBeGreaterThan(0);
	const live = await callTool(page, 'select_case', { case_id: 'case7000' });
	expect(live).toMatchObject({ ok: true, data: { case_id: 'case7000', state_id: null } });
	await expect.poll(() => listTools(page)).toContain('propose_capacity_plan');
	const branches = await callTool(page, 'query_network', {
		case_id: 'case7000',
		element_kind: 'branch',
		sort_by: 'loading',
		direction: 'desc',
		limit: 3
	});
	if (!branches.ok) throw new Error(branches.error.message);
	const inspected = await callTool(page, 'inspect_case', {});
	if (!inspected.ok) throw new Error(inspected.error.message);
	console.info('Full-data check: requesting bounded Texas7k planning');
	const plan = await callTool(page, 'propose_capacity_plan', {
		case_id: 'case7000',
		expected_revision: inspected.data.revision,
		objective: { kind: 'weighted_lmp', weights: [{ bus_id: peak.element_id, weight: 1 }] },
		candidates: (branches.data.elements as Array<{ element_id: string }>).map(
			(branch) => branch.element_id
		),
		max_increase_per_branch_mw: 10,
		budget_mw: 10,
		increment_mw: 5,
		max_changed_lines: 1,
		exact_solve_budget: 3
	});
	expect(plan.ok, JSON.stringify(plan)).toBe(true);
	if (!plan.ok) throw new Error(plan.error.message);
	expect(Number(plan.data.exact_solves)).toBeLessThanOrEqual(3);
	const final = await callTool(page, 'query_network', query);
	expect(final).toMatchObject({
		ok: true,
		data: { state_id: null, elements: [{ price: peak.price }] }
	});
	const evidence = JSON.stringify(
		{
			version: JSON.parse(
				await readFile(new URL('../build/_app/version.json', import.meta.url), 'utf8')
			),
			case_count: cases.length,
			highest_lmp: peak,
			sensitivity: derivative.data,
			planning: plan.data,
			save_worker_operations: afterSaveOps
		},
		null,
		2
	);
	const evidencePath = testInfo.outputPath('texas7k-evidence.json');
	await writeFile(evidencePath, evidence);
	await testInfo.attach('Texas7k browser evidence', {
		path: evidencePath,
		contentType: 'application/json'
	});
	await page.screenshot({ path: testInfo.outputPath('texas7k-saved-and-planned.png') });
});

test('a geographic attachment copies the selected demo and preserves its result', async ({
	page,
	request
}) => {
	test.setTimeout(180_000);
	await page.goto('/');
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
	await callTool(page, 'select_case', { case_id: 'case200' });
	const network = await (await request.get('/api/cases/case200/network')).json();
	const bus = network.buses.find((entry: { demand_mw: number }) => entry.demand_mw > 0);
	const state = await callTool(page, 'inspect_case', {});
	if (!state.ok) throw new Error(state.error.message);
	const changed = await callTool(page, 'update_case', {
		case_id: 'case200',
		expected_revision: state.data.revision,
		mode: 'increment',
		demand: [{ bus_id: String(bus.id), delta_mw: 1 }]
	});
	expect(changed.ok, JSON.stringify(changed)).toBe(true);
	const original = await callTool(page, 'inspect_case', {});
	if (!original.ok) throw new Error(original.error.message);
	const ops = await workerOps(page);
	await page.locator('input[type="file"][accept*=".m,"]').setInputFiles({
		name: 'selected-case-position.csv',
		mimeType: 'text/csv',
		buffer: Buffer.from(`bus_i,lat,lon\n${bus.id},${bus.lat + 0.01},${bus.lon + 0.01}\n`)
	});
	await expect
		.poll(async () => {
			const result = await callTool(page, 'list_cases', {});
			return result.ok ? result.data.total : 0;
		})
		.toBe(5);
	const copy = await callTool(page, 'inspect_case', {});
	expect(copy.ok, JSON.stringify(copy)).toBe(true);
	if (!copy.ok) throw new Error(copy.error.message);
	expect(copy.data.case_id).not.toBe('case200');
	expect(copy.data.solution).toEqual(original.data.solution);
	expect(copy.data.edits).toEqual(original.data.edits);
	expect((await workerOps(page)).slice(ops.length)).not.toContain('study_new');
	const created = await callTool(page, 'create_study', {
		case_id: copy.data.case_id,
		expected_case_revision: copy.data.revision,
		study: { title: 'Attached coordinates', formulation: 'dcopf' }
	});
	expect(created.ok, JSON.stringify(created)).toBe(true);
	if (!created.ok) throw new Error(created.error.message);
	const saved = await storedSummary(page, String(created.data.id), String(bus.id));
	expect(saved.sample).toEqual([bus.lon + 0.01, bus.lat + 0.01]);
	expect(saved.solve_count).toBe(0);
	expect(saved.has_solution).toBe(true);
	await callTool(page, 'select_case', { case_id: 'case200' });
	const restored = await callTool(page, 'inspect_case', {});
	expect(restored).toMatchObject({
		ok: true,
		data: { case_id: 'case200', solution: original.data.solution, edits: original.data.edits }
	});
});
