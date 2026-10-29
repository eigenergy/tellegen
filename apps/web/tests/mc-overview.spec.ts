import { readFile } from 'node:fs/promises';
import type { Page } from '@playwright/test';
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

/** Load a BMOPF document from memory, solve it, and return the overview. */
async function solveDocument(page: Page, name: string, doc: unknown) {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles({
		name,
		mimeType: 'application/json',
		buffer: Buffer.from(JSON.stringify(doc))
	});
	const solve = page.getByRole('button', { name: 'Solve AC power flow', exact: true });
	await expect(solve).toBeEnabled({ timeout: 60_000 });
	await solve.click();
	const overview = page.getByRole('region', { name: 'Operating point overview' });
	await expect(overview.getByRole('heading', { name: /^Issues/ })).toBeVisible({ timeout: 60_000 });
	return overview;
}

test('phase-to-ground bounds are ranked like the other references', async ({ page }) => {
	// `lb` trades its phase–neutral band for BMOPF phase-to-ground bounds, one per phase terminal:
	// non-uniform minimums and a uniform maximum. Solved, c–ground is 213.58 V.
	const doc = JSON.parse(await readFile(fixture, 'utf8'));
	const { vpn_min: _min, vpn_max: _max, ...lb } = doc.bus.lb;
	doc.bus.lb = { ...lb, v_min: [210, 220, 216], v_max: [253, 253, 253] };
	const overview = await solveDocument(page, 'four-wire-ground.bmopf.json', doc);

	const issues = overview.locator('.issues button');
	await expect(issues).toHaveCount(2);
	await expect(issues.nth(0)).toContainText('lb');
	await expect(issues.nth(0)).toContainText('c–ground below 216.00 V by 1.12 %');
	await expect(overview.getByRole('button', { name: /V phase–ground/ })).toContainText(
		'limit violated'
	);
	await issues.nth(0).click();
	const bars = overview
		.getByRole('list', { name: 'Pair voltages against declared limits' })
		.getByRole('listitem');
	await expect(bars).toHaveCount(3);
	await expect(bars.nth(1)).toContainText('b–ground');

	const download = page.waitForEvent('download');
	await overview.getByRole('button', { name: 'Bus CSV' }).click();
	const csv = await readFile(await (await download).path(), 'utf8');
	expect(csv).toMatch(/^lb,.*phase–ground,a–ground b–ground c–ground,.*,violation$/m);
});

test('the overview says whether a case declares limits it does not rank', async ({ page }) => {
	const example = new URL('../static/examples/four-wire.bmopf.json', import.meta.url);
	const doc = JSON.parse(await readFile(example, 'utf8'));
	delete doc.linecode['4w'].i_max;
	const plain = await solveDocument(page, 'four-wire-plain.bmopf.json', doc);
	await expect(plain).toContainText(
		'This case declares no voltage limits or ratings, so nothing is ranked against them.'
	);
	await expect(plain.getByRole('button', { name: /Violations/ })).toContainText(
		'no limits declared'
	);

	// A neutral-to-ground cap is declared but not a band the overview ranks.
	doc.bus.lb.vn_max = 10;
	const capped = await solveDocument(page, 'four-wire-capped.bmopf.json', doc);
	await expect(capped).toContainText('none of its voltage limits is a band the overview ranks');
	await expect(capped.getByRole('button', { name: /Violations/ })).toContainText(
		'no ranked limits'
	);
});
