/**
 * What the Agent panel's Connect tab says about agent tools in this browser,
 * and what to do in each case. The panel and the table in docs/src/webmcp.md
 * both come from CONNECT_STATUSES (scripts/webmcp-docs.mjs writes the table).
 */
import type { WebMcpBrowserState } from './browser-support.js';

export type ConnectStatusId = 'loading' | 'connected' | 'off' | 'update' | 'unsupported' | 'error';

export const CONNECT_STATUSES: readonly {
	id: ConnectStatusId;
	/** The status line. `{browser}` and `{version}` are filled in for `update`. */
	label: string;
	/** What to do, shown under the status line. Empty when there is nothing to do. */
	next: string;
	/** A longer version for the docs table. */
	docs: string;
}[] = [
	{
		id: 'loading',
		label: 'Loading…',
		next: '',
		docs: 'Wait. It changes within a few seconds.'
	},
	{
		id: 'connected',
		label: 'Connected',
		next: '',
		docs: 'The tools are registered. Press **Copy prompt** and go to step 2.'
	},
	{
		id: 'off',
		label: 'Off in this browser',
		next: 'Open this address, choose Enabled, relaunch',
		docs: 'The browser has WebMCP but this page may not use it. Copy the address shown (`chrome://flags/#enable-webmcp-testing`), paste it into the address bar, set it to **Enabled** and relaunch.'
	},
	{
		id: 'update',
		label: 'Needs {browser} {version}+',
		next: 'Update the browser',
		docs: 'Update the browser to the version named.'
	},
	{
		id: 'unsupported',
		label: 'Not available in this browser',
		next: 'Use Chrome, Edge or Brave on a computer',
		docs: 'Firefox, Safari and phones have no WebMCP. Use Chrome, Edge or Brave on a computer.'
	},
	{
		id: 'error',
		label: 'Tools failed to load',
		next: 'Reload the page',
		docs: 'Press **Reload**. **Details** shows the error; report it if it comes back.'
	}
];

export interface ConnectStatus {
	id: ConnectStatusId;
	label: string;
	next: string;
	/** The flag address to copy, for `off`. */
	flagUrl?: string;
}

/** The status for the page's current state. `browser` is null until it is checked. */
export function connectStatus(state: {
	supported: boolean;
	registrationError: string | null;
	browser: WebMcpBrowserState | null;
}): ConnectStatus {
	const { supported, registrationError, browser } = state;
	const row = (id: ConnectStatusId) => CONNECT_STATUSES.find((status) => status.id === id)!;
	const plain = (id: ConnectStatusId): ConnectStatus => {
		const { label, next } = row(id);
		return { id, label, next };
	};
	if (registrationError) return plain('error');
	if (supported) return plain('connected');
	if (!browser || browser.kind === 'ready') return plain('loading');
	if (browser.kind === 'update' || browser.kind === 'legacy') {
		const name = browser.brand === 'Chromium' ? 'this browser' : browser.brand;
		const { label, next } = row('update');
		return {
			id: 'update',
			label: label.replace('{browser}', name).replace('{version}', String(browser.minVersion)),
			next
		};
	}
	if (browser.kind === 'flag') return { ...plain('off'), flagUrl: browser.flagUrl };
	return plain('unsupported');
}

/**
 * Tools the page registers only in some states, with the reason an agent does
 * not see one. `registered` is the list of tool names the tab offers now.
 */
export const CONDITIONAL_TOOLS: readonly { name: string; reason: string }[] = [
	{ name: 'simulate_dynamics', reason: 'Dynamics is off in Settings' },
	{ name: 'propose_capacity_plan', reason: 'Needs a solved DC OPF on the live case' }
];

export function unavailableTools(registered: readonly string[]) {
	return CONDITIONAL_TOOLS.filter((tool) => !registered.includes(tool.name));
}
