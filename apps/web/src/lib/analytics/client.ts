import {
	createAnalytics,
	localStorageOptOut,
	type AnalyticsClient,
	type CleanEventData,
	type EventData,
	type Vocabulary
} from '@tellegen/analytics';

export type { AnalyticsClient, EventData } from '@tellegen/analytics';

// The hosted demo's analytics vocabulary: the only pages, events, property
// keys and values that ever leave the browser. @tellegen/analytics rebuilds
// every payload from it and posts the result to Umami Cloud from this site's
// own code; no third-party script runs on the page.
export const UMAMI_WEBSITE = '41f22b57-b287-42b2-935c-037f82a80020';
export const OPT_OUT_KEY = 'umami.disabled';
export const HOSTS = ['tellegen.dev', 'www.tellegen.dev'] as const;
const PAGES: Record<string, string> = {
	'/': 'Tellegen',
	'/privacy/': 'Privacy',
	'/credits/': 'Credits',
	'/changelog/': 'Changelog'
};
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
		'export_case',
		'import_case',
		'inspect_webmcp_registration',
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
const PROPERTIES = {
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
} as const satisfies Record<string, readonly string[]>;
export type AnalyticsEvent = keyof typeof PROPERTIES;
export const VOCABULARY: Vocabulary = {
	pages: PAGES,
	events: PROPERTIES,
	enums: ENUMS,
	numbers: NUMBERS
};

// A demo case name says something only with the demo source.
function refine(_name: string, data: CleanEventData): CleanEventData {
	if (data.source === 'demo') return data;
	const { demo_case: _dropped, ...rest } = data;
	return rest;
}

let client: AnalyticsClient | null = null;
export function browserAnalytics(): AnalyticsClient | null {
	if (typeof window === 'undefined') return null;
	return (client ??= createAnalytics({
		...VOCABULARY,
		website: UMAMI_WEBSITE,
		hosts: HOSTS,
		optOut: localStorageOptOut(OPT_OUT_KEY),
		refine
	}));
}
export function trackUsage(name: AnalyticsEvent | string, data: EventData = {}) {
	try {
		browserAnalytics()?.track(name, data);
	} catch {
		/* Measurement must not interrupt an action. */
	}
}
