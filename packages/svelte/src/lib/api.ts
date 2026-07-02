import type {
	CaseSummary,
	DemandDeltas,
	Network,
	SensitivityColumn,
	Solution,
	SolveIteration
} from '@tellegen/engine';

export type {
	BranchRatingDeltas,
	CaseSummary,
	DemandDeltas,
	Network,
	NetworkBranch,
	NetworkBus,
	SensitivityColumn,
	Solution,
	SolveIteration
} from '@tellegen/engine';

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export interface TellegenApiClientOptions {
	apiBase?: string;
	fetch?: FetchLike;
	EventSource?: typeof EventSource;
}

export interface TellegenApiClient {
	getCases(): Promise<CaseSummary[]>;
	getNetwork(caseId: string): Promise<Network>;
	getCaseNetworkJson(caseId: string, signal?: AbortSignal): Promise<string>;
	getSolution(caseId: string): Promise<Solution>;
	getSensitivity(
		caseId: string,
		bus: number,
		deltas?: DemandDeltas,
		signal?: AbortSignal
	): Promise<SensitivityColumn>;
	getBranchSensitivity(
		caseId: string,
		branch: number,
		deltas?: DemandDeltas,
		signal?: AbortSignal
	): Promise<SensitivityColumn>;
	openSolveStream(
		caseId: string,
		deltas: DemandDeltas,
		sensBus: number | null,
		handlers: SolveStreamHandlers
	): () => void;
}

function encodeDeltas(deltas: DemandDeltas): string {
	return Object.entries(deltas)
		.filter(([, mw]) => mw !== 0)
		.map(([bus, mw]) => `${bus}:${mw.toFixed(2)}`)
		.join(',');
}

function apiPath(apiBase: string, path: string): string {
	const base = apiBase.replace(/\/+$/, '');
	return `${base}${path}`;
}

function defaultFetch(): FetchLike {
	if (typeof fetch === 'undefined') throw new Error('fetch is not available');
	return fetch.bind(globalThis);
}

function defaultEventSource(): typeof EventSource {
	if (typeof EventSource === 'undefined') throw new Error('EventSource is not available');
	return EventSource;
}

/** A failed API request. `status` is the HTTP status, or 0 for a network
 * failure (the request never completed). `serverMessage` is the backend's
 * own `{"error": ...}` copy when the response carried one; edge-proxy
 * rejections (e.g. Caddy 429s) have none. */
export class ApiError extends Error {
	status: number;
	serverMessage: string | null;

	constructor(message: string, status: number, serverMessage: string | null) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.serverMessage = serverMessage;
	}
}

function userMessage(status: number, serverMessage: string | null): string {
	if (status === 0) return 'server unreachable, check your connection';
	if (status === 429) return 'rate limited; wait a few seconds and try again';
	if (status >= 500) return 'server error, try again';
	return serverMessage ?? `request failed (HTTP ${status})`;
}

async function checkedFetch(
	fetchImpl: FetchLike,
	url: string,
	signal?: AbortSignal
): Promise<Response> {
	let res: Response;
	try {
		res = await fetchImpl(url, { signal });
	} catch (e) {
		// Aborts must pass through unchanged: the controller filters them by
		// DOMException to distinguish cancellation from failure.
		if (e instanceof DOMException) throw e;
		throw new ApiError(userMessage(0, null), 0, null);
	}
	if (!res.ok) {
		let serverMessage: string | null = null;
		try {
			const body = (await res.json()) as { error?: unknown };
			if (typeof body.error === 'string') serverMessage = body.error;
		} catch {
			// non-JSON body (edge-proxy rejections); keep serverMessage null
		}
		throw new ApiError(userMessage(res.status, serverMessage), res.status, serverMessage);
	}
	return res;
}

async function getJson<T>(
	fetchImpl: FetchLike,
	url: string,
	signal?: AbortSignal
): Promise<T> {
	const res = await checkedFetch(fetchImpl, url, signal);
	return res.json() as Promise<T>;
}

export interface SolveStreamHandlers {
	onsolution?: (
		sol: Solution & { case: string; solve_ms: number; iterations: SolveIteration[] }
	) => void;
	onsensitivity?: (col: SensitivityColumn) => void;
	onfail?: (message: string) => void;
	ondone?: () => void;
}

export function createApiClient(options: TellegenApiClientOptions = {}): TellegenApiClient {
	const apiBase = options.apiBase ?? '/api';
	const fetchImpl = options.fetch ?? defaultFetch();

	return {
		getCases: () => getJson<CaseSummary[]>(fetchImpl, apiPath(apiBase, '/cases')),
		getNetwork: (caseId) => getJson<Network>(fetchImpl, apiPath(apiBase, `/cases/${caseId}/network`)),
		async getCaseNetworkJson(caseId, signal) {
			const url = apiPath(apiBase, `/cases/${caseId}/case`);
			const res = await checkedFetch(fetchImpl, url, signal);
			return res.text();
		},
		getSolution: (caseId) =>
			getJson<Solution>(fetchImpl, apiPath(apiBase, `/cases/${caseId}/solution`)),
		getSensitivity(caseId, bus, deltas = {}, signal) {
			const d = encodeDeltas(deltas);
			const query = d ? `?d=${encodeURIComponent(d)}` : '';
			return getJson<SensitivityColumn>(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/sensitivity/lmp/d/${bus}${query}`),
				signal
			);
		},
		getBranchSensitivity(caseId, branch, deltas = {}, signal) {
			const d = encodeDeltas(deltas);
			const query = d ? `?d=${encodeURIComponent(d)}` : '';
			return getJson<SensitivityColumn>(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/sensitivity/lmp/fmax/${branch}${query}`),
				signal
			);
		},
		openSolveStream(caseId, deltas, sensBus, handlers) {
			const EventSourceImpl = options.EventSource ?? defaultEventSource();
			const params = new URLSearchParams();
			const d = encodeDeltas(deltas);
			if (d) params.set('d', d);
			if (sensBus !== null) params.set('sens', String(sensBus));
			const qs = params.toString();
			const es = new EventSourceImpl(apiPath(apiBase, `/cases/${caseId}/solve${qs ? `?${qs}` : ''}`));
			let finished = false;
			const finish = () => {
				finished = true;
				es.close();
			};
			es.addEventListener('solution', (e) => handlers.onsolution?.(JSON.parse(e.data)));
			es.addEventListener('sensitivity', (e) => handlers.onsensitivity?.(JSON.parse(e.data)));
			es.addEventListener('fail', (e) => {
				finish();
				handlers.onfail?.(JSON.parse(e.data).error ?? 'solve failed');
			});
			es.addEventListener('done', () => {
				finish();
				handlers.ondone?.();
			});
			es.onerror = () => {
				// EventSource auto-reconnects; a closed-by-server stream after "done"
				// never reaches here because finish() already ran.
				if (!finished) {
					finish();
					// EventSource exposes no status code, so this covers both a lost
					// connection and a rate-limited stream open.
					handlers.onfail?.(
						'solve stream interrupted; server busy or rate limited, try again in a few seconds'
					);
				}
			};
			return finish;
		}
	};
}
