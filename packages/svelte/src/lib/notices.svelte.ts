import { getAppState } from './context.svelte.js';
import { ERROR_TITLE, MessageLog, type LogEntry, type LogEntryInput } from './log.svelte.js';

/** @deprecated Record a `LogEntryInput` with `app.log.notify` or `app.log.record`. */
export interface NoticeInput {
	title: string;
	details: string;
	/** Structured findings behind `details`; the notice lists them when present. */
	entries?: LogEntryInput[];
	kind?: 'error' | 'warning';
	retry?: () => void;
	canRetry?: () => boolean;
	/** Record the message in the Log only: the page already shows it where it happened. */
	quiet?: boolean;
}
/** @deprecated Read `LogEntry` values from `app.log`. */
export interface Notice extends NoticeInput {
	id: number;
	count: number;
	updatedAt: number;
}

const notice = (entry: LogEntry): Notice => ({
	id: entry.id,
	count: entry.count,
	updatedAt: entry.last,
	title: entry.title ?? entry.message,
	details: entry.message,
	kind: entry.severity === 'error' ? 'error' : 'warning',
	retry: entry.retry,
	canRetry: entry.canRetry
});

/** @deprecated The toast and the Log read one store, `app.log` (a `MessageLog`).
 * This keeps the old calls working on top of it. */
export class NoticeCenter {
	expanded = $state(false);
	#log: MessageLog;
	constructor(log: MessageLog = new MessageLog()) {
		this.#log = log;
	}
	get entries(): Notice[] {
		return this.#log.entries.map(notice).reverse();
	}
	get active(): Notice | null {
		const toast = this.#log.toast;
		return toast ? { ...notice(toast.entries[0]), title: toast.title } : null;
	}
	get activeId(): number | null {
		return this.active?.id ?? null;
	}
	get announcement(): string {
		return this.#log.announcement;
	}
	push(input: NoticeInput) {
		if (!input.details) return;
		const { title, details: message, retry, canRetry } = input;
		const severity = input.kind ?? 'warning';
		if (input.entries?.length) this.#log.notify(input.entries, { title });
		else if (input.quiet) this.#log.record({ severity, title, message, retry, canRetry });
		else this.#log.notify({ severity, title, message, retry, canRetry });
	}
	pause(value: boolean) {
		this.#log.pause(value);
	}
	dismiss() {
		this.#log.dismiss();
	}
	clear() {
		this.#log.clear();
		this.expanded = false;
	}
	dispose() {
		this.#log.dismiss();
	}
}
/** @deprecated Use `getAppState().log`. */
export function getNoticeCenter(): NoticeCenter {
	return new NoticeCenter(getAppState().log);
}
/** @deprecated Reporters give each message its own title; nothing reads one out of the text. */
export function errorNoticeTitle(_details: string): string {
	return ERROR_TITLE;
}
