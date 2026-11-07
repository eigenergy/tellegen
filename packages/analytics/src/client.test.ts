import { describe, expect, it } from 'vitest';
import { createAnalytics, localStorageOptOut, type AnalyticsConfig } from './client.js';
import type { Payload } from './sender.js';
import type { Vocabulary } from './vocabulary.js';

const WEBSITE = '00000000-0000-4000-8000-000000000000';

const vocabulary: Vocabulary = {
	pages: { '/': 'Site', '/privacy': 'Privacy' },
	events: {
		'case.select': ['source', 'demo_case'],
		'calculation.solve': ['source', 'demo_case', 'calculation', 'result', 'duration_ms'],
		'study.save': ['result']
	},
	enums: {
		source: new Set(['demo', 'local']),
		demo_case: new Set(['case200', 'case500']),
		calculation: new Set(['dcopf']),
		result: new Set(['completed', 'failed'])
	},
	numbers: new Set(['duration_ms'])
};

function host(url = 'https://example.test/', dnt = '0', gpc = false) {
	const values = new Map<string, string>();
	const win = {
		location: new URL(url),
		navigator: { doNotTrack: dnt, globalPrivacyControl: gpc },
		localStorage: {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => void values.set(key, value),
			removeItem: (key: string) => void values.delete(key)
		}
	};
	const sent: Payload[] = [];
	const config: AnalyticsConfig = {
		...vocabulary,
		website: WEBSITE,
		hosts: ['example.test', 'www.example.test'],
		window: win as unknown as Window,
		sender: { send: (payload) => void sent.push(payload) },
		optOut: localStorageOptOut('umami.disabled', win),
		refine: (_name, data) => {
			if (data.source !== 'demo') {
				const { demo_case: _dropped, ...rest } = data;
				return rest;
			}
			return data;
		}
	};
	return { client: createAnalytics(config), sent, values, win };
}

describe('createAnalytics', () => {
	it('sends only fixed page and event fields', () => {
		const { client, sent } = host('https://example.test/?file=SECRET#bus-123');
		client.page('/privacy?SECRET');
		client.track('case.select', {
			source: 'local',
			demo_case: 'case500',
			filename: 'SECRET.raw',
			coordinates: '-80,40'
		});
		client.track('calculation.solve', {
			source: 'demo',
			demo_case: 'case500',
			calculation: 'dcopf',
			result: 'completed',
			duration_ms: 123.4,
			objective: 98765,
			error: 'SECRET'
		});
		client.track('SECRET.event', { result: 'completed' });
		expect(sent).toEqual([
			{
				website: WEBSITE,
				hostname: 'example.test',
				url: '/privacy',
				title: 'Privacy',
				referrer: ''
			},
			{
				website: WEBSITE,
				hostname: 'example.test',
				url: '/',
				title: 'Site',
				referrer: '',
				name: 'case.select',
				data: { source: 'local' }
			},
			{
				website: WEBSITE,
				hostname: 'example.test',
				url: '/',
				title: 'Site',
				referrer: '',
				name: 'calculation.solve',
				data: {
					source: 'demo',
					demo_case: 'case500',
					calculation: 'dcopf',
					result: 'completed',
					duration_ms: 123
				}
			}
		]);
		expect(JSON.stringify(sent)).not.toMatch(/SECRET|98765|bus-123|file=/);
	});

	it('sends nothing from other hosts, over http, or under a browser privacy signal', () => {
		for (const [url, dnt, gpc] of [
			['http://127.0.0.1:4191/', '0', false],
			['https://preview.example.test/', '0', false],
			['http://example.test/', '0', false],
			['https://example.test/', '1', false],
			['https://example.test/', 'yes', false],
			['https://example.test/', '0', true]
		] as const) {
			const { client, sent } = host(url, dnt, gpc);
			client.page('/');
			client.track('case.select', { source: 'demo' });
			expect(client.allowed).toBe(false);
			expect(sent).toHaveLength(0);
		}
		expect(host('https://example.test/', '1').client.blockedByBrowser).toBe(true);
		expect(host('https://example.test/', '0', true).client.blockedByBrowser).toBe(true);
		expect(host().client.blockedByBrowser).toBe(false);
	});

	it('keeps the opt-out in the store and tells subscribers', () => {
		const { client, sent, values } = host();
		let told = 0;
		const stop = client.subscribe(() => told++);
		expect(client.optedOut).toBe(false);
		client.setOptOut(true);
		expect(values.get('umami.disabled')).toBe('1');
		expect(client.optedOut).toBe(true);
		expect(client.allowed).toBe(false);
		client.track('study.save', { result: 'completed' });
		expect(sent).toHaveLength(0);
		client.setOptOut(false);
		expect(values.has('umami.disabled')).toBe(false);
		client.track('study.save', { result: 'completed' });
		expect(sent).toHaveLength(1);
		expect(told).toBe(2);
		stop();
		client.setOptOut(true);
		expect(told).toBe(2);
		const previous = host();
		previous.values.set('umami.disabled', '1');
		expect(previous.client.optedOut).toBe(true);
	});

	it('treats storage that cannot be read as opted out', () => {
		const broken = {
			location: new URL('https://example.test/'),
			navigator: { doNotTrack: '0' },
			get localStorage(): Storage {
				throw new Error('denied');
			}
		};
		const sent: Payload[] = [];
		const client = createAnalytics({
			...vocabulary,
			website: WEBSITE,
			hosts: ['example.test'],
			window: broken as unknown as Window,
			sender: { send: (payload) => void sent.push(payload) },
			optOut: localStorageOptOut('umami.disabled', broken as unknown as { localStorage: Storage })
		});
		expect(client.optedOut).toBe(true);
		client.page('/');
		expect(sent).toHaveLength(0);
		expect(() => client.setOptOut(false)).not.toThrow();
	});

	it('never throws when the sender does, and sends nothing without a window', () => {
		const client = createAnalytics({
			...vocabulary,
			website: WEBSITE,
			hosts: ['example.test'],
			window: host().win as unknown as Window,
			sender: {
				send: () => {
					throw new Error('blocked collector');
				}
			}
		});
		expect(() => client.track('study.save', { result: 'completed' })).not.toThrow();
		expect(() => client.page('/')).not.toThrow();
		const headless = createAnalytics({
			...vocabulary,
			website: WEBSITE,
			hosts: ['example.test'],
			window: undefined
		});
		expect(headless.allowed).toBe(false);
		expect(headless.sanitize(undefined, {}, '/')).toBeNull();
	});
});
