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

const collectionsEnabled = (process.env.TELLEGEN_POWERIO_FEATURES ?? '')
	.split(',')
	.includes('powerio-collections');

test('the baseline build hides collection controls and explains unsupported ZIP imports', async ({
	page
}) => {
	test.skip(collectionsEnabled, 'The candidate build enables collections');
	await expect(page.getByRole('button', { name: 'Open folder' })).toHaveCount(0);
	await page.locator('input[type="file"]').setInputFiles({
		name: 'study.zip',
		mimeType: 'application/zip',
		buffer: storedZip({ 'case.m': comparison })
	});
	await expect(
		page.getByText(
			'ZIP and folder imports are unavailable in this build. Open a single case file instead.'
		)
	).toBeVisible();
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

test.describe('PowerIO collection build', () => {
	test.skip(!collectionsEnabled, 'Requires the optional PowerIO collection build');

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

	test('folder picker preserves shared paths and accepts more than 32 members', async ({
		page
	}) => {
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

	test('rejecting an unsafe archive leaves an independent valid ZIP available', async ({
		page
	}) => {
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
});
