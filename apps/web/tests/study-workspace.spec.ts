import { readFile, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import type { Page } from '@playwright/test';
import type { StudyBundle } from '@tellegen/engine';
import { expect, test } from './fixtures/page-errors.js';
import { installWebMcpHarness, congestCase, callTool } from './fixtures/planning-case.js';

async function bundle(page: Page): Promise<StudyBundle> {
	const download = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const file = await download;
	return JSON.parse(await readFile((await file.path())!, 'utf8'));
}

async function create(page: Page) {
	await installWebMcpHarness(page);
	await congestCase(page);
	await page.getByRole('button', { name: 'Studies Explore a goal +' }).click();
	await page.getByRole('button', { name: 'Resolve equipment and weights' }).click();
	await page.getByRole('button', { name: 'Create from live case' }).click();
	await expect(page.getByText('Revision 0, 1 saved states')).toBeVisible({ timeout: 60_000 });
	return bundle(page);
}

test('Study proposal, branching, durable reload and explicit application', async ({
	page
}, testInfo) => {
	test.setTimeout(180_000);
	const initial = await create(page);
	const d = initial.document;
	const request = {
		study_id: d.id,
		expected_revision: d.revision,
		operation: {
			kind: 'propose',
			state: d.inspected_state,
			goal: d.active_goal,
			options: { max_solves: 8, max_iterations: 3, beam_width: 2, min_improvement: 1e-7 },
			rationale: 'Relieve the constrained corridor and compare prices across the network'
		}
	};
	const result = await callTool(page, 'propose_study', request);
	expect(result).toMatchObject({ ok: true, data: { revision: 1, applied_state: d.applied_state } });
	if (!result.ok) throw new Error(result.error.message);
	const proposed = await bundle(page);
	expect(proposed.document.recommended_state).toBeTruthy();
	expect(proposed.document.recommended_state).not.toBe(d.applied_state);
	const experiment = proposed.document.experiments[String(result.data.experiment)];
	expect(experiment.solve_count).toBeLessThanOrEqual(8);
	expect(experiment.trials.length).toBeGreaterThan(0);
	expect(proposed.document.applied_state).toBe(d.applied_state);

	const stale = await callTool(page, 'propose_study', request);
	expect(stale.ok).toBe(false);
	const branch = await callTool(page, 'branch_study', {
		study_id: d.id,
		expected_revision: 1,
		operation: {
			kind: 'branch',
			state: proposed.document.recommended_state,
			rationale: 'Inspect the exact candidate before applying it'
		}
	});
	expect(branch).toMatchObject({ ok: true, data: { revision: 2, applied_state: d.applied_state } });
	await page.getByRole('button', { name: 'Compare with starting point' }).click();
	await expect(page.getByRole('heading', { name: 'Goal progress' })).toBeVisible();
	await testInfo.attach('Study comparison, desktop', {
		body: await page.screenshot(),
		contentType: 'image/png'
	});
	const viewed = await bundle(page);
	expect(viewed.document.inspected_state).toBe(proposed.document.recommended_state);
	expect(viewed.document.applied_state).toBe(d.applied_state);
	await page.reload();
	await page.getByRole('button', { name: 'Studies Explore a goal +' }).click();
	await page.getByLabel('Saved study').selectOption(d.id);
	await expect(page.getByText('Saved-state map.', { exact: false })).toBeVisible();
	const restored = await bundle(page);
	expect(restored).toEqual(viewed);
	await page.getByRole('button', { name: 'Apply this recommendation to the Study' }).click();
	await expect(
		page.getByRole('button', { name: 'Apply this recommendation to the Study' })
	).toHaveCount(0);
	const applied = await bundle(page);
	expect(applied.document.applied_state).toBe(proposed.document.recommended_state);
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.locator('.study-workspace')).toBeVisible();
	await page.locator('.study-workspace section').evaluate((el) => (el.scrollTop = 0));
	await testInfo.attach('Applied Study, mobile', {
		body: await page.screenshot(),
		contentType: 'image/png'
	});

	if (process.env.TELLEGEN_STUDY_CLI) {
		const dir = await mkdtemp(join(tmpdir(), 'tellegen-study-parity-'));
		const path = join(dir, 'study.json');
		await writeFile(path, JSON.stringify(initial));
		const output = execFileSync(process.env.TELLEGEN_STUDY_CLI, ['study', 'run', path], {
			input: JSON.stringify({ expected_revision: d.revision, operation: request.operation }),
			maxBuffer: 16 * 1024 * 1024,
			encoding: 'utf8'
		});
		const native = JSON.parse(await readFile(path, 'utf8')) as StudyBundle;
		const nativeExperiment = native.document.experiments[JSON.parse(output).experiment];
		expect(nativeExperiment.solve_count).toBe(experiment.solve_count);
		expect(nativeExperiment.trials.length).toBe(experiment.trials.length);
		for (let i = 0; i < experiment.trials.length; i++) {
			expect(nativeExperiment.trials[i].accepted).toBe(experiment.trials[i].accepted);
			if (experiment.trials[i].exact_value != null)
				expect(nativeExperiment.trials[i].exact_value).toBeCloseTo(
					experiment.trials[i].exact_value!,
					5
				);
		}
	}
});

test('Study import rejects tampered artifacts and goal revisions invalidate recommendations', async ({
	page
}) => {
	test.setTimeout(120_000);
	const initial = await create(page),
		d = initial.document;
	const [goalId, goal] = Object.entries(d.goals)[0];
	const revised = await callTool(page, 'revise_study_goal', {
		study_id: d.id,
		expected_revision: 0,
		operation: {
			kind: 'revise_goal',
			goal: { ...goal, parent: goalId, request: 'Try a different price target' }
		}
	});
	expect(revised).toMatchObject({ ok: true, data: { revision: 1, recommended_state: null } });
	const saved = await bundle(page);
	expect(Object.keys(saved.document.goals)).toHaveLength(2);
	expect(saved.document.goals[goalId]).toEqual(goal);
	const corrupt = structuredClone(saved);
	corrupt.artifacts[Object.keys(corrupt.artifacts)[0]].text += 'tampered';
	await page.locator('.study-workspace input[type="file"]').setInputFiles({
		name: 'tampered.json',
		mimeType: 'application/json',
		buffer: Buffer.from(JSON.stringify(corrupt))
	});
	await expect(page.getByRole('alert')).toContainText(/hash|artifact|JSON|invalid/i);
	expect(await bundle(page)).toEqual(saved);
});

test('cancelled Study proposal saves its completed planning record', async ({ page }) => {
	test.setTimeout(120_000);
	const initial = await create(page);
	await page.getByRole('button', { name: 'Find a proposal' }).click();
	await page.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Find a proposal' })).toBeEnabled({
		timeout: 60_000
	});
	const saved = await bundle(page);
	expect(saved.document.revision).toBe(1);
	expect(saved.document.applied_state).toBe(initial.document.applied_state);
	const record = saved.document.experiments[saved.document.experiment_order.at(-1)!];
	expect(record.kind).toBe('planning');
	expect(record.termination).toBe('cancelled');
	await page.reload();
	await page.getByRole('button', { name: 'Studies Explore a goal +' }).click();
	await page.getByLabel('Saved study').selectOption(saved.document.id);
	expect(await bundle(page)).toEqual(saved);
});

test('storage exhaustion leaves the saved Study intact and permits recovery', async ({ page }) => {
	test.setTimeout(120_000);
	await page.addInitScript(() => {
		const put = IDBObjectStore.prototype.put;
		IDBObjectStore.prototype.put = function (...args: Parameters<IDBObjectStore['put']>) {
			if (sessionStorage.getItem('simulate-study-quota') === 'yes')
				throw new DOMException('Synthetic quota exhaustion', 'QuotaExceededError');
			return put.apply(this, args);
		};
	});
	const initial = await create(page);
	await page.evaluate(() => sessionStorage.setItem('simulate-study-quota', 'yes'));
	await page.getByRole('button', { name: 'Find a proposal' }).click();
	await expect(page.getByRole('alert')).toContainText('Free browser storage', { timeout: 60_000 });
	expect(await bundle(page)).toEqual(initial);
	await page.evaluate(() => sessionStorage.removeItem('simulate-study-quota'));
	await page.getByRole('button', { name: 'Find a proposal' }).click();
	await expect(page.getByText(/Revision 1, .* saved states/)).toBeVisible({ timeout: 60_000 });
	expect((await bundle(page)).document.revision).toBe(1);
});
