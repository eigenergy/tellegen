import { getContext, setContext } from 'svelte';

export interface NoticeInput {
	title: string;
	details: string;
	kind?: 'error' | 'warning';
	retry?: () => void;
	canRetry?: () => boolean;
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
export function errorNoticeTitle(details: string): string {
	if (/rate limit/i.test(details)) return 'Too many requests';
	if (/storage|quota|space|indexeddb/i.test(details)) return 'Could not save locally';
	if (/sensitivity|derivative|singular/i.test(details)) return 'Sensitivity unavailable';
	if (/infeasib|converg|solver|solving|calculation|compute|webassembly/i.test(details))
		return 'Calculation did not complete';
	if (/parse|read|file|format/i.test(details)) return 'Could not read this file';
	return 'Operation did not complete';
}
