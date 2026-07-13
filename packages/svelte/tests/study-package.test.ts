import { describe, expect, it } from 'vitest';
import {
	isStudyPackageText,
	PIO_PACKAGE_SCHEMA_PREFIX,
	studyCommitIndex
} from '../src/lib/study-package.js';

describe('isStudyPackageText', () => {
	it('recognizes a `.pio.json` envelope by its schema field', () => {
		const pkg = JSON.stringify({ schema: `${PIO_PACKAGE_SCHEMA_PREFIX}/0.1`, model_kind: 'balanced' });
		expect(isStudyPackageText(pkg)).toBe(true);
	});

	it('rejects a plain powerio network JSON (no package schema)', () => {
		expect(isStudyPackageText(JSON.stringify({ buses: [], branches: [] }))).toBe(false);
		expect(isStudyPackageText(JSON.stringify({ schema: 'https://example.com/other' }))).toBe(false);
	});

	it('returns false for non-JSON or truncated input rather than throwing', () => {
		for (const bad of ['', '   ', '{', 'not json', 'null', '[]', '42']) {
			expect(isStudyPackageText(bad)).toBe(false);
		}
	});
});

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
