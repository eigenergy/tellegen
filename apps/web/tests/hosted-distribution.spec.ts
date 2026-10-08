import { readFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness } from './fixtures/planning-case.js';

test('hosted distribution cases load, solve, remain unique in WebMCP, and restore', async ({
	page
}) => {
	await installWebMcpHarness(page);
	const module = await readFile(
		new URL('./fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json', import.meta.url),
		'utf8'
	);
	const catalogue = ['feeder-a', 'feeder-b'].map((id) => ({
		id,
		name: `Hosted ${id}`,
		model: 'multiconductor',
		distribution: { pf_options: { voltage_envelope: false, max_iterations: 500 } },
		n_bus: 2,
		n_branch: 1,
		n_gen: 0,
		unavailable_reason: null
	}));
	const caseRequests: string[] = [];
	await page.route('**/api/**', (route) => {
		const path = new URL(route.request().url()).pathname;
		if (path === '/api/cases') return route.fulfill({ json: catalogue });
		if (path === '/api/compute') return route.fulfill({ json: { enabled: false } });
		if (/\/api\/cases\/feeder-[ab]\/case$/.test(path)) {
			caseRequests.push(path);
			return route.fulfill({ contentType: 'application/json', body: module });
		}
		throw new Error(`Unexpected balanced/server-compute request: ${path}`);
	});
	await page.goto('/');
	await expect(
		page.getByRole('button', { name: 'Solve AC power flow', exact: true })
	).toBeEnabled();
	await expect(
		page.getByRole('application', { name: 'Network diagram', exact: true })
	).toBeVisible();
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
	let listed = await callTool(page, 'list_cases', {});
	expect(listed).toMatchObject({
		ok: true,
		data: {
			cases: expect.arrayContaining([
				{
					case_id: 'feeder-a',
					name: 'Hosted feeder-a',
					kind: 'server',
					availability: 'ready',
					calculation: 'multiconductor_ac_pf',
					selected: true
				},
				{
					case_id: 'feeder-b',
					name: 'Hosted feeder-b',
					kind: 'server',
					availability: 'load_on_selection',
					calculation: 'multiconductor_ac_pf',
					selected: false
				}
			])
		}
	});
	expect(caseRequests).toEqual(['/api/cases/feeder-a/case']);
	expect(await callTool(page, 'select_case', { case_id: 'feeder-b' })).toMatchObject({
		ok: true,
		data: { case_id: 'feeder-b', selected: true, network_ready: true }
	});
	await page.getByRole('button', { name: 'Solve AC power flow', exact: true }).click();
	await expect(page.getByRole('table', { name: 'Terminal results', exact: true })).toBeVisible();
	const inspected = await callTool(page, 'inspect_case', {});
	expect(inspected.ok).toBe(true);
	if (!inspected.ok) throw new Error(inspected.error.message);
	expect(
		await callTool(page, 'solve_multiconductor_pf', {
			case_id: 'feeder-b',
			expected_revision: inspected.data.revision
		})
	).toMatchObject({ ok: true, data: { converged: true, terminal_count: 8 } });
	if (!(await page.getByRole('region', { name: 'Study workspace' }).isVisible()))
		await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const exported = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const exportPath = await (await exported).path();
	const snapshot = JSON.parse(await readFile(exportPath!, 'utf8'));
	expect(snapshot.options).toMatchObject({ voltage_envelope: false, max_iterations: 500 });
	await page.locator('label.file-button input[type="file"]').setInputFiles(exportPath!);
	await expect
		.poll(async () => {
			const value = await callTool(page, 'inspect_case', {});
			return value.ok ? value.data.case_id : '';
		})
		.toMatch(/^dist-/);
	const study = page.getByRole('region', { name: 'Study workspace' });
	await expect(study.getByText('Converged', { exact: true })).toBeVisible();
	await study.getByRole('button', { name: 'Run power flow' }).click();
	await expect(study.getByText('Converged', { exact: true })).toBeVisible();
	await expect(
		page
			.getByRole('region', { name: 'Study workspace' })
			.getByRole('button', { name: 'Run power flow' })
	).toBeEnabled({ timeout: 90_000 });
	const reopened = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const reopenedPath = await (await reopened).path();
	expect(JSON.parse(await readFile(reopenedPath!, 'utf8')).options).toEqual(snapshot.options);
	expect(await callTool(page, 'select_case', { case_id: 'feeder-b' })).toMatchObject({ ok: true });
	const imported = page.locator('.case-chip.local .case-remove');
	// Remove only the reopened local case; retain the hosted catalogue entry.
	await imported.last().click();
	listed = await callTool(page, 'list_cases', {});
	if (!listed.ok) throw new Error(listed.error.message);
	expect(listed.data.cases).toHaveLength(2);
	await page.getByRole('button', { name: 'remove Hosted feeder-b', exact: true }).click();
	await page.reload();
	await expect(
		page.getByRole('button', { name: 'Solve AC power flow', exact: true })
	).toBeEnabled();
	await expect(page.locator('.case-activate').filter({ hasText: 'Hosted feeder-b' })).toHaveCount(
		0
	);
	await page.getByRole('button', { name: /restore default cases/ }).click();
	await expect(page.locator('.case-activate').filter({ hasText: 'Hosted feeder-b' })).toHaveCount(
		1
	);
});
