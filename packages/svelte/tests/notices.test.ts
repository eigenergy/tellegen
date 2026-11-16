import { afterEach, describe, expect, it, vi } from 'vitest';
import { ERROR_TITLE, MessageLog, errorEntry, errorTitle } from '../src/lib/log.svelte.js';
import { NoticeCenter } from '../src/lib/notices.svelte.js';
import { AppState } from '../src/lib/state.svelte.js';

afterEach(() => vi.useRealTimers());

const coded = (code: string) => Object.assign(new Error('engine text'), { code });

describe('one store behind the toast and the Log', () => {
	it('shows a notified entry in both, and keeps it in the Log after the toast closes', () => {
		vi.useFakeTimers();
		const log = new MessageLog();
		log.notify({
			severity: 'error',
			title: 'DC OPF failed',
			message: 'Full solver error'
		});
		expect(log.entries).toHaveLength(1);
		expect(log.toast).toMatchObject({
			title: 'DC OPF failed',
			severity: 'error'
		});
		expect(log.toast?.entries[0]).toBe(log.entries[0]);
		vi.advanceTimersByTime(6500);
		expect(log.toast).toBeNull();
		expect(log.entries[0]).toMatchObject({
			title: 'DC OPF failed',
			message: 'Full solver error'
		});
	});

	it('counts a repeat on the same entry and in the toast', () => {
		const log = new MessageLog();
		// A title that only repeats the message is the same report as no title.
		log.notify({ severity: 'error', message: 'Rate limited' });
		log.notify({ severity: 'error', title: 'Rate limited', message: 'Rate limited' });
		expect(log.entries).toHaveLength(1);
		expect(log.toast?.entries[0].count).toBe(2);
		expect(log.announcement).toBe('Rate limited, repeated 2 times');
		log.dismiss();
	});

	it('records without a toast', () => {
		const log = new MessageLog();
		log.record({
			severity: 'error',
			title: 'Could not load the case list',
			message: '502'
		});
		expect(log.entries).toHaveLength(1);
		expect(log.toast).toBeNull();
	});

	it('shows several entries under one title, at the worst severity among them', () => {
		const log = new MessageLog();
		log.notify(
			[
				{ severity: 'note', message: 'a note' },
				{ severity: 'warning', message: 'a warning' }
			],
			{ title: '1 import warning' }
		);
		expect(log.toast).toMatchObject({
			title: '1 import warning',
			severity: 'warning'
		});
		expect(log.toast?.entries.map((entry) => entry.message)).toEqual(['a note', 'a warning']);
		log.dismiss();
	});

	it('counts unseen errors and warnings once each, and not notes', () => {
		const log = new MessageLog();
		log.record([
			{ severity: 'error', message: 'failed' },
			{ severity: 'warning', message: 'dropped' },
			{ severity: 'note', message: 'placed' }
		]);
		log.record({ severity: 'error', message: 'failed' });
		expect(log.unseen).toBe(2);
		log.markSeen();
		expect(log.unseen).toBe(0);
		// A restated finding is not news; a repeated one is.
		log.record({ severity: 'warning', message: 'dropped' }, { repeat: false });
		expect(log.unseen).toBe(0);
		log.record({ severity: 'warning', message: 'dropped' });
		expect(log.unseen).toBe(1);
		log.clear();
		expect(log.unseen).toBe(0);
	});

	it('pauses the toast during interaction and restarts the remaining time', () => {
		vi.useFakeTimers();
		const log = new MessageLog();
		log.notify({ severity: 'warning', message: 'Could not save locally' });
		vi.advanceTimersByTime(2000);
		log.pause(true);
		vi.advanceTimersByTime(30_000);
		expect(log.toast).not.toBeNull();
		log.pause(false);
		vi.advanceTimersByTime(4499);
		expect(log.toast).not.toBeNull();
		vi.advanceTimersByTime(1);
		expect(log.toast).toBeNull();
	});

	it('leaves retry callbacks out of the saved log', () => {
		const log = new MessageLog();
		log.record({ severity: 'error', message: 'failed', retry: () => {} });
		expect(log.toJSON().entries[0]).not.toHaveProperty('retry');
	});
});

describe('error titles', () => {
	it.each([
		['invalid_input', 'Invalid input'],
		['unsupported', 'Not supported'],
		['infeasible', 'No solution'],
		['not_converged', 'Did not converge'],
		['NotConverged', 'Did not converge'],
		['not-converged', 'Did not converge'],
		['cancelled', 'Cancelled'],
		['internal', ERROR_TITLE]
	])('maps the code %s to "%s"', (code, title) => {
		expect(errorTitle(coded(code))).toBe(title);
		expect(errorEntry(coded(code))).toMatchObject({
			title,
			code,
			message: 'engine text'
		});
	});

	it('gives an error without a code, or with an unknown one, the neutral title', () => {
		expect(errorEntry(new Error('singular Jacobian')).title).toBe(ERROR_TITLE);
		expect(errorEntry(coded('something_new')).title).toBe(ERROR_TITLE);
		expect(errorTitle(new Error('storage quota'), 'Could not save')).toBe('Could not save');
	});

	it("uses the caller's title over the code, and text the caller wrote as its own title", () => {
		expect(errorEntry(coded('infeasible'), { title: 'AC OPF failed' }).title).toBe('AC OPF failed');
		const entry = errorEntry('Run power flow first');
		expect(entry.title).toBeUndefined();
		expect(entry.message).toBe('Run power flow first');
	});
});

describe('AppState errors', () => {
	it('sends an error to the toast and the Log as one entry', () => {
		const app = new AppState();
		app.reportError(coded('not_converged'), {
			stage: 'Solve',
			subject: 'case9'
		});
		expect(app.error).toBe('engine text');
		expect(app.log.entries).toHaveLength(1);
		expect(app.log.toast?.entries[0]).toBe(app.log.entries[0]);
		expect(app.log.entries[0]).toMatchObject({
			title: 'Did not converge',
			code: 'not_converged',
			stage: 'Solve',
			subject: 'case9'
		});
		expect(app.log.unseen).toBe(1);
		app.log.dismiss();
	});

	it('keeps a quiet error out of the toast', () => {
		const app = new AppState();
		app.reportError('Case list: 502', {
			title: 'Could not load the case list',
			quiet: true
		});
		expect(app.log.entries).toHaveLength(1);
		expect(app.log.toast).toBeNull();
	});

	it('offers Retry only while the error is current and has one', () => {
		const app = new AppState();
		const retry = vi.fn();
		app.error = 'Plain message';
		expect(app.log.entries[0].canRetry?.()).toBe(false);
		app.reportError('Rate limited', { retry });
		const entry = app.log.entries[1];
		expect(entry.canRetry?.()).toBe(true);
		entry.retry?.();
		expect(retry).toHaveBeenCalledOnce();
		expect(app.error).toBeNull();
		expect(entry.canRetry?.()).toBe(false);
		app.log.dismiss();
	});

	it('counts identical error writes and invalidates the previous retry', () => {
		const app = new AppState();
		app.error = 'Repeated error';
		app.errorRetry = vi.fn();
		const first = app.errorRevision;
		app.error = 'Repeated error';
		expect(app.errorRevision).toBe(first + 1);
		expect(app.errorRetry).toBeNull();
		expect(app.log.entries).toHaveLength(1);
		expect(app.log.entries[0].count).toBe(2);
		app.log.dismiss();
	});
});

describe('the deprecated NoticeCenter', () => {
	it('writes to the same store', () => {
		const log = new MessageLog();
		const notices = new NoticeCenter(log);
		notices.push({
			kind: 'error',
			title: 'Calculation failed',
			details: 'Full solver error'
		});
		notices.push({ title: 'Logged only', details: 'quiet text', quiet: true });
		expect(log.entries.map((entry) => entry.title)).toEqual(['Calculation failed', 'Logged only']);
		expect(notices.active).toMatchObject({
			title: 'Calculation failed',
			kind: 'error'
		});
		expect(notices.entries[0].details).toBe('quiet text');
		notices.dismiss();
		expect(log.toast).toBeNull();
	});
});
