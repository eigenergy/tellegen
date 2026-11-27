import { parseApiResult, preloadValidators, validateApiResult } from '@tellegen/engine';
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
	NetworkSubstation,
	SensitivityColumn,
	Solution,
	SolveIteration
} from '@tellegen/engine';

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export interface TellegenApiClientOptions {
	/** Backend base path, default `/api`. `null` means no backend: the
	 * factory returns `null` instead of a client. */
	apiBase?: string | null;
	fetch?: FetchLike;
	EventSource?: typeof EventSource;
}

export interface TellegenApiClient {
	getCases(): Promise<CaseSummary[]>;
	getNetwork(caseId: string): Promise<Network>;
	getCaseModuleJson(caseId: string, signal?: AbortSignal): Promise<string>;
	getSolution(caseId: string): Promise<Solution>;
	getSavedCase?(caseId: string): Promise<{
		base_input: string;
		input: string;
		solution: string;
		view: import('@tellegen/engine').StudyView;
		display_solution: Solution;
	}>;
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
	/** Whether the server's compute endpoints (solve stream, sensitivities) are
	 * enabled on this deploy, so the frontend can pick honest fallback copy and
	 * skip doomed requests. */
	getComputeStatus(signal?: AbortSignal): Promise<{ enabled: boolean }>;
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

/** The notice for a rate-limited request, shared with the controller's own cooldown. */
export const RATE_LIMITED = 'Rate limited';

function userMessage(status: number, serverMessage: string | null): string {
	if (status === 0) return 'Server unreachable';
	if (status === 429) return RATE_LIMITED;
	if (status >= 500) return 'Server error';
	return serverMessage ?? `Request failed (HTTP ${status})`;
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
	signal?: AbortSignal,
	schema?: Parameters<typeof validateApiResult>[0]
): Promise<T> {
	const res = await checkedFetch(fetchImpl, url, signal);
	const value: unknown = await res.json();
	return schema ? await validateApiResult<T>(schema, value) : (value as T);
}

export interface SolveStreamHandlers {
	onsolution?: (
		sol: Solution & {
			case: string;
			solve_ms: number;
			iterations: SolveIteration[];
		}
	) => void;
	onsensitivity?: (col: SensitivityColumn) => void;
	onfail?: (message: string) => void;
	ondone?: () => void;
}

export function createApiClient(options: TellegenApiClientOptions & { apiBase: null }): null;
export function createApiClient(
	options?: TellegenApiClientOptions & { apiBase?: string }
): TellegenApiClient;
export function createApiClient(options?: TellegenApiClientOptions): TellegenApiClient | null;
export function createApiClient(options: TellegenApiClientOptions = {}): TellegenApiClient | null {
	if (options.apiBase === null) return null;
	const apiBase = options.apiBase ?? '/api';
	const fetchImpl = options.fetch ?? defaultFetch();

	return {
		getCases: () =>
			getJson<CaseSummary[]>(fetchImpl, apiPath(apiBase, '/cases'), undefined, 'http_cases'),
		getNetwork: (caseId) =>
			getJson<Network>(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/network`),
				undefined,
				'http_network'
			),
		async getCaseModuleJson(caseId, signal) {
			const url = apiPath(apiBase, `/cases/${caseId}/case`);
			const res = await checkedFetch(fetchImpl, url, signal);
			return res.text();
		},
		getSavedCase: (caseId) =>
			getJson(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/snapshot`),
				undefined,
				'http_saved_case'
			),
		getSolution: (caseId) =>
			getJson<Solution>(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/solution`),
				undefined,
				'http_solution'
			),
		getSensitivity(caseId, bus, deltas = {}, signal) {
			const d = encodeDeltas(deltas);
			const query = d ? `?d=${encodeURIComponent(d)}` : '';
			return getJson<SensitivityColumn>(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/sensitivity/lmp/d/${bus}${query}`),
				signal,
				'http_sensitivity'
			);
		},
		getBranchSensitivity(caseId, branch, deltas = {}, signal) {
			const d = encodeDeltas(deltas);
			const query = d ? `?d=${encodeURIComponent(d)}` : '';
			return getJson<SensitivityColumn>(
				fetchImpl,
				apiPath(apiBase, `/cases/${caseId}/sensitivity/lmp/fmax/${branch}${query}`),
				signal,
				'http_sensitivity'
			);
		},
		getComputeStatus(signal) {
			return getJson<{ enabled: boolean }>(
				fetchImpl,
				apiPath(apiBase, `/compute`),
				signal,
				'http_compute'
			);
		},
		openSolveStream(caseId, deltas, sensBus, handlers) {
			const EventSourceImpl = options.EventSource ?? defaultEventSource();
			const params = new URLSearchParams();
			const d = encodeDeltas(deltas);
			if (d) params.set('d', d);
			if (sensBus !== null) params.set('sens', String(sensBus));
			const qs = params.toString();
			const es = new EventSourceImpl(
				apiPath(apiBase, `/cases/${caseId}/solve${qs ? `?${qs}` : ''}`)
			);
			let finished = false;
			let closed = false;
			const close = () => {
				if (closed) return;
				closed = true;
				es.close();
			};
			const finish = () => {
				finished = true;
				close();
			};
			const invalid = (error: unknown) => {
				finish();
				handlers.onfail?.(error instanceof Error ? error.message : 'Invalid solve response');
			};
			// Validation may first import its validators, so each event is handled
			// after the ones before it: a late solution never follows "done". The
			// stream closes as soon as it ends, so EventSource does not reconnect.
			let queue = Promise.resolve();
			const enqueue = (handle: () => void | Promise<void>) => {
				queue = queue.then(() => (finished ? undefined : handle()));
			};
			void preloadValidators(['http_stream_solution', 'http_sensitivity']).catch(() => {});
			es.addEventListener('solution', (e) =>
				enqueue(async () => {
					let solution: Parameters<NonNullable<SolveStreamHandlers['onsolution']>>[0];
					try {
						solution = await parseApiResult('http_stream_solution', e.data);
					} catch (error) {
						invalid(error);
						return;
					}
					if (!finished) handlers.onsolution?.(solution);
				})
			);
			es.addEventListener('sensitivity', (e) =>
				enqueue(async () => {
					let column: SensitivityColumn;
					try {
						column = await parseApiResult('http_sensitivity', e.data);
					} catch (error) {
						invalid(error);
						return;
					}
					if (!finished) handlers.onsensitivity?.(column);
				})
			);
			es.addEventListener('fail', (e) => {
				close();
				enqueue(() => {
					finish();
					let value: unknown;
					try {
						value = JSON.parse(e.data);
					} catch {
						handlers.onfail?.('Invalid solve failure response');
						return;
					}
					handlers.onfail?.(
						value &&
							typeof value === 'object' &&
							'error' in value &&
							typeof value.error === 'string'
							? value.error
							: 'Solve failed'
					);
				});
			});
			es.addEventListener('done', () => {
				close();
				enqueue(() => {
					finish();
					handlers.ondone?.();
				});
			});
			es.onerror = () => {
				// EventSource auto-reconnects; a closed-by-server stream after "done"
				// never reaches here because the stream is already closed.
				close();
				enqueue(() => {
					finish();
					// EventSource exposes no status code, so this covers both a lost
					// connection and a rate-limited stream open.
					handlers.onfail?.(
						'solve stream interrupted; server busy or rate limited, try again in a few seconds'
					);
				});
			};
			return finish;
		}
	};
}
