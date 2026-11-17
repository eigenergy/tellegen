import { SvelteSet } from 'svelte/reactivity';
import { errorText, type PowerIoDiagnostic, type SolveDiagnostic } from '@tellegen/engine';

/** How serious a finding is. PowerIO's `remark` and `note` both read as notes. */
export type LogSeverity = 'error' | 'warning' | 'note';
export const LOG_SEVERITIES: readonly LogSeverity[] = ['error', 'warning', 'note'];

/** One message to record. `stage` names what was happening (`Open`, `Solve`,
 * `Export`, or a host's own step such as `Generate`); `source` names who
 * reported it (`PowerIO`, `tellegen`, or a host's data source). */
export interface LogEntryInput {
	severity: LogSeverity;
	/** A few words naming what happened. Without one, views show the message. */
	title?: string;
	message: string;
	/** Defaults to `App`. */
	stage?: string;
	/** Defaults to `tellegen`. */
	source?: string;
	/** Stable machine code, such as `EMIT.MATPOWER.FIELD_DROPPED`. */
	code?: string;
	/** The case, file or download the finding belongs to. */
	subject?: string;
	/** The element the finding names, in PowerIO's spelling. */
	target?: string;
	buses?: number[];
	/** What to do about it, when the reporter says. */
	hint?: string;
	details?: Record<string, unknown>;
	/** Runs the failed operation again; views offer it as Retry. */
	retry?: () => void;
	/** False once `retry` no longer applies. */
	canRetry?: () => boolean;
}

export interface LogEntry extends LogEntryInput {
	stage: string;
	source: string;
	id: number;
	/** How many times the same finding was reported. */
	count: number;
	/** Epoch milliseconds of the first and the latest report. */
	first: number;
	last: number;
}

/** The log as a file: what `MessageLog.toJSON` returns and the log view downloads. */
export interface LogDocument {
	format: 'tellegen.log';
	version: 1;
	written_at: string;
	counts: Record<LogSeverity, number>;
	entries: (Omit<LogEntry, 'first' | 'last' | 'retry' | 'canRetry'> & {
		first: string;
		last: string;
	})[];
}

/** The messages on screen as a toast: one entry, or a group under one title. */
export interface LogToast {
	title: string;
	severity: LogSeverity;
	entries: LogEntry[];
}

const MAX_ENTRIES = 500;
const TOAST_MS = 6500;

/** What makes two reports the same message; taken from the entry as stored. */
const keyOf = (e: LogEntryInput) =>
	JSON.stringify([e.severity, e.stage, e.source, e.code, e.subject, e.target, e.title, e.message]);

/** Every message of the session, in the order each first appeared: errors,
 * warnings and notes from the app, the readers and the solvers. The Log lists
 * all of them; the toast shows the latest ones sent with `notify`. A message
 * reported again counts up in place instead of adding a row, so a re-solve or a
 * second export of the same case does not flood the log. */
export class MessageLog {
	entries: LogEntry[] = $state.raw<LogEntry[]>([]);
	/** Bumped on every write, including a repeat that only moves `last`. */
	revision = $state(0);
	/** What the toast last said, for a screen reader's live region. */
	announcement = $state('');
	#toast = $state.raw<{ ids: number[]; title?: string } | null>(null);
	#unseen = new SvelteSet<number>();
	#nextId = 0;
	#now: () => number;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#deadline = 0;
	#remaining = TOAST_MS;
	#paused = false;

	constructor(now: () => number = Date.now) {
		this.#now = now;
	}

	/** Record messages in the Log only, and return their ids. With
	 * `repeat: false`, a message already in the log keeps its count and takes the
	 * new details: use it for state that every solve restates, or to add the
	 * detail behind an error just recorded. */
	record(
		input: LogEntryInput | readonly LogEntryInput[],
		options: { repeat?: boolean } = {}
	): number[] {
		const inputs = Array.isArray(input) ? input : [input as LogEntryInput];
		const repeat = options.repeat ?? true;
		const now = this.#now();
		const next = [...this.entries];
		const index = new Map(next.map((entry, i) => [keyOf(entry), i]));
		const ids: number[] = [];
		for (const input of inputs) {
			if (!input.message) continue;
			const item = clean(input);
			const key = keyOf(item);
			const at = index.get(key);
			const previous = at === undefined ? undefined : next[at];
			const entry: LogEntry = previous
				? {
						...previous,
						...item,
						count: previous.count + (repeat ? 1 : 0),
						last: now
					}
				: {
						...item,
						id: ++this.#nextId,
						count: 1,
						first: now,
						last: now
					};
			if (at === undefined) index.set(key, next.push(entry) - 1);
			else next[at] = entry;
			ids.push(entry.id);
			if (entry.severity !== 'note' && (!previous || repeat)) this.#unseen.add(entry.id);
		}
		if (!ids.length) return ids;
		this.entries = next.length > MAX_ENTRIES ? next.slice(next.length - MAX_ENTRIES) : next;
		this.revision++;
		return ids;
	}

	/** Record messages and show them as the toast. Several messages show under
	 * `title`, which names the group ("3 import warnings"). */
	notify(
		input: LogEntryInput | readonly LogEntryInput[],
		options: { repeat?: boolean; title?: string } = {}
	) {
		const ids = this.record(input, options);
		if (!ids.length) return;
		this.#toast = { ids, title: options.title };
		const toast = this.toast;
		const count = toast?.entries.length === 1 ? toast.entries[0].count : 1;
		this.announcement = `${toast?.title}${count > 1 ? `, repeated ${count} times` : ''}`;
		this.#remaining = TOAST_MS;
		this.#schedule();
	}

	/** The newest messages sent with `notify`, until dismissed or timed out. */
	get toast(): LogToast | null {
		const shown = this.#toast;
		if (!shown) return null;
		const entries = this.entries.filter((entry) => shown.ids.includes(entry.id));
		if (!entries.length) return null;
		const severity = LOG_SEVERITIES.find((s) => entries.some((e) => e.severity === s))!;
		return {
			title: shown.title ?? entries[0].title ?? entries[0].message,
			severity,
			entries
		};
	}

	#schedule() {
		clearTimeout(this.#timer);
		if (this.#paused || !this.#toast) return;
		this.#deadline = Date.now() + this.#remaining;
		this.#timer = setTimeout(() => this.dismiss(), this.#remaining);
	}

	/** Hold the toast open while the reader is using it. */
	pause(value: boolean) {
		if (value === this.#paused) return;
		if (value) this.#remaining = Math.max(0, this.#deadline - Date.now());
		this.#paused = value;
		this.#schedule();
	}

	/** Close the toast. Its messages stay in the Log. */
	dismiss() {
		clearTimeout(this.#timer);
		this.#toast = null;
	}

	/** The Log is on screen: its tab stops counting what is already there. */
	markSeen() {
		this.#unseen.clear();
	}

	/** Errors and warnings recorded since `markSeen`; the Log tab's count. */
	get unseen(): number {
		return this.#unseen.size;
	}

	clear() {
		this.dismiss();
		this.markSeen();
		this.entries = [];
		this.revision++;
	}

	get counts(): Record<LogSeverity, number> {
		const counts = { error: 0, warning: 0, note: 0 };
		for (const entry of this.entries) counts[entry.severity]++;
		return counts;
	}

	toJSON(): LogDocument {
		return {
			format: 'tellegen.log',
			version: 1,
			written_at: new Date(this.#now()).toISOString(),
			counts: this.counts,
			entries: this.entries.map(({ retry: _retry, canRetry: _canRetry, ...entry }) => ({
				...entry,
				first: new Date(entry.first).toISOString(),
				last: new Date(entry.last).toISOString()
			}))
		};
	}
}

/** @deprecated Use `MessageLog`: the Log and the toast read one store. */
export class DiagnosticLog extends MessageLog {}

function clean(input: LogEntryInput): LogEntryInput & { stage: string; source: string } {
	const out: LogEntryInput & { stage: string; source: string } = {
		severity: input.severity,
		stage: input.stage ?? 'App',
		source: input.source ?? 'tellegen',
		message: input.message
	};
	if (input.title && input.title !== input.message) out.title = input.title;
	if (input.code) out.code = input.code;
	if (input.subject) out.subject = input.subject;
	if (input.target) out.target = input.target;
	if (input.buses?.length) out.buses = [...input.buses];
	if (input.hint) out.hint = input.hint;
	if (input.details && Object.keys(input.details).length) out.details = input.details;
	if (input.retry) out.retry = input.retry;
	if (input.canRetry) out.canRetry = input.canRetry;
	return out;
}

export function logSeverity(severity: PowerIoDiagnostic['severity']): LogSeverity {
	return severity === 'error' ? 'error' : severity === 'warning' ? 'warning' : 'note';
}

interface EntryContext {
	stage: string;
	subject?: string;
}

/** A PowerIO reader or writer finding as a log entry. */
export function powerIoEntry(d: PowerIoDiagnostic, context: EntryContext): LogEntryInput {
	return {
		severity: logSeverity(d.severity),
		stage: context.stage,
		source: 'PowerIO',
		code: d.code,
		message: d.message,
		subject: context.subject,
		target: d.target,
		hint: d.suggested_action,
		details: d.details
	};
}

// Solve findings that change what the numbers mean; the rest say how the
// solver read the case.
const SOLVE_WARNINGS = new Set([
	'island_deenergized',
	'BUILD.HVDC.TERMINAL_INACTIVE',
	'settings_ignored'
]);

/** A solver finding (islands, HVDC transfers) as a log entry. */
export function solveEntry(d: SolveDiagnostic, context: { subject?: string }): LogEntryInput {
	return {
		severity: SOLVE_WARNINGS.has(d.code) ? 'warning' : 'note',
		stage: 'Solve',
		source: 'tellegen',
		code: d.code,
		message: d.message,
		subject: context.subject,
		buses: d.buses
	};
}

/** The title of an error that says nothing more about itself. */
export const ERROR_TITLE = 'Something went wrong';

/** Titles for the machine-readable `code` an engine error carries. */
const ERROR_TITLES: Record<string, string> = {
	invalid_input: 'Invalid input',
	unsupported: 'Not supported',
	infeasible: 'No solution',
	not_converged: 'Did not converge',
	cancelled: 'Cancelled',
	internal: ERROR_TITLE
};

const codeOf = (error: unknown) => {
	const code = (error as { code?: unknown } | null)?.code;
	return typeof code === 'string' ? code : undefined;
};

/** An error's title: the one for its `code` when it has a known one, else `fallback`. */
export function errorTitle(error: unknown, fallback = ERROR_TITLE): string {
	// `NotConverged`, `not-converged` and `not_converged` are one code.
	const key = codeOf(error)
		?.replace(/([a-z])([A-Z])/g, '$1_$2')
		.toLowerCase()
		.replace(/[^a-z]+/g, '_');
	return (key && ERROR_TITLES[key]) || fallback;
}

/** What the reporter of an error knows about it. */
export type ErrorContext = Partial<Omit<LogEntryInput, 'severity' | 'details'>> & {
	/** Technical text behind a message written for people. */
	detail?: string | null;
};

/** An error as a log entry. A thrown error takes the title for its `code`; text
 * the caller wrote is its own title. `known` overrides either, and its
 * `message` replaces the error's own text. */
export function errorEntry(error: unknown, known?: ErrorContext | string | null): LogEntryInput {
	const { detail, ...context } = typeof known === 'object' && known ? known : { detail: known };
	return {
		...context,
		severity: 'error',
		title: context.title ?? (typeof error === 'string' ? undefined : errorTitle(error)),
		code: context.code ?? codeOf(error),
		message: context.message ?? errorText(error),
		details: detail ? { detail } : undefined
	};
}

/** @deprecated Reporters name the step themselves (`LogEntryInput.stage`). */
export function errorStage(_message: string): string {
	return 'App';
}

/** @deprecated Entries carry their own `title`; this returns the first line. */
export function logTitle(message: string): string {
	return message.split('\n')[0].trim();
}
