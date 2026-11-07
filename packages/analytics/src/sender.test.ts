import { describe, expect, it, vi } from 'vitest';
import { type FetchLike, type Payload, UMAMI_SEND, umamiSender } from './sender.js';

const payload: Payload = {
	website: 'site',
	hostname: 'example.test',
	url: '/',
	title: 'Site',
	referrer: '',
	name: 'case.select',
	data: { source: 'demo' }
};

async function settle() {
	for (let i = 0; i < 5; i++) await Promise.resolve();
}

describe('umamiSender', () => {
	it('posts one JSON event per call with no credentials and no referrer', async () => {
		const calls: Array<[string, Parameters<FetchLike>[1]]> = [];
		const fetchFn: FetchLike = (input, init) => {
			calls.push([input, init]);
			return Promise.resolve({ ok: true, json: async () => ({ cache: 'token-1' }) });
		};
		const sender = umamiSender(undefined, fetchFn);
		sender.send(payload);
		expect(calls).toHaveLength(1);
		const [endpoint, init] = calls[0]!;
		expect(endpoint).toBe(UMAMI_SEND);
		expect(init.method).toBe('POST');
		expect(init.credentials).toBe('omit');
		expect(init.referrerPolicy).toBe('no-referrer');
		expect(init.keepalive).toBe(true);
		expect(init.headers).toEqual({ 'content-type': 'application/json' });
		expect(JSON.parse(init.body)).toEqual({ type: 'event', payload });
	});

	it('resends the cache token the collector answered with', async () => {
		const headers: Array<Record<string, string>> = [];
		const fetchFn: FetchLike = (_input, init) => {
			headers.push(init.headers);
			return Promise.resolve({ ok: true, json: async () => ({ cache: 'token-1' }) });
		};
		const sender = umamiSender('https://collector.test/send', fetchFn);
		sender.send(payload);
		await settle();
		sender.send(payload);
		expect(headers[0]).toEqual({ 'content-type': 'application/json' });
		expect(headers[1]).toEqual({ 'content-type': 'application/json', 'x-umami-cache': 'token-1' });
	});

	it('ignores rejected, failing and malformed answers and never throws', async () => {
		const throwing = umamiSender(undefined, () => {
			throw new Error('blocked');
		});
		expect(() => throwing.send(payload)).not.toThrow();
		const rejecting = umamiSender(undefined, () => Promise.reject(new Error('offline')));
		expect(() => rejecting.send(payload)).not.toThrow();
		await settle();
		const headers: Array<Record<string, string>> = [];
		const bad: FetchLike = (_input, init) => {
			headers.push(init.headers);
			return Promise.resolve({ ok: false, json: async () => ({ cache: 'never' }) });
		};
		const sender = umamiSender(undefined, bad);
		sender.send(payload);
		await settle();
		sender.send(payload);
		expect(headers[1]).toEqual({ 'content-type': 'application/json' });
		const spy = vi.fn(() => undefined);
		umamiSender(undefined, spy as unknown as FetchLike).send(payload);
		expect(spy).toHaveBeenCalledTimes(1);
	});

	it('names the collector as one literal', () => {
		expect(UMAMI_SEND).toBe('https://gateway.umami.is/api/send');
	});
});
