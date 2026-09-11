export const UMAMI_SCRIPT = 'https://cloud.umami.is/script.js';
export const UMAMI_WEBSITE = '41f22b57-b287-42b2-935c-037f82a80020';
export const OPT_OUT_KEY = 'umami.disabled';
const HOSTS = new Set(['tellegen.dev', 'www.tellegen.dev']);
const PAGES: Record<string, string> = {
	'/': 'Tellegen',
	'/privacy/': 'Privacy',
	'/credits/': 'Credits',
	'/changelog/': 'Changelog'
};
const EVENTS = new Set([
	'case.select',
	'case.import',
	'case.edit',
	'calculation.solve',
	'calculation.sensitivity',
	'view.change',
	'study.save',
	'study.open',
	'study.import',
	'study.export',
	'study.operation',
	'agent.call',
	'agent.availability'
]);
const ENUMS: Record<string, ReadonlySet<string>> = {
	source: new Set(['demo', 'local', 'saved', 'distribution']),
	demo_case: new Set(['case200', 'case500', 'case7000', 'cats']),
	calculation: new Set(['dcopf', 'acpf', 'socwr', 'acopf', 'mcpf']),
	backend: new Set(['browser', 'server', 'cached']),
	result: new Set([
		'completed',
		'failed',
		'cancelled',
		'unavailable',
		'infeasible',
		'no_improvement',
		'improved'
	]),
	view: new Set(['price', 'angle', 'voltage', 'geographic', 'diagram']),
	edit: new Set(['demand', 'capacity']),
	operation: new Set([
		'branch',
		'compare',
		'revise_goal',
		'record_evidence',
		'plan',
		'challenge',
		'edit_demand',
		'restore_base',
		'apply',
		'inspect',
		'propose',
		'solve'
	]),
	tool: new Set([
		'inspect_case',
		'solve_multiconductor_pf',
		'list_cases',
		'select_case',
		'query_network',
		'focus_network',
		'update_case',
		'analyze_sensitivity',
		'preview_case_update',
		'reset_case',
		'propose_capacity_plan',
		'apply_capacity_plan',
		'create_study',
		'inspect_study',
		'revise_study_goal',
		'branch_study',
		'compare_study_states',
		'propose_study',
		'apply_study_proposal',
		'edit_demand',
		'restore_base_case',
		'record_study_evidence'
	])
};
const NUMBERS = new Set([
	'duration_ms',
	'solve_ms',
	'iterations',
	'solve_count',
	'trial_count',
	'item_count',
	'changed_count'
]);
const PROPERTIES: Record<string, readonly string[]> = {
	'case.select': ['source', 'demo_case'],
	'case.import': ['result', 'duration_ms', 'item_count'],
	'case.edit': ['source', 'demo_case', 'edit', 'changed_count'],
	'calculation.solve': [
		'source',
		'demo_case',
		'calculation',
		'backend',
		'result',
		'duration_ms',
		'solve_ms',
		'iterations'
	],
	'calculation.sensitivity': ['source', 'demo_case', 'calculation', 'result', 'duration_ms'],
	'view.change': ['view'],
	'study.save': ['result', 'duration_ms'],
	'study.open': ['result', 'duration_ms'],
	'study.import': ['result', 'duration_ms'],
	'study.export': ['result'],
	'study.operation': ['operation', 'result', 'duration_ms', 'solve_count', 'trial_count'],
	'agent.call': ['tool', 'result', 'duration_ms'],
	'agent.availability': ['result']
};
export type AnalyticsEvent = keyof typeof PROPERTIES;
export type EventData = Record<string, string | number | undefined>;
interface Umami {
	track(payload: Record<string, unknown>): void | Promise<unknown>;
}
interface AnalyticsWindow extends Window {
	umami?: Umami;
	tellegenBeforeAnalyticsSend?: (
		type: string,
		payload: Record<string, unknown>
	) => Record<string, unknown> | false;
}
export function knownPage(value: unknown): string {
	if (typeof value !== 'string') return '/';
	try {
		const path = new URL(value, 'https://tellegen.dev').pathname;
		return path in PAGES ? path : '/';
	} catch {
		return '/';
	}
}
export function eventData(
	name: string,
	input: Record<string, unknown>
): Record<string, string | number> {
	const output: Record<string, string | number> = {};
	for (const key of PROPERTIES[name] ?? []) {
		const value = input[key];
		if (typeof value === 'string' && ENUMS[key]?.has(value)) output[key] = value;
		else if (NUMBERS.has(key) && typeof value === 'number' && Number.isFinite(value) && value >= 0)
			output[key] = Math.min(86_400_000, Math.round(value));
	}
	if (output.source !== 'demo') delete output.demo_case;
	return output;
}

/** Every outgoing payload is reconstructed from fixed names and coarse measurements. */
export class AnalyticsClient {
	#window: AnalyticsWindow;
	#script: HTMLScriptElement | null = null;
	#queue: Record<string, unknown>[] = [];
	#listeners = new Set<() => void>();
	#disabled = false;
	constructor(window: Window) {
		this.#window = window as AnalyticsWindow;
	}
	get blockedByBrowser(): boolean {
		const nav = this.#window.navigator as Navigator & {
			msDoNotTrack?: string;
			globalPrivacyControl?: boolean;
		};
		return (
			['1', 'yes'].includes(nav.doNotTrack ?? nav.msDoNotTrack ?? '') ||
			nav.globalPrivacyControl === true
		);
	}
	get optedOut(): boolean {
		try {
			return this.#disabled || !!this.#window.localStorage.getItem(OPT_OUT_KEY);
		} catch {
			return true;
		}
	}
	get allowed(): boolean {
		return (
			this.#window.location.protocol === 'https:' &&
			HOSTS.has(this.#window.location.hostname) &&
			!this.blockedByBrowser &&
			!this.optedOut
		);
	}
	subscribe(callback: () => void) {
		this.#listeners.add(callback);
		return () => {
			this.#listeners.delete(callback);
		};
	}
	setOptOut(disabled: boolean) {
		this.#disabled = disabled;
		try {
			if (disabled) this.#window.localStorage.setItem(OPT_OUT_KEY, '1');
			else this.#window.localStorage.removeItem(OPT_OUT_KEY);
		} catch {
			this.#disabled = true;
		}
		if (disabled) this.#queue = [];
		else this.start();
		for (const callback of this.#listeners) callback();
	}
	sanitize(type: string, payload: Record<string, unknown>): Record<string, unknown> | false {
		if (!this.allowed || !['event', 'performance'].includes(type)) return false;
		const path = knownPage(payload.url);
		const safe: Record<string, unknown> = {
			website: UMAMI_WEBSITE,
			hostname: this.#window.location.hostname,
			url: path,
			title: PAGES[path],
			referrer: ''
		};
		if (type === 'performance') {
			for (const metric of ['ttfb', 'fcp', 'lcp', 'cls', 'inp', 'duration']) {
				const value = payload[metric];
				if (typeof value === 'number' && Number.isFinite(value) && value >= 0)
					safe[metric] =
						metric === 'cls'
							? Math.min(100, Math.round(value * 10000) / 10000)
							: Math.min(86_400_000, Math.round(value));
			}
			return safe;
		}
		if (payload.name === undefined) return safe;
		if (typeof payload.name !== 'string' || !EVENTS.has(payload.name)) return false;
		safe.name = payload.name;
		safe.data = eventData(
			payload.name,
			payload.data && typeof payload.data === 'object'
				? (payload.data as Record<string, unknown>)
				: {}
		);
		return safe;
	}
	start() {
		if (!this.allowed || this.#script) return;
		const win = this.#window;
		win.tellegenBeforeAnalyticsSend = (type, payload) => this.sanitize(type, payload);
		const script = win.document.createElement('script');
		script.src = UMAMI_SCRIPT;
		script.defer = true;
		script.dataset.websiteId = UMAMI_WEBSITE;
		script.dataset.domains = [...HOSTS].join(',');
		script.dataset.autoPageview = 'false';
		script.dataset.performance = 'true';
		script.dataset.doNotTrack = 'true';
		script.dataset.excludeSearch = 'true';
		script.dataset.excludeHash = 'true';
		script.dataset.beforeSend = 'tellegenBeforeAnalyticsSend';
		script.referrerPolicy = 'no-referrer';
		script.onload = () => {
			const queued = this.#queue;
			this.#queue = [];
			for (const payload of queued) this.#send(payload);
		};
		script.onerror = () => {
			this.#queue = [];
		};
		this.#script = script;
		win.document.head.appendChild(script);
	}
	#send(payload: Record<string, unknown>) {
		if (!this.allowed) return;
		try {
			if (this.#window.umami) {
				Promise.resolve(this.#window.umami.track(payload)).catch(() => {});
			} else this.#queue = [...this.#queue, payload].slice(-30);
		} catch {
			/* Analytics failures do not alter application state. */
		}
	}
	page(path: string) {
		this.#send({ website: UMAMI_WEBSITE, url: knownPage(path) });
	}
	track(name: string, data: EventData = {}) {
		if (!EVENTS.has(name)) return;
		const safe = this.sanitize('event', { name, data, url: this.#window.location.pathname });
		if (safe) this.#send(safe);
	}
}
let client: AnalyticsClient | null = null;
export function browserAnalytics(): AnalyticsClient | null {
	if (typeof window === 'undefined') return null;
	return (client ??= new AnalyticsClient(window));
}
export function trackUsage(name: string, data: EventData = {}) {
	try {
		browserAnalytics()?.track(name, data);
	} catch {
		/* Measurement must not interrupt an action. */
	}
}
