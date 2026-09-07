import { describe, expect, it, vi } from 'vitest';
import { createStudyAdapter } from '../../../apps/web/src/lib/studies/adapter.js';
import type { StudyWorkspace } from '../../../apps/web/src/lib/studies/workspace.svelte.js';

describe('Study creation input', () => {
	it.each(['study', 'goal'])('accepts %s without a planning objective', async (field) => {
		const create = vi.fn(async () => {});
		const workspace = { create, summary: () => ({ id: 'saved', active_goal: null }) };
		const adapter = createStudyAdapter(workspace as unknown as StudyWorkspace);
		const draft = { title: 'Saved case', formulation: 'dcopf' };
		const signal = new AbortController().signal;
		expect(
			await adapter.execute(
				'create_study',
				{ case_id: 'case-a', expected_case_revision: 'r1', [field]: draft },
				signal
			)
		).toEqual({ id: 'saved', active_goal: null });
		expect(create).toHaveBeenCalledWith(draft, 'case-a', 'r1', signal);
	});

	it('rejects conflicting input aliases without saving', async () => {
		const create = vi.fn(async () => {});
		const adapter = createStudyAdapter({ create } as unknown as StudyWorkspace);
		await expect(
			adapter.execute(
				'create_study',
				{ case_id: 'case-a', expected_case_revision: 'r1', study: {}, goal: {} },
				new AbortController().signal
			)
		).rejects.toMatchObject({ code: 'INVALID_INPUT' });
		expect(create).not.toHaveBeenCalled();
	});
});
