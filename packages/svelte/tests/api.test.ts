import { describe, expect, it, vi } from 'vitest';
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
		const e = await caught(
			async () =>
				new Response(JSON.stringify({ error: 'sensitivity rate limit exceeded' }), { status: 429 })
		);
		expect(e).toBeInstanceOf(ApiError);
		const err = e as ApiError;
		expect(err.status).toBe(429);
		expect(err.serverMessage).toBe('sensitivity rate limit exceeded');
		expect(err.message).toBe('Rate limited');
	});

	it('maps an empty-body 429 (edge proxy) to the same copy with no server message', async () => {
		const e = await caught(async () => new Response(null, { status: 429 }));
		expect(e).toBeInstanceOf(ApiError);
		const err = e as ApiError;
		expect(err.status).toBe(429);
		expect(err.serverMessage).toBeNull();
		expect(err.message).toBe('Rate limited');
	});

	it('maps 5xx to the server error copy', async () => {
		const e = (await caught(async () => new Response(null, { status: 503 }))) as ApiError;
		expect(e.status).toBe(503);
		expect(e.message).toBe('Server error');
	});

	it('surfaces the backend message for other 4xx', async () => {
		const e = (await caught(
			async () =>
				new Response(
					JSON.stringify({
						error: 'demand delta for bus 3 would make demand negative'
					}),
					{
						status: 400
					}
				)
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
		expect(e.message).toBe('Server unreachable');
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

describe('solution wire', () => {
	it('reads convention-neutral prices from the server response', async () => {
		const client = clientWith(
			async () =>
				new Response(
					JSON.stringify({
						objective: 12,
						prices: [{ bus: 4, value: 3.5 }],
						va: [],
						w: [],
						flows: [],
						dispatch: []
					})
				)
		);

		const solution = await client.getSolution('case-1');
		expect(solution.prices).toEqual([{ bus: 4, value: 3.5 }]);
	});

	it('rejects malformed nested numbers before displaying a result', async () => {
		const client = clientWith(
			async () =>
				new Response(
					JSON.stringify({
						objective: 12,
						prices: [{ bus: 4, value: 'wrong' }],
						va: [],
						w: [],
						flows: [],
						dispatch: []
					})
				)
		);
		await expect(client.getSolution('case-1')).rejects.toThrow('/prices/0/value');
	});
});

class FakeEventSource {
	static last: FakeEventSource;
	listeners = new Map<string, (event: { data: string }) => void>();
	close = vi.fn();
	onerror: (() => void) | null = null;
	constructor(_url: string) {
		FakeEventSource.last = this;
	}
	addEventListener(name: string, listener: (event: { data: string }) => void) {
		this.listeners.set(name, listener);
	}
	emit(name: string, value: unknown) {
		this.listeners.get(name)?.({ data: JSON.stringify(value) });
	}
}

describe('streamed result validation', () => {
	const result = {
		case: 'case-1',
		solve_ms: 5,
		iterations: [],
		objective: 12,
		prices: [{ bus: 4, value: 3.5 }],
		va: [],
		w: [],
		flows: [],
		dispatch: []
	};
	function open() {
		const handlers = {
			onsolution: vi.fn(),
			onsensitivity: vi.fn(),
			onfail: vi.fn(),
			ondone: vi.fn()
		};
		createApiClient({
			fetch: async () => new Response('{}'),
			EventSource: FakeEventSource as unknown as typeof EventSource
		}).openSolveStream('case-1', {}, null, handlers);
		return { handlers, stream: FakeEventSource.last };
	}
	it('accepts existing server records with and without solver context', () => {
		const { handlers, stream } = open();
		stream.emit('solution', result);
		stream.emit('solution', {
			...result,
			formulation: 'dcopf',
			status: 'optimal',
			objective_kind: 'cost'
		});
		stream.emit('sensitivity', {
			case: 'case-1',
			operand: 'price',
			parameter: 'demand',
			bus: 4,
			units: 'USD/MWh/MW',
			values: [{ bus: 4, value: 0.1 }]
		});
		stream.emit('done', { ok: true });
		expect(handlers.onsolution).toHaveBeenCalledTimes(2);
		expect(handlers.onsensitivity).toHaveBeenCalledTimes(1);
		expect(handlers.onfail).not.toHaveBeenCalled();
		expect(handlers.ondone).toHaveBeenCalledTimes(1);
		expect(stream.close).toHaveBeenCalledTimes(1);
	});
	it('closes a malformed result stream and ignores already queued results', () => {
		const { handlers, stream } = open();
		stream.emit('solution', { ...result, solve_ms: 'invalid' });
		stream.emit('solution', result);
		stream.emit('done', { ok: true });
		expect(handlers.onfail).toHaveBeenCalledWith(expect.stringContaining('/solve_ms'));
		expect(handlers.onsolution).not.toHaveBeenCalled();
		expect(handlers.ondone).not.toHaveBeenCalled();
		expect(stream.close).toHaveBeenCalledTimes(1);
	});
	it.each([
		[
			'solution',
			{ ...result, iterations: [{ iter: 1, objective: 12, inf_pr: 'bad', inf_du: 0 }] },
			'/iterations/0/inf_pr'
		],
		[
			'sensitivity',
			{
				case: 'case-1',
				operand: 'price',
				parameter: 'demand',
				units: 'USD/MWh/MW',
				values: [{ bus: 4, value: 'bad' }]
			},
			'/values/0/value'
		]
	])('rejects malformed nested %s data', (event, data, path) => {
		const { handlers, stream } = open();
		stream.emit(event as string, data);
		expect(handlers.onfail).toHaveBeenCalledWith(expect.stringContaining(path as string));
		expect(handlers.onsolution).not.toHaveBeenCalled();
		expect(handlers.onsensitivity).not.toHaveBeenCalled();
		expect(stream.close).toHaveBeenCalledTimes(1);
	});
	it('retains an actual server failure message', () => {
		const { handlers, stream } = open();
		stream.emit('fail', { error: 'DC OPF solve infeasible: PrimalInfeasible' });
		expect(handlers.onfail).toHaveBeenCalledWith('DC OPF solve infeasible: PrimalInfeasible');
		expect(handlers.onsolution).not.toHaveBeenCalled();
	});
});
