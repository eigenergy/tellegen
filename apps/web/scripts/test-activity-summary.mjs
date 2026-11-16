import { test } from 'node:test';
import assert from 'node:assert/strict';
import { activitySummary, formatChange } from '../src/lib/webmcp/activity-summary.ts';

test('a before and after pair shares one precision from the larger magnitude', () => {
	assert.equal(formatChange(626.95, 876.5), '626.95 → 876.50');
	assert.equal(formatChange(876.5, 1234.567), '876.5 → 1,234.6');
	assert.equal(formatChange(9_999.4, 12_345.678), '9,999 → 12,346');
	assert.equal(formatChange(-1500, 20), '-1,500.0 → 20.0');
	assert.equal(formatChange(3, 3), '3.00 → 3.00');
});

test('the update summary uses the shared precision', () => {
	const summary = activitySummary({
		type: 'finished',
		id: 'tellegen-tool-1',
		toolName: 'update_case',
		title: 'Update and solve case',
		startedAt: 0,
		finishedAt: 208,
		response: {
			ok: true,
			data: {
				before: { objective: 626.95, binding_branches: 0 },
				after: { objective: 876.5, binding_branches: 1 }
			}
		}
	});
	assert.equal(summary, 'Cost 626.95 → 876.50 $/h, 1 line at limit');
});

test('a failure reads in a word or two, not the message written for the agent', () => {
	const failed = (code) =>
		activitySummary({
			type: 'finished',
			id: 'tellegen-tool-2',
			toolName: 'focus_network',
			title: 'Focus network element',
			startedAt: 0,
			finishedAt: 10,
			response: {
				ok: false,
				error: { code, message: 'case_id must be a nonempty string of at most 128 characters' }
			}
		});
	assert.equal(failed('INVALID_INPUT'), 'Invalid input');
	assert.equal(failed('SOMETHING_NEW'), 'Failed');
});
