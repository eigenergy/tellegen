import { readFile } from 'node:fs/promises';
import { expect, test } from './fixtures/page-errors.js';

// The shipped four-wire example with declared limits (fixtures/four-wire-limits.LICENSE.md): the
// solved case has one phase-neutral undervoltage at `lb` and one loading warning on `l1`.
const fixture = new URL('./fixtures/four-wire-limits.bmopf.json', import.meta.url);

test('multiconductor overview ranks declared-limit issues and links them to the selection', async ({
	page
}) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	// The prerendered input exists before hydration attaches its listener; a file set earlier is
	// lost. The empty-cases panel renders only after load.
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name: 'four-wire-limits.bmopf.json',
		mimeType: 'application/json',
		buffer: await readFile(fixture)
	});
	const solve = page.getByRole('button', { name: 'Solve AC power flow', exact: true });
	await expect(solve).toBeEnabled({ timeout: 60_000 });
	await solve.click();

	const overview = page.getByRole('region', { name: 'Operating point overview' });
	await expect(overview.getByRole('button', { name: /Violations/ })).toContainText('1', {
		timeout: 60_000
	});
	await expect(overview.getByRole('button', { name: /Violations/ })).toContainText('+ 1 warning');

	const issues = overview.locator('.issues button');
	await expect(issues).toHaveCount(2);
	await expect(issues.nth(0)).toContainText('lb');
	await expect(issues.nth(0)).toContainText('c–n below 207.00 V');
	await expect(issues.nth(1)).toContainText('l1');
	await expect(issues.nth(1)).toContainText('of rating');

	await issues.nth(0).click();
	await expect(overview.getByRole('heading', { name: 'Phases at lb' })).toBeVisible();
	await expect(
		overview
			.getByRole('list', { name: 'Pair voltages against declared limits' })
			.getByRole('listitem')
	).toHaveCount(3);
	await expect(overview).toContainText('(VUF)');

	const search = overview.getByRole('combobox', { name: 'Find a bus or branch' });
	await search.fill('l1');
	await expect(overview.getByRole('option')).toContainText(['l1']);
	await search.press('Enter');
	await expect(overview.getByRole('heading', { name: 'Conductors on l1' })).toBeVisible();
	await expect(overview).toContainText('neutral carries');

	await overview.getByText('Results table').click();
	await overview.getByRole('tab', { name: 'Branches' }).click();
	const table = overview.getByRole('grid', { name: 'Results by branch' });
	await expect(table.locator('[role=row][aria-selected=true]')).toContainText('l1');

	const download = page.waitForEvent('download');
	await overview.getByRole('button', { name: 'Bus CSV' }).click();
	const csv = await readFile(await (await download).path(), 'utf8');
	expect(csv).toContain('# Case:');
	expect(csv).toMatch(/^lb,.*,violation$/m);
});
