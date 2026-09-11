import { afterEach, describe, expect, it, vi } from 'vitest';
import { NoticeCenter } from '../src/lib/notices.svelte.js';
import { AppState } from '../src/lib/state.svelte.js';

afterEach(() => vi.useRealTimers());
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
		notices.push({ title: 'Could not save locally', details: 'Storage details' });
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
