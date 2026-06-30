import type {
	CaseSummary,
	DemandDeltas,
	Network,
	SensitivityColumn,
	Solution,
	SolveIteration
} from '@tellegen/engine';

export type {
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

async function getJson<T>(
	fetchImpl: FetchLike,
	url: string,
	signal?: AbortSignal
): Promise<T> {
	const res = await fetchImpl(url, { signal });
	if (!res.ok) throw new Error(`${url} -> ${res.status}`);
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
			const res = await fetchImpl(url, { signal });
			if (!res.ok) throw new Error(`${url} -> ${res.status}`);
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
					handlers.onfail?.('solve stream interrupted');
				}
			};
			return finish;
		}
	};
}

const defaultClient = createApiClient();

export const getCases = () => defaultClient.getCases();
export const getNetwork = (caseId: string) => defaultClient.getNetwork(caseId);
export const getCaseNetworkJson = (caseId: string, signal?: AbortSignal) =>
	defaultClient.getCaseNetworkJson(caseId, signal);
export const getSolution = (caseId: string) => defaultClient.getSolution(caseId);
export const getSensitivity = (
	caseId: string,
	bus: number,
	deltas: DemandDeltas = {},
	signal?: AbortSignal
) => defaultClient.getSensitivity(caseId, bus, deltas, signal);
export const openSolveStream = (
	caseId: string,
	deltas: DemandDeltas,
	sensBus: number | null,
	handlers: SolveStreamHandlers
) => defaultClient.openSolveStream(caseId, deltas, sensBus, handlers);
