import { getContext, setContext } from 'svelte';
import type { LogEntryInput } from './log.svelte.js';

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
export interface Notice extends NoticeInput {
	id: number;
	count: number;
	updatedAt: number;
}
const KEY = Symbol('tellegen.notices');
const LIFETIME_MS = 6500;
const MAX_NOTICES = 30;

/** Recent messages stay in memory; dismissal hides the temporary notice only. */
export class NoticeCenter {
	entries: Notice[] = $state.raw<Notice[]>([]);
	activeId: number | null = $state(null);
	expanded = $state(false);
	announcement = $state('');
	#nextId = 0;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#deadline = 0;
	#remaining = LIFETIME_MS;
	#paused = false;
	get active() {
		return this.entries.find((entry) => entry.id === this.activeId) ?? null;
	}
	push(input: NoticeInput) {
		if (!input.details) return;
		const previous = this.entries.find(
			(entry) => entry.details === input.details && entry.kind === input.kind
		);
		const entry: Notice = {
			...input,
			id: previous?.id ?? ++this.#nextId,
			count: (previous?.count ?? 0) + 1,
			updatedAt: Date.now()
		};
		this.entries = [entry, ...this.entries.filter((item) => item.id !== entry.id)].slice(
			0,
			MAX_NOTICES
		);
		if (input.quiet) return;
		this.activeId = entry.id;
		this.announcement = `${entry.title}${entry.count > 1 ? `, repeated ${entry.count} times` : ''}`;
		this.#remaining = LIFETIME_MS;
		this.#schedule();
	}
	#schedule() {
		clearTimeout(this.#timer);
		if (this.#paused || this.activeId === null) return;
		this.#deadline = Date.now() + this.#remaining;
		this.#timer = setTimeout(() => this.dismiss(), this.#remaining);
	}
	pause(value: boolean) {
		if (value === this.#paused) return;
		if (value) this.#remaining = Math.max(0, this.#deadline - Date.now());
		this.#paused = value;
		this.#schedule();
	}
	dismiss() {
		clearTimeout(this.#timer);
		this.activeId = null;
	}
	clear() {
		this.dismiss();
		this.entries = [];
		this.expanded = false;
	}
	dispose() {
		clearTimeout(this.#timer);
	}
}
export const setNoticeCenter = (value: NoticeCenter) => setContext(KEY, value);
export function getNoticeCenter(): NoticeCenter {
	const value = getContext<NoticeCenter | undefined>(KEY);
	if (!value) throw new Error('NoticeCenter requires a Tellegen provider');
	return value;
}
/** Messages that tell the reader what to change: running the same thing
 * again cannot succeed, so their notices offer no Retry. */
const INSTRUCTIONS = [
	/experimental\. Turn it on in Settings/i,
	/is not a case or geographic file|does not hold a complete network|could not (parse|read)|unsupported format|incomplete network|ambiguous format/i,
	// A reader's finding on a malformed file: the same file fails the same way again.
	/\bPARSE\.[A-Z_.]+|malformed|missing required/i
];
export function errorNoticeCanRetry(details: string): boolean {
	return !INSTRUCTIONS.some((pattern) => pattern.test(details));
}
/** The case or map file a message names, if any. */
function fileOf(details: string): string | null {
	return (
		/([\w.-]+\.(?:m|raw|rawx|aux|epc|pwb|pwd|dss|dyr|csv|json|geojson|xml|zip))\b/i.exec(
			details
		)?.[1] ?? null
	);
}
/** A notice's title: a few words naming what failed. The full message is in Details. */
export function errorNoticeTitle(details: string): string {
	if (/^Case list:/.test(details)) return 'Could not load the case list';
	if (/dynamics.*is experimental\. Turn it on in Settings/i.test(details))
		return 'Turn on Experimental features in Settings';
	if (/^ZIP and folder imports are unavailable in this build/.test(details))
		return 'Imports unavailable';
	if (/rate limit|limiting requests/i.test(details)) return 'Rate limited';
	// MapLibre's WebGL failure includes "Read more"; classify our map errors
	// before the generic file/read fallback so it does not blame a case file.
	if (/^map failed to load:|^the map lost its graphics context/i.test(details))
		return 'Map failed to start';
	if (/storage|quota|space|indexeddb/i.test(details)) return 'Storage full';
	// A singular matrix on its own is a solve failure (an AC power flow Jacobian,
	// say); only a sensitivity that fails is about prices.
	if (/sensitivity|derivative/i.test(details)) return 'No price sensitivity';
	if (
		/infeasib|converg|singular|jacobian|solver|solving|calculation|compute|webassembly|power flow|\bstudy\b/i.test(
			details
		)
	)
		return 'Calculation failed';
	if (/parse|read|file|format/i.test(details)) {
		const file = fileOf(details);
		return file ? `Could not open ${file}` : 'Unreadable file';
	}
	return firstWords(details);
}
/** The start of an unclassified message, so its notice still names what failed. */
function firstWords(details: string): string {
	const line = details
		.split(/\n|(?<=[.:;])\s/)[0]
		.replace(/[.:;]$/, '')
		.trim();
	if (!line) return 'Error';
	return line.length > 48 ? `${line.slice(0, 47).trimEnd()}…` : line;
}
