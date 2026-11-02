import type { PowerIoDiagnostic, SolveDiagnostic } from '@tellegen/engine';

/** How serious a finding is. PowerIO's `remark` and `note` both read as notes. */
export type LogSeverity = 'error' | 'warning' | 'note';
export const LOG_SEVERITIES: readonly LogSeverity[] = ['error', 'warning', 'note'];

/** One finding to record. `stage` names what was happening (`Open`, `Solve`,
 * `Export`, or a host's own step such as `Generate`); `source` names who
 * reported it (`PowerIO`, `tellegen`, or a host's data source). */
export interface LogEntryInput {
	severity: LogSeverity;
	stage: string;
	source: string;
	message: string;
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
}

export interface LogEntry extends LogEntryInput {
	id: number;
	/** How many times the same finding was reported. */
	count: number;
	/** Epoch milliseconds of the first and the latest report. */
	first: number;
	last: number;
}

/** The log as a file: what `DiagnosticLog.toJSON` returns and the log view downloads. */
export interface LogDocument {
	format: 'tellegen.log';
	version: 1;
	written_at: string;
	counts: Record<LogSeverity, number>;
	entries: (Omit<LogEntry, 'first' | 'last'> & {
		first: string;
		last: string;
	})[];
}

const MAX_ENTRIES = 500;

const keyOf = (e: LogEntryInput) =>
	JSON.stringify([e.severity, e.stage, e.source, e.code, e.subject, e.target, e.message]);

/** Every finding the app reported, in the order it first appeared. A finding
 * reported again counts up in place instead of adding a row, so a re-solve or a
 * second export of the same case does not flood the log. */
export class DiagnosticLog {
	entries: LogEntry[] = $state.raw<LogEntry[]>([]);
	/** Bumped on every write, including a repeat that only moves `last`. */
	revision = $state(0);
	#nextId = 0;
	#now: () => number;

	constructor(now: () => number = Date.now) {
		this.#now = now;
	}

	/** Record findings. With `repeat: false`, a finding already in the log keeps
	 * its count and takes the new details: use it for state that every solve
	 * restates, or to add the detail behind an error just recorded. */
	record(input: LogEntryInput | readonly LogEntryInput[], options: { repeat?: boolean } = {}) {
		const inputs = Array.isArray(input) ? input : [input as LogEntryInput];
		if (!inputs.length) return;
		const repeat = options.repeat ?? true;
		const now = this.#now();
		const next = [...this.entries];
		const index = new Map(next.map((entry, i) => [keyOf(entry), i]));
		for (const item of inputs) {
			if (!item.message) continue;
			const key = keyOf(item);
			const at = index.get(key);
			if (at !== undefined) {
				const previous = next[at];
				next[at] = repeat
					? { ...previous, count: previous.count + 1, last: now }
					: { ...previous, ...clean(item), last: now };
				continue;
			}
			index.set(key, next.length);
			next.push({
				...clean(item),
				id: ++this.#nextId,
				count: 1,
				first: now,
				last: now
			});
		}
		this.entries = next.length > MAX_ENTRIES ? next.slice(next.length - MAX_ENTRIES) : next;
		this.revision++;
	}

	clear() {
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
			entries: this.entries.map((entry) => ({
				...entry,
				first: new Date(entry.first).toISOString(),
				last: new Date(entry.last).toISOString()
			}))
		};
	}
}

function clean(input: LogEntryInput): LogEntryInput {
	const out: LogEntryInput = {
		severity: input.severity,
		stage: input.stage,
		source: input.source,
		message: input.message
	};
	if (input.code) out.code = input.code;
	if (input.subject) out.subject = input.subject;
	if (input.target) out.target = input.target;
	if (input.buses?.length) out.buses = [...input.buses];
	if (input.hint) out.hint = input.hint;
	if (input.details && Object.keys(input.details).length) out.details = input.details;
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
const SOLVE_WARNINGS = new Set(['island_deenergized', 'BUILD.HVDC.TERMINAL_INACTIVE']);

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

/** Which step an app error message comes from, by its wording. */
export function errorStage(message: string): string {
	if (/export|could not save|download/i.test(message)) return 'Export';
	if (/coordinat|geographic|layout|placement|placed/i.test(message)) return 'Coordinates';
	if (
		/infeasib|converg|solv|OPF|power flow|calculation|sensitivity|webassembly|study/i.test(message)
	)
		return 'Solve';
	if (/pars|read|file|format|module/i.test(message)) return 'Open';
	return 'App';
}

/** An app error as a log entry. */
export function errorEntry(message: string, detail?: string | null): LogEntryInput {
	return {
		severity: 'error',
		stage: errorStage(message),
		source: 'tellegen',
		message,
		details: detail ? { detail } : undefined
	};
}
