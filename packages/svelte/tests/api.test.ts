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
			async () => new Response(JSON.stringify([{ id: 'c1', name: 'Case 1', n_bus: 1, n_branch: 0, n_gen: 1 }]), { status: 200 })
		).getCases();
		expect(cases).toEqual([{ id: 'c1', name: 'Case 1', n_bus: 1, n_branch: 0, n_gen: 1 }]);
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
	it('accepts existing server records with and without solver context, in order', async () => {
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
		// The stream closes at once; the handlers follow in event order.
		expect(stream.close).toHaveBeenCalledTimes(1);
		await vi.waitFor(() => expect(handlers.ondone).toHaveBeenCalledTimes(1));
		expect(handlers.onsolution).toHaveBeenCalledTimes(2);
		expect(handlers.onsensitivity).toHaveBeenCalledTimes(1);
		expect(handlers.onfail).not.toHaveBeenCalled();
		expect(handlers.onsensitivity.mock.invocationCallOrder[0]).toBeGreaterThan(
			handlers.onsolution.mock.invocationCallOrder[1]
		);
		expect(handlers.ondone.mock.invocationCallOrder[0]).toBeGreaterThan(
			handlers.onsensitivity.mock.invocationCallOrder[0]
		);
		expect(stream.close).toHaveBeenCalledTimes(1);
	});
	it('closes a malformed result stream and ignores already queued results', async () => {
		const { handlers, stream } = open();
		stream.emit('solution', { ...result, solve_ms: 'invalid' });
		stream.emit('solution', result);
		stream.emit('done', { ok: true });
		await vi.waitFor(() =>
			expect(handlers.onfail).toHaveBeenCalledWith(expect.stringContaining('/solve_ms'))
		);
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(handlers.onsolution).not.toHaveBeenCalled();
		expect(handlers.ondone).not.toHaveBeenCalled();
		expect(stream.close).toHaveBeenCalledTimes(1);
	});
	it('delivers nothing after the caller closes the stream', async () => {
		const handlers = {
			onsolution: vi.fn(),
			onsensitivity: vi.fn(),
			onfail: vi.fn(),
			ondone: vi.fn()
		};
		const close = createApiClient({
			fetch: async () => new Response('{}'),
			EventSource: FakeEventSource as unknown as typeof EventSource
		}).openSolveStream('case-1', {}, null, handlers);
		FakeEventSource.last.emit('solution', result);
		close();
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(handlers.onsolution).not.toHaveBeenCalled();
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
	])('rejects malformed nested %s data', async (event, data, path) => {
		const { handlers, stream } = open();
		stream.emit(event as string, data);
		await vi.waitFor(() =>
			expect(handlers.onfail).toHaveBeenCalledWith(expect.stringContaining(path as string))
		);
		expect(handlers.onsolution).not.toHaveBeenCalled();
		expect(handlers.onsensitivity).not.toHaveBeenCalled();
		expect(stream.close).toHaveBeenCalledTimes(1);
	});
	it('retains an actual server failure message', async () => {
		const { handlers, stream } = open();
		stream.emit('fail', { error: 'DC OPF solve infeasible: PrimalInfeasible' });
		await vi.waitFor(() =>
			expect(handlers.onfail).toHaveBeenCalledWith('DC OPF solve infeasible: PrimalInfeasible')
		);
		expect(handlers.onsolution).not.toHaveBeenCalled();
	});
});


describe('HTTP case and network validation', () => {
	const legacyNetwork = {
		id: 'c1', name: 'Case 1', base_mva: 100, synthetic_coords: false,
		buses: [{ id: 1, lon: 0, lat: 0, demand_mw: 1, gen_mw: 1 }],
		branches: [{ id: 1, from: 1, to: 1, rate_mw: 10, status: 1, path: [[0, 0], [1, 1]] }]
	};
	const response = (value: unknown) => clientWith(async () => new Response(JSON.stringify(value)));
	it('preserves legacy network fields and validates current substation records without omitted Rust fields', async () => {
		expect(await response(legacyNetwork).getNetwork('c1')).toEqual(legacyNetwork);
		const network = {
			...legacyNetwork,
			substations: [{ key: 'bus:1', name: null, source: 'single', kv: [13.8] }],
			equipment: { transformers: [], shunts: [], hvdc: [], detailed_dc: { nodes: 0, lines: 0, converters: 0 } }
		};
		expect(await response(network).getNetwork('c1')).toEqual(network);
	});
	it.each([
		{ ...legacyNetwork, buses: null },
		{ ...legacyNetwork, buses: [{ ...legacyNetwork.buses[0], lon: 'invalid' }] },
		{ ...legacyNetwork, branches: [{ ...legacyNetwork.branches[0], path: [[0, 'invalid']] }] },
		{ ...legacyNetwork, substations: [{ key: 'bus:1', name: null, source: 'single', kv: ['invalid'] }] },
		{ ...legacyNetwork, equipment: { transformers: [], shunts: [], hvdc: [], detailed_dc: { nodes: 'invalid', lines: 0, converters: 0 } } }
	])('rejects malformed network content', async (network) => {
		await expect(response(network).getNetwork('c1')).rejects.toThrow('Invalid engine http_network');
	});
	it.each([null, {}, [{ id: 'c1', name: 'Case 1', n_bus: 'invalid', n_branch: 0, n_gen: 1 }],
		[{ id: 'c1', name: 'Case 1', n_bus: 1, n_branch: 0, n_gen: 1, distribution: { source_url: 12 } }]])('rejects malformed case listings', async (cases) => {
		await expect(response(cases).getCases()).rejects.toThrow('Invalid engine http_cases');
	});
	it('requires a real compute capability boolean and preserves both values', async () => {
		for (const enabled of [false, true])
			expect(await response({ enabled }).getComputeStatus()).toEqual({ enabled });
		for (const value of [null, {}, { enabled: 'false' }, { enabled: 0 }])
			await expect(response(value).getComputeStatus()).rejects.toThrow('Invalid engine http_compute');
	});
});
