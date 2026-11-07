import { describe, expect, it } from 'vitest';
import {
	checkVocabulary,
	eventCredits,
	eventData,
	knownPage,
	type Vocabulary
} from './vocabulary.js';

const vocabulary: Vocabulary = {
	pages: { '/': 'Site', '/privacy': 'Privacy' },
	events: {
		'case.select': ['source', 'demo_case'],
		'calculation.solve': ['source', 'calculation', 'result', 'duration_ms', 'iterations']
	},
	enums: {
		source: new Set(['demo', 'local']),
		demo_case: new Set(['case200', 'case500']),
		calculation: new Set(['dcopf', 'acpf']),
		result: new Set(['completed', 'failed'])
	},
	numbers: new Set(['duration_ms', 'iterations'])
};

describe('knownPage', () => {
	it('keeps listed paths and reduces everything else to the root', () => {
		expect(knownPage(vocabulary.pages, '/privacy')).toBe('/privacy');
		expect(knownPage(vocabulary.pages, 'https://example.test/privacy?secret#secret')).toBe(
			'/privacy'
		);
		expect(knownPage(vocabulary.pages, '/studies/SECRET')).toBe('/');
		expect(knownPage(vocabulary.pages, 42)).toBe('/');
		expect(knownPage({ '/app': 'App' }, '/nowhere')).toBe('/app');
	});
});

describe('eventData', () => {
	it('keeps listed enum values and clamped integers and drops everything else', () => {
		expect(
			eventData(vocabulary, 'calculation.solve', {
				source: 'demo',
				calculation: 'dcopf',
				result: 'completed',
				duration_ms: 123.4,
				iterations: 1e12,
				objective: 98765,
				error: 'SECRET',
				demo_case: 'case500'
			})
		).toEqual({
			source: 'demo',
			calculation: 'dcopf',
			result: 'completed',
			duration_ms: 123,
			iterations: 86_400_000
		});
		expect(eventData(vocabulary, 'case.select', { source: 'UNKNOWN', demo_case: 7 })).toEqual({});
		expect(
			eventData(vocabulary, 'calculation.solve', { duration_ms: -1, iterations: NaN })
		).toEqual({});
		expect(eventData(vocabulary, 'unknown.event', { source: 'demo' })).toEqual({});
	});

	it('honours a lower ceiling', () => {
		expect(eventData(vocabulary, 'calculation.solve', { duration_ms: 5000 }, 1000)).toEqual({
			duration_ms: 1000
		});
	});
});

describe('eventCredits', () => {
	it('counts the event and each stored property', () => {
		expect(eventCredits(vocabulary, 'case.select')).toBe(3);
		expect(eventCredits(vocabulary, 'calculation.solve')).toBe(6);
		expect(eventCredits(vocabulary, 'unknown.event')).toBe(0);
	});
});

describe('checkVocabulary', () => {
	it('accepts a well-formed vocabulary', () => {
		expect(checkVocabulary(vocabulary)).toEqual([]);
	});

	it('names every problem', () => {
		const problems = checkVocabulary({
			pages: { privacy: '' },
			events: {
				['x'.repeat(51)]: ['mystery', 'duration_ms', 'duration_ms'],
				ok: Array.from({ length: 51 }, (_, i) => `p${i}`)
			},
			enums: { empty: new Set(), long: new Set(['y'.repeat(501)]), duration_ms: new Set(['1']) },
			numbers: new Set(['duration_ms'])
		});
		expect(problems.join('\n')).toMatch(/not an absolute path/);
		expect(problems.join('\n')).toMatch(/bad title/);
		expect(problems.join('\n')).toMatch(/name longer than 50/);
		expect(problems.join('\n')).toMatch(/more than 50 properties/);
		expect(problems.join('\n')).toMatch(/lists a property twice/);
		expect(problems.join('\n')).toMatch(/mystery .* exactly one of an enum or a number/);
		expect(problems.join('\n')).toMatch(/duration_ms .* exactly one of an enum or a number/);
		expect(problems.join('\n')).toMatch(/enum empty has no values/);
		expect(problems.join('\n')).toMatch(/enum long has a value outside/);
	});
});
