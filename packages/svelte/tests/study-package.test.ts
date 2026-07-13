import { describe, expect, it } from 'vitest';
import { studyCommitIndex } from '../src/lib/study-package.js';

describe('studyCommitIndex', () => {
	it('targets the last study commit', () => {
		expect(studyCommitIndex(JSON.stringify({ study: { commits: [{}, {}, {}] } }))).toBe(2);
		expect(studyCommitIndex(JSON.stringify({ study: { commits: [{}] } }))).toBe(1 - 1);
	});

	it('is 0 for a package with no study commits (export writes the base network)', () => {
		expect(studyCommitIndex(JSON.stringify({ study: { commits: [] } }))).toBe(0);
		expect(studyCommitIndex(JSON.stringify({ model_kind: 'balanced' }))).toBe(0);
	});

	it('is 0 for malformed input rather than throwing', () => {
		for (const bad of ['', '{', 'not json']) {
			expect(studyCommitIndex(bad)).toBe(0);
		}
	});
});
