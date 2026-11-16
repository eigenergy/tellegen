import { afterEach, describe, expect, it, vi } from 'vitest';
import { errorNoticeCanRetry, errorNoticeTitle, NoticeCenter } from '../src/lib/notices.svelte.js';
import { AppState } from '../src/lib/state.svelte.js';

afterEach(() => vi.useRealTimers());
describe('error titles', () => {
	it.each([
		'map failed to load: Failed to initialize WebGL2. Read more at the browser support page',
		'the map lost its graphics context repeatedly; reload the page to restore it'
	])('labels graphics failures as map errors: %s', (details) => {
		expect(errorNoticeTitle(details)).toBe('Map failed to start');
	});
	it('identifies an unsupported AC power flow boundary as a calculation failure', () => {
		expect(
			errorNoticeTitle(
				'Texas7k: AC power flow reference bus 111333 states angle -11.99 degrees; Tellegen currently requires zero'
			)
		).toBe('Calculation failed');
	});
	it('says how to turn on dynamics, and offers no retry that cannot succeed', () => {
		const details =
			'Dynamics (RMS) simulation is experimental. Turn it on in Settings, under Experimental features.';
		expect(errorNoticeTitle(details)).toBe('Turn on Experimental features in Settings');
		expect(errorNoticeCanRetry(details)).toBe(false);
		expect(errorNoticeCanRetry('sensitivity rate limit exceeded')).toBe(true);
	});
	it('treats a singular Jacobian as a calculation failure, not a price sensitivity one', () => {
		expect(errorNoticeTitle('AC power flow failed: singular Jacobian at iteration 3')).toBe(
			'Calculation failed'
		);
		expect(errorNoticeTitle('sensitivity unavailable: singular basis')).toBe(
			'No price sensitivity'
		);
	});
	it('names the file in a file error, and offers no retry for one', () => {
		expect(errorNoticeTitle('Could not parse case.m: invalid bus row')).toBe(
			'Could not open case.m'
		);
		expect(errorNoticeTitle('notes.txt: unsupported format')).toBe('Unreadable file');
		expect(errorNoticeCanRetry('grid.raw: unsupported format')).toBe(false);
		// A malformed file fails the same way again: no Retry.
		expect(
			errorNoticeCanRetry(
				'broken.m: PARSE.MATPOWER.MALFORMED: missing required MATPOWER field `baseMVA`'
			)
		).toBe(false);
	});
	it('names the calculation that has no solution', () => {
		expect(errorNoticeTitle('Portugal: DC OPF has no solution at this demand')).toBe(
			'DC OPF has no solution'
		);
		expect(errorNoticeTitle('AC OPF: problem infeasible')).toBe('AC OPF has no solution');
	});
	it('titles a case message with what happened, not the case name', () => {
		expect(errorNoticeTitle('IEEE 14: solve again before saving')).toBe(
			'Solve again before saving'
		);
	});
	it('names a failed case list', () => {
		expect(errorNoticeTitle('Case list: Server error')).toBe('Could not load the case list');
		expect(errorNoticeCanRetry('Case list: Server error')).toBe(true);
	});
	it('never titles a notice with a bare "Failed"', () => {
		expect(errorNoticeTitle('PowerIO balanced_network input cannot start a DcPf Study')).toBe(
			'Calculation failed'
		);
		expect(errorNoticeTitle('Clipboard denied. The page lost focus')).toBe('Clipboard denied');
		expect(
			errorNoticeTitle('An unclassified message that runs on far longer than a notice title should')
		).toBe('An unclassified message that runs on far longer…');
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
	it('records a quiet message in the Log without a toast', () => {
		const notices = new NoticeCenter();
		notices.push({
			title: 'Could not load the case list',
			details: 'Case list: 502',
			quiet: true
		});
		expect(notices.entries).toHaveLength(1);
		expect(notices.active).toBeNull();
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
