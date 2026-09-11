import { describe, expect, it, vi } from 'vitest';
import {
	AnalyticsClient,
	UMAMI_SCRIPT,
	UMAMI_WEBSITE,
	OPT_OUT_KEY,
	eventData
} from '../../../apps/web/src/lib/analytics/client.js';

function host(url = 'https://tellegen.dev/', dnt = '0') {
	const values = new Map<string, string>();
	const scripts: Array<Record<string, any>> = [];
	const collector: unknown[] = [];
	const browser = {
		location: new URL(url),
		navigator: { doNotTrack: dnt },
		localStorage: {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value),
			removeItem: (key: string) => values.delete(key)
		},
		document: {
			createElement: () => ({ dataset: {} }),
			head: { appendChild: (script: Record<string, any>) => scripts.push(script) }
		},
		umami: {
			track: vi.fn((payload: Record<string, unknown>) => {
				const outgoing = client.sanitize('event', payload);
				if (outgoing) collector.push({ type: 'event', payload: outgoing });
			})
		}
	};
	const client = new AnalyticsClient(browser as unknown as Window);
	return { client, browser, scripts, collector, values };
}
describe('analytics payload privacy', () => {
	it('sends only fixed page and action fields to a mocked collector', () => {
		const { client, collector } = host('https://tellegen.dev/?file=SECRET#bus-123');
		client.track('case.select', {
			source: 'local',
			demo_case: 'case500',
			filename: 'SECRET.raw',
			study: 'SECRET',
			coordinates: '-80,40'
		});
		client.track('calculation.solve', {
			source: 'demo',
			demo_case: 'case7000',
			calculation: 'dcopf',
			result: 'completed',
			duration_ms: 123.4,
			solve_ms: 95,
			iterations: 8,
			objective: 98765,
			error: 'SECRET'
		});
		expect(collector).toEqual([
			{
				type: 'event',
				payload: {
					website: UMAMI_WEBSITE,
					hostname: 'tellegen.dev',
					url: '/',
					title: 'Tellegen',
					referrer: '',
					name: 'case.select',
					data: { source: 'local' }
				}
			},
			{
				type: 'event',
				payload: {
					website: UMAMI_WEBSITE,
					hostname: 'tellegen.dev',
					url: '/',
					title: 'Tellegen',
					referrer: '',
					name: 'calculation.solve',
					data: {
						source: 'demo',
						demo_case: 'case7000',
						calculation: 'dcopf',
						result: 'completed',
						duration_ms: 123,
						solve_ms: 95,
						iterations: 8
					}
				}
			}
		]);
		expect(JSON.stringify(collector)).not.toMatch(/SECRET|98765|bus-123/);
	});
	it('reconstructs vendor performance data and blocks identification or arbitrary events', () => {
		const { client } = host();
		const payload = {
			url: '/privacy/?SECRET#SECRET',
			title: 'SECRET',
			referrer: 'https://secret.example',
			id: 'SECRET',
			lcp: 300.2,
			cls: 0.123456,
			inp: 21,
			selector: 'SECRET',
			data: { filename: 'SECRET' }
		};
		expect(client.sanitize('performance', payload)).toEqual({
			website: UMAMI_WEBSITE,
			hostname: 'tellegen.dev',
			url: '/privacy/',
			title: 'Privacy',
			referrer: '',
			lcp: 300,
			cls: 0.1235,
			inp: 21
		});
		expect(client.sanitize('identify', payload)).toBe(false);
		expect(client.sanitize('replay', payload)).toBe(false);
		expect(client.sanitize('event', { name: 'SECRET', ...payload })).toBe(false);
		expect(
			eventData('agent.call', { tool: 'UNKNOWN_SECRET', duration_ms: NaN, result: 'failed' })
		).toEqual({ result: 'failed' });
	});
	it('does not load scripts or send events for previews, browser privacy settings, or opt-out', () => {
		for (const [url, dnt] of [
			['http://127.0.0.1:4191', '0'],
			['https://preview.tellegen.dev', '0'],
			['https://tellegen.dev', '1']
		]) {
			const { client, scripts, collector } = host(url, dnt);
			client.start();
			client.track('case.select', { source: 'demo' });
			expect(scripts).toHaveLength(0);
			expect(collector).toHaveLength(0);
		}
		const { client, values, scripts, collector } = host();
		values.set(OPT_OUT_KEY, '1');
		client.start();
		expect(scripts).toHaveLength(0);
		client.setOptOut(false);
		expect(scripts).toHaveLength(1);
		client.setOptOut(true);
		client.track('case.select', { source: 'demo' });
		expect(collector).toHaveLength(0);
	});
	it('uses official endpoints and sanitized performance tracking without manual identifiers', () => {
		const { client, scripts, browser } = host();
		client.start();
		expect(scripts[0].src).toBe(UMAMI_SCRIPT);
		expect(scripts[0].referrerPolicy).toBe('no-referrer');
		expect(scripts[0].dataset).toMatchObject({
			websiteId: UMAMI_WEBSITE,
			autoPageview: 'false',
			performance: 'true',
			doNotTrack: 'true',
			excludeSearch: 'true',
			excludeHash: 'true',
			beforeSend: 'tellegenBeforeAnalyticsSend'
		});
		expect(scripts[0].dataset.hostUrl).toBeUndefined();
		browser.umami.track.mockImplementation(() => {
			throw new Error('blocked collector');
		});
		expect(() => client.track('study.save', { result: 'completed' })).not.toThrow();
	});
});
