import { test } from 'node:test';
import assert from 'node:assert/strict';
import { detectWebMcpBrowser, readBrowserSignals } from '../src/lib/webmcp/browser-support.ts';

const chromeUa = (v) =>
	`Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${v}.0.0.0 Safari/537.36`;
const brands = (name, v) => [
	{ brand: 'Not.A/Brand', version: '99' },
	{ brand: 'Chromium', version: String(v) },
	{ brand: name, version: String(v) }
];
const signals = (overrides) => ({
	documentModelContext: false,
	navigatorModelContext: false,
	userAgent: chromeUa(157),
	userAgentData: { brands: brands('Google Chrome', 157), mobile: false },
	...overrides
});

test('document.modelContext means ready, whatever the browser says', () => {
	assert.deepEqual(detectWebMcpBrowser(signals({ documentModelContext: true })), {
		kind: 'ready',
		mobile: false
	});
	assert.deepEqual(
		detectWebMcpBrowser(
			signals({ documentModelContext: true, userAgentData: { brands: [], mobile: true } })
		),
		{ kind: 'ready', mobile: true }
	);
});

test('recent Chrome or Edge without modelContext gets the flag for its own scheme', () => {
	assert.deepEqual(detectWebMcpBrowser(signals({})), {
		kind: 'flag',
		brand: 'Chrome',
		version: 157,
		flagUrl: 'chrome://flags/#enable-webmcp-testing'
	});
	const edge = detectWebMcpBrowser(
		signals({ userAgentData: { brands: brands('Microsoft Edge', 150), mobile: false } })
	);
	assert.equal(edge.kind, 'flag');
	assert.equal(edge.brand, 'Edge');
	assert.equal(edge.flagUrl, 'edge://flags/#enable-webmcp-testing');
});

test('Chromium before the trial is told to update', () => {
	assert.deepEqual(
		detectWebMcpBrowser(
			signals({ userAgentData: { brands: brands('Google Chrome', 140), mobile: false } })
		),
		{ kind: 'update', brand: 'Chrome', version: 140, minVersion: 149 }
	);
});

test("Edge's own trial starts in 150, a release after Chrome's", () => {
	assert.deepEqual(
		detectWebMcpBrowser(
			signals({ userAgentData: { brands: brands('Microsoft Edge', 149), mobile: false } })
		),
		{ kind: 'update', brand: 'Edge', version: 149, minVersion: 150 }
	);
	assert.equal(
		detectWebMcpBrowser(
			signals({ userAgentData: { brands: brands('Microsoft Edge', 150), mobile: false } })
		).kind,
		'flag'
	);
});

test('navigator.modelContext on a current build is a polyfill, not an old browser', () => {
	assert.equal(detectWebMcpBrowser(signals({ navigatorModelContext: true })).kind, 'flag');
	assert.equal(
		detectWebMcpBrowser(
			signals({
				navigatorModelContext: true,
				userAgentData: { brands: brands('Google Chrome', 'unknown'), mobile: false }
			})
		).kind,
		'legacy'
	);
});

test('Android tablets and desktop-mode Android are mobile; desktop-mode iPad Safari is Safari', () => {
	assert.equal(
		detectWebMcpBrowser(
			signals({
				userAgent:
					'Mozilla/5.0 (Linux; Android 15; SM-X910) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36',
				userAgentData: { brands: brands('Google Chrome', 157), mobile: false, platform: 'Android' }
			})
		).kind,
		'mobile'
	);
	assert.equal(
		detectWebMcpBrowser({
			documentModelContext: false,
			navigatorModelContext: false,
			userAgent:
				'Mozilla/5.0 (Linux; Android 15; Pixel Tablet) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36'
		}).kind,
		'mobile'
	);
	assert.deepEqual(
		detectWebMcpBrowser({
			documentModelContext: false,
			navigatorModelContext: false,
			userAgent:
				'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.0 Safari/605.1.15'
		}),
		{ kind: 'other', browser: 'Safari' }
	);
});

test('only navigator.modelContext is the early preview', () => {
	assert.equal(
		detectWebMcpBrowser(
			signals({
				navigatorModelContext: true,
				userAgentData: { brands: brands('Google Chrome', 147), mobile: false }
			})
		).kind,
		'legacy'
	);
});

test('Firefox and Safari are named; no Chromium brand means another engine', () => {
	assert.deepEqual(
		detectWebMcpBrowser({
			documentModelContext: false,
			navigatorModelContext: false,
			userAgent:
				'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:140.0) Gecko/20100101 Firefox/140.0'
		}),
		{ kind: 'other', browser: 'Firefox' }
	);
	assert.deepEqual(
		detectWebMcpBrowser({
			documentModelContext: false,
			navigatorModelContext: false,
			userAgent:
				'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.0 Safari/605.1.15'
		}),
		{ kind: 'other', browser: 'Safari' }
	);
});

test('phones and tablets without modelContext get the desktop hint', () => {
	assert.equal(
		detectWebMcpBrowser(
			signals({ userAgentData: { brands: brands('Google Chrome', 157), mobile: true } })
		).kind,
		'mobile'
	);
	assert.equal(
		detectWebMcpBrowser({
			documentModelContext: false,
			navigatorModelContext: false,
			userAgent:
				'Mozilla/5.0 (iPhone; CPU iPhone OS 19_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/157.0 Mobile/15E148 Safari/604.1'
		}).kind,
		'mobile'
	);
});

test('without client hints the user agent string decides', () => {
	const base = { documentModelContext: false, navigatorModelContext: false, userAgentData: null };
	assert.deepEqual(detectWebMcpBrowser({ ...base, userAgent: chromeUa(120) }), {
		kind: 'update',
		brand: 'Chrome',
		version: 120,
		minVersion: 149
	});
	const edge = detectWebMcpBrowser({ ...base, userAgent: `${chromeUa(150)} Edg/150.0.0.0` });
	assert.equal(edge.kind, 'flag');
	assert.equal(edge.brand, 'Edge');
	assert.equal(
		detectWebMcpBrowser({ ...base, userAgent: `${chromeUa(157)} OPR/120.0.0.0` }).brand,
		'Chromium'
	);
});

test('a Chromium fork reports its engine version under its own brand', () => {
	const state = detectWebMcpBrowser(
		signals({ userAgentData: { brands: brands('Brave', 157), mobile: false } })
	);
	assert.equal(state.kind, 'flag');
	assert.equal(state.brand, 'Chromium');
});

test('readBrowserSignals reads registerTool, not just the property', () => {
	const nav = { userAgent: chromeUa(157), modelContext: {} };
	assert.equal(readBrowserSignals({ modelContext: {} }, nav).documentModelContext, false);
	const ok = readBrowserSignals({ modelContext: { registerTool() {} } }, nav);
	assert.equal(ok.documentModelContext, true);
	assert.equal(ok.navigatorModelContext, true);
});
