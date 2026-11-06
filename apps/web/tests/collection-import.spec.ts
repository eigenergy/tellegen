import { expect, test } from './fixtures/page-errors.js';
import { storedZip, day, night, line, comparison } from './fixtures/collection-zip.js';
import { mkdtemp, mkdir, writeFile, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

test.beforeEach(async ({ page }) => {
	await page.route('**/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
});

test('dragging a mixed ZIP discovers both shared masters and keeps successful imports', async ({
	page
}) => {
	const bytes = storedZip({
		'masters/day.dss': day,
		'masters/night.dss': night,
		'shared/line.inc': line,
		'comparison.m': comparison,
		'bad.m': 'not a MATPOWER case'
	});
	const transfer = await page.evaluateHandle((encoded) => {
		const data = new DataTransfer();
		const bytes = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
		data.items.add(new File([bytes], 'study.zip', { type: 'application/zip' }));
		return data;
	}, bytes.toString('base64'));
	await page.locator('body').dispatchEvent('drop', { dataTransfer: transfer });
	const dialog = page.getByRole('dialog', { name: 'Choose cases from collection' });
	await expect(dialog).toBeVisible();
	await expect(dialog.locator('.candidate')).toHaveCount(4);
	await dialog.getByText('Source collections and downloads', { exact: true }).click();
	const downloadEvent = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Download original ZIP' }).click();
	const download = await downloadEvent;
	expect(await readFile((await download.path())!)).toEqual(bytes);
	await expect(page.locator('.case-chip.local')).toHaveCount(0);
	await dialog.getByRole('button', { name: 'Select all matching' }).click();
	await dialog.getByRole('button', { name: 'Import selected cases' }).click();
	await expect(dialog.getByRole('status')).toContainText('Import finished', { timeout: 30_000 });
	await expect(dialog.locator('[data-outcome="failed"]')).toHaveCount(1);
	await expect(page.locator('.case-chip.local')).toHaveCount(3);
	// Imported rows remain selectable for source export, without being imported twice.
	await dialog.getByRole('button', { name: 'Clear selection' }).click();
	await dialog
		.locator('.candidate')
		.filter({ hasText: 'comparison.m' })
		.getByRole('checkbox')
		.check();
	await expect(dialog.getByRole('button', { name: 'Import selected cases' })).toBeDisabled();
	const sourceEvent = page.waitForEvent('download');
	await dialog.getByRole('button', { name: 'Export selected source ZIP' }).click();
	const source = await readFile((await (await sourceEvent).path())!);
	expect(source.includes(Buffer.from(comparison))).toBe(true);
	expect(source.includes(Buffer.from(day))).toBe(false);
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(
		page.getByRole('button', { name: /study.zip \/ masters\/day.dss Multiconductor/ })
	).toBeVisible();
	await expect(
		page.getByRole('button', { name: /study.zip \/ masters\/night.dss Multiconductor/ })
	).toBeVisible();
});

test('folder picker preserves shared paths and accepts more than 32 members', async ({ page }) => {
	const folder = await mkdtemp(join(tmpdir(), 'tellegen-folder-'));
	try {
		await mkdir(join(folder, 'masters'));
		await mkdir(join(folder, 'shared'));
		await Promise.all([
			writeFile(join(folder, 'masters/day.dss'), day),
			writeFile(join(folder, 'masters/night.dss'), night),
			writeFile(join(folder, 'shared/line.inc'), line),
			...Array.from({ length: 40 }, (_, i) =>
				writeFile(join(folder, `note-${i}.txt`), 'opaque companion')
			)
		]);
		const chooserEvent = page.waitForEvent('filechooser');
		await page.getByRole('button', { name: 'Open folder' }).click();
		await (await chooserEvent).setFiles(folder);
		const dialog = page.getByRole('dialog');
		await expect(dialog.locator('.candidate')).toHaveCount(2);
		await dialog.getByRole('button', { name: 'Select all matching' }).click();
		await dialog.getByRole('button', { name: 'Import selected cases' }).click();
		await expect(dialog.getByRole('status')).toContainText('Import finished');
		await expect(dialog.locator('[data-outcome="failed"]')).toHaveCount(0);
		await expect(page.locator('.case-chip.local')).toHaveCount(2);
	} finally {
		await rm(folder, { recursive: true, force: true });
	}
});

test('separate ZIPs cannot supply missing includes even when their names collide', async ({
	page
}) => {
	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'same.zip',
			mimeType: 'application/zip',
			buffer: storedZip({ 'main.dss': 'New Circuit.broken basekv=12.47\nRedirect line.inc\n' })
		},
		{
			name: 'same.zip',
			mimeType: 'application/zip',
			buffer: storedZip({ 'line.inc': line, 'comparison.m': comparison })
		}
	]);
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('.candidate')).toHaveCount(2);
	await dialog.getByRole('button', { name: 'Select all matching' }).click();
	await dialog.getByRole('button', { name: 'Import selected cases' }).click();
	await expect(dialog.locator('[data-outcome="incomplete"]')).toHaveCount(1);
	await expect(dialog.locator('[data-outcome="imported"]')).toHaveCount(1);
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(page.locator('.case-chip.local')).toHaveCount(2);
});

test('rejecting an unsafe archive leaves an independent valid ZIP available', async ({ page }) => {
	await page.locator('input[type="file"]').setInputFiles([
		{
			name: 'unsafe.zip',
			mimeType: 'application/zip',
			buffer: storedZip({ '../escape.m': comparison })
		},
		{ name: 'good.zip', mimeType: 'application/zip', buffer: storedZip({ 'case.m': comparison }) }
	]);
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('.candidate')).toHaveCount(1);
	await expect(dialog.getByRole('alert')).toContainText('unsafe.zip');
	await dialog.getByRole('button', { name: 'Select all matching' }).click();
	await dialog.getByRole('button', { name: 'Import selected cases' }).click();
	await expect(dialog.locator('[data-outcome="imported"]')).toHaveCount(1);
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(page.locator('.case-chip.local')).toHaveCount(1);
});

test('closing a chooser releases it and allows re-import without adding unchecked cases', async ({
	page
}) => {
	const file = {
		name: 'study.zip',
		mimeType: 'application/zip',
		buffer: storedZip({ 'case.m': comparison })
	};
	await page.locator('input[type="file"]').setInputFiles(file);
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('.candidate')).toHaveCount(1);
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(page.locator('.case-chip.local')).toHaveCount(0);
	await page.locator('input[type="file"]').setInputFiles(file);
	await expect(dialog.locator('.candidate')).toHaveCount(1);
	await dialog.getByRole('button', { name: 'Select all matching' }).click();
	await dialog.getByRole('button', { name: 'Import selected cases' }).click();
	await expect(dialog.locator('[data-outcome="imported"]')).toHaveCount(1);
});

test('a zipped four-wire snapshot runs while an unsupported neighbour stays view-only', async ({
	page
}) => {
	const references = JSON.parse(
		await readFile(
			new URL('../../../crates/tellegen/tests/data/dss_snapshots.json', import.meta.url),
			'utf8'
		)
	);
	const source = references.cases.find((c: { name: string }) => c.name === 'four_wire').source;
	const unsupported =
		'new circuit.three basekv=11\nnew transformer.t phases=3 windings=3 buses=[sourcebus lb lc] conns=[wye wye delta] kvs=[11 .4 .4] kvas=[100 100 100] xhl=4 xht=5 xlt=6\n';
	await page.locator('input[type="file"]').setInputFiles({
		name: 'snapshots.zip',
		mimeType: 'application/zip',
		buffer: storedZip({ 'four-wire.dss': source, 'three-winding.dss': unsupported })
	});
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('.candidate')).toHaveCount(2);
	await dialog.getByRole('button', { name: 'Select all matching' }).click();
	await dialog.getByRole('button', { name: 'Import selected cases' }).click();
	await expect(dialog.locator('[data-outcome="view-only"]')).toContainText('3 windings');
	await expect(dialog.locator('[data-outcome="imported"]')).toHaveCount(1);
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await page.getByRole('button', { name: /snapshots.zip \/ four-wire.dss Multiconductor/ }).click();
	await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const study = page.getByRole('region', { name: 'Study workspace' });
	await expect(study.getByRole('button', { name: 'Run power flow' })).toBeEnabled();
	await study.getByRole('button', { name: 'Run power flow' }).click();
	await expect(study.getByText('Converged', { exact: true })).toBeVisible();
	await expect(
		study.getByText('Original load model verified', { exact: true }).first()
	).toBeVisible();
	await expect(study.getByRole('button', { name: 'Save result' })).toBeEnabled();
});

test('a large collection keeps the first and last case reachable', async ({ page }) => {
	const cases = Object.fromEntries(
		Array.from({ length: 20 }, (_, i) => [`case-${String(i).padStart(2, '0')}.m`, comparison])
	);
	await page
		.locator('input[type="file"]')
		.setInputFiles({ name: 'many.zip', mimeType: 'application/zip', buffer: storedZip(cases) });
	const dialog = page.getByRole('dialog');
	await expect(dialog.locator('.candidate')).toHaveCount(20);
	await dialog.getByRole('button', { name: 'Select all matching' }).click();
	await dialog.getByRole('button', { name: 'Import selected cases' }).click();
	await expect(dialog.locator('[data-outcome="imported"]')).toHaveCount(20);
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	for (const width of [1280, 390]) {
		await page.setViewportSize({ width, height: 800 });
		for (const name of ['case-00.m', 'case-19.m', 'case-00.m']) {
			await page.getByRole('button', { name: new RegExp(`${name} Local`) }).click();
			await expect(page.locator('.case-chip.active .cname')).toContainText(name);
		}
	}
});
