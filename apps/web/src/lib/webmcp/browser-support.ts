/**
 * Works out why a browser does or does not offer WebMCP, so the Agent panel
 * can give one specific next step. Pure: it reads only the values passed in.
 */

/**
 * The first release of each browser with WebMCP. Chrome's origin trial starts
 * in 149; Edge runs its own, separate trial from 150. Other Chromium browsers
 * follow Chrome's engine version.
 */
export const WEBMCP_MIN_VERSION: Record<ChromiumBrand, number> = {
	Chrome: 149,
	Edge: 150,
	Chromium: 149
};
/** The origin whose prerendered pages carry tellegen's Chrome origin-trial token. */
export const WEBMCP_TRIAL_ORIGIN = 'https://tellegen.dev';
/** The flag that turns WebMCP on for every site, for builds or origins the trial skips. */
export const WEBMCP_FLAG = 'enable-webmcp-testing';

export type ChromiumBrand = 'Chrome' | 'Edge' | 'Chromium';

export type WebMcpBrowserState =
	/** `document.modelContext` is present; the page can register tools. */
	| { kind: 'ready'; mobile: boolean }
	/** Only the older `navigator.modelContext` exists: a Chromium build from before the trial. */
	| { kind: 'legacy'; brand: ChromiumBrand; version: number | null; minVersion: number }
	/** A phone or tablet without WebMCP. Agents that use page tools run on desktop. */
	| { kind: 'mobile' }
	/** A Chromium browser older than its first WebMCP release. */
	| { kind: 'update'; brand: ChromiumBrand; version: number; minVersion: number }
	/** A recent Chromium browser with WebMCP off for this page. */
	| { kind: 'flag'; brand: ChromiumBrand; version: number | null; flagUrl: string }
	/** Firefox, Safari, or another engine without WebMCP. */
	| { kind: 'other'; browser: string | null };

export interface BrowserSignals {
	/** `typeof document.modelContext?.registerTool === 'function'` */
	documentModelContext: boolean;
	/** `'modelContext' in navigator` */
	navigatorModelContext: boolean;
	userAgent: string;
	userAgentData?: {
		brands?: readonly { brand: string; version: string }[];
		mobile?: boolean;
		platform?: string;
	} | null;
}

/** Reads the signals from live browser globals. */
export function readBrowserSignals(
	doc: Document | undefined = globalThis.document,
	nav: Navigator | undefined = globalThis.navigator
): BrowserSignals {
	const modelContext = (doc as { modelContext?: { registerTool?: unknown } } | undefined)
		?.modelContext;
	const uaData = (nav as { userAgentData?: BrowserSignals['userAgentData'] } | undefined)
		?.userAgentData;
	return {
		documentModelContext: typeof modelContext?.registerTool === 'function',
		navigatorModelContext: !!nav && 'modelContext' in nav,
		userAgent: nav?.userAgent ?? '',
		userAgentData: uaData ?? null
	};
}

function chromium(
	signals: BrowserSignals
): { brand: ChromiumBrand; version: number | null } | null {
	const brands = signals.userAgentData?.brands;
	if (brands?.length) {
		const find = (name: string) => brands.find((entry) => entry.brand === name);
		const base = find('Chromium');
		if (!base) return null;
		const named = find('Microsoft Edge') ? 'Edge' : find('Google Chrome') ? 'Chrome' : 'Chromium';
		const version = Number.parseInt(base.version, 10);
		return { brand: named, version: Number.isFinite(version) ? version : null };
	}
	const ua = signals.userAgent;
	// Chrome and Edge on iOS use WebKit, so they are not Chromium here.
	if (/CriOS|EdgiOS|FxiOS/.test(ua)) return null;
	const edge = /Edg(?:A)?\/(\d+)/.exec(ua);
	if (edge) return { brand: 'Edge', version: Number(edge[1]) };
	const chrome = /(?:Chrome|Chromium)\/(\d+)/.exec(ua);
	if (!chrome) return null;
	const brand: ChromiumBrand = /OPR\/|Brave|Vivaldi|YaBrowser|SamsungBrowser/.test(ua)
		? 'Chromium'
		: 'Chrome';
	return { brand, version: Number(chrome[1]) };
}

function isMobile(signals: BrowserSignals): boolean {
	// Android tablets and "Request desktop site" report mobile: false, but no
	// desktop agent runs there either.
	if (signals.userAgentData?.platform === 'Android' || /Android/.test(signals.userAgent))
		return true;
	if (typeof signals.userAgentData?.mobile === 'boolean') return signals.userAgentData.mobile;
	// iPadOS Safari in desktop mode reports Macintosh and lands on 'other'.
	return /iPhone|iPad|iPod|Mobile/.test(signals.userAgent);
}

function otherBrowser(ua: string): string | null {
	if (/Firefox|FxiOS/.test(ua)) return 'Firefox';
	if (/Safari/.test(ua) && !/Chrome|Chromium/.test(ua)) return 'Safari';
	return null;
}

export function flagUrl(brand: ChromiumBrand): string {
	return `${brand === 'Edge' ? 'edge' : 'chrome'}://flags/#${WEBMCP_FLAG}`;
}

/** Classifies the browser from its signals; see {@link WebMcpBrowserState}. */
export function detectWebMcpBrowser(signals: BrowserSignals): WebMcpBrowserState {
	const mobile = isMobile(signals);
	if (signals.documentModelContext) return { kind: 'ready', mobile };
	if (mobile) return { kind: 'mobile' };
	const engine = chromium(signals);
	if (!engine) return { kind: 'other', browser: otherBrowser(signals.userAgent) };
	const minVersion = WEBMCP_MIN_VERSION[engine.brand];
	const old = engine.version !== null && engine.version < minVersion;
	// navigator.modelContext alone marks the early preview, unless the version
	// is current (an extension polyfill on a host without the trial, say).
	if (signals.navigatorModelContext && (old || engine.version === null))
		return { kind: 'legacy', ...engine, minVersion };
	if (old) return { kind: 'update', brand: engine.brand, version: engine.version!, minVersion };
	return { kind: 'flag', ...engine, flagUrl: flagUrl(engine.brand) };
}
