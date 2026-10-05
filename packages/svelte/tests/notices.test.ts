import { afterEach, describe, expect, it, vi } from 'vitest';
import { errorNoticeTitle, NoticeCenter } from '../src/lib/notices.svelte.js';
import { AppState } from '../src/lib/state.svelte.js';

afterEach(() => vi.useRealTimers());
describe('error titles', () => {
	it.each([
		'map failed to load: Failed to initialize WebGL2. Read more at the browser support page',
		'the map lost its graphics context repeatedly; reload the page to restore it'
	])('labels graphics failures as map errors: %s', (details) => {
		expect(errorNoticeTitle(details)).toBe('Map unavailable');
	});
	it('identifies an unsupported AC power flow boundary as a calculation failure', () => {
		expect(
			errorNoticeTitle(
				'Texas7k: AC power flow reference bus 111333 states angle -11.99 degrees; Tellegen currently requires zero'
			)
		).toBe('Calculation did not complete');
	});
	it('keeps the file error title for actual parse failures', () => {
		expect(errorNoticeTitle('Could not parse case.m: invalid bus row')).toBe(
			'Could not read this file'
		);
	});
});
describe('recent messages', () => {
	it('groups repeats, dismisses without losing details, and bounds retained messages', () => {
		vi.useFakeTimers();
		const notices = new NoticeCenter();
		notices.push({
			title: 'Calculation did not complete',
			details: 'Full solver error',
			kind: 'error'
		});
		notices.push({
			title: 'Calculation did not complete',
			details: 'Full solver error',
			kind: 'error'
		});
		expect(notices.entries).toHaveLength(1);
		expect(notices.active?.count).toBe(2);
		vi.advanceTimersByTime(6500);
		expect(notices.active).toBeNull();
		expect(notices.entries[0].details).toBe('Full solver error');
		for (let index = 0; index < 40; index++)
			notices.push({ title: 'Message', details: `Details ${index}` });
		expect(notices.entries).toHaveLength(30);
		notices.dispose();
	});
	it('pauses dismissal during interaction and restarts the remaining time', () => {
		vi.useFakeTimers();
		const notices = new NoticeCenter();
		notices.push({
			title: 'Could not save locally',
			details: 'Storage details'
		});
		vi.advanceTimersByTime(2000);
		notices.pause(true);
		vi.advanceTimersByTime(30_000);
		expect(notices.active).not.toBeNull();
		notices.pause(false);
		vi.advanceTimersByTime(4499);
		expect(notices.active).not.toBeNull();
		vi.advanceTimersByTime(1);
		expect(notices.active).toBeNull();
	});
	it('counts identical error writes and invalidates the previous retry', () => {
		const app = new AppState();
		app.error = 'Repeated error';
		app.errorRetry = vi.fn();
		const first = app.errorRevision;
		app.error = 'Repeated error';
		expect(app.errorRevision).toBe(first + 1);
		expect(app.errorRetry).toBeNull();
	});
});
