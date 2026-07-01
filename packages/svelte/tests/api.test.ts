import { describe, expect, it } from 'vitest';
import { ApiError, createApiClient } from '../src/lib/api.js';

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

function clientWith(fetchImpl: FetchLike) {
	return createApiClient({ fetch: fetchImpl });
}

async function caught(fetchImpl: FetchLike): Promise<unknown> {
	try {
		await clientWith(fetchImpl).getCases();
	} catch (e) {
		return e;
	}
	throw new Error('expected getCases to throw');
}

describe('checkedFetch error mapping', () => {
	it('maps a 429 with a JSON body to the rate limit copy and keeps the server message', async () => {
		const e = await caught(async () =>
			new Response(JSON.stringify({ error: 'sensitivity rate limit exceeded' }), { status: 429 })
		);
		expect(e).toBeInstanceOf(ApiError);
		const err = e as ApiError;
		expect(err.status).toBe(429);
		expect(err.serverMessage).toBe('sensitivity rate limit exceeded');
		expect(err.message).toBe('rate limited; wait a few seconds and try again');
	});

	it('maps an empty-body 429 (edge proxy) to the same copy with no server message', async () => {
		const e = await caught(async () => new Response(null, { status: 429 }));
		expect(e).toBeInstanceOf(ApiError);
		const err = e as ApiError;
		expect(err.status).toBe(429);
		expect(err.serverMessage).toBeNull();
		expect(err.message).toBe('rate limited; wait a few seconds and try again');
	});

	it('maps 5xx to the server error copy', async () => {
		const e = (await caught(async () => new Response(null, { status: 503 }))) as ApiError;
		expect(e.status).toBe(503);
		expect(e.message).toBe('server error, try again');
	});

	it('surfaces the backend message for other 4xx', async () => {
		const e = (await caught(async () =>
			new Response(JSON.stringify({ error: 'demand delta for bus 3 would make demand negative' }), {
				status: 400
			})
		)) as ApiError;
		expect(e.status).toBe(400);
		expect(e.message).toBe('demand delta for bus 3 would make demand negative');
	});

	it('maps a rejected fetch to the unreachable copy with status 0', async () => {
		const e = (await caught(async () => {
			throw new TypeError('Failed to fetch');
		})) as ApiError;
		expect(e).toBeInstanceOf(ApiError);
		expect(e.status).toBe(0);
		expect(e.message).toBe('server unreachable, check your connection');
	});

	it('passes DOMException aborts through unwrapped', async () => {
		const abort = new DOMException('The operation was aborted.', 'AbortError');
		const e = await caught(async () => {
			throw abort;
		});
		expect(e).toBe(abort);
	});

	it('returns parsed JSON on 2xx', async () => {
		const cases = await clientWith(
			async () => new Response(JSON.stringify([{ id: 'c1' }]), { status: 200 })
		).getCases();
		expect(cases).toEqual([{ id: 'c1' }]);
	});
});
