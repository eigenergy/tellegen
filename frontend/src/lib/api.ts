export interface NetworkBus {
	id: number;
	lon: number;
	lat: number;
	demand_mw: number;
	gen_mw: number;
}

export interface NetworkBranch {
	id: number;
	from: number;
	to: number;
	rate_mw: number;
	status: number;
	path: [number, number][];
}

export interface Network {
	id: string;
	name: string;
	base_mva: number;
	synthetic_coords: boolean;
	buses: NetworkBus[];
	branches: NetworkBranch[];
}

export interface Solution {
	objective: number;
	lmp: { bus: number; usd_per_mwh: number }[];
	flows: { branch: number; mw: number; loading: number }[];
	dispatch: { gen: number; mw: number }[];
}

export interface SensitivityColumn {
	case: string;
	operand: string;
	parameter: string;
	bus: number;
	units: string;
	values: { bus: number; value: number }[];
}

export interface CaseSummary {
	id: string;
	name: string;
	n_bus: number;
	n_branch: number;
	n_gen: number;
}

export interface SolveIteration {
	iter: number;
	objective: number;
	inf_pr: number;
	inf_du: number;
}

/** Demand deltas in MW keyed by bus id, encoded as `bus:mw,bus:mw`. */
export type DemandDeltas = Record<number, number>;

function encodeDeltas(deltas: DemandDeltas): string {
	return Object.entries(deltas)
		.filter(([, mw]) => mw !== 0)
		.map(([bus, mw]) => `${bus}:${mw.toFixed(2)}`)
		.join(',');
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
	const res = await fetch(url, { signal });
	if (!res.ok) throw new Error(`${url} -> ${res.status}`);
	return res.json() as Promise<T>;
}

export const getCases = () => getJson<CaseSummary[]>('/api/cases');

export const getNetwork = (caseId: string) => getJson<Network>(`/api/cases/${caseId}/network`);

/** The raw powerio Network JSON, for solving the case in the browser. */
export async function getCaseNetworkJson(caseId: string, signal?: AbortSignal): Promise<string> {
	const url = `/api/cases/${caseId}/case`;
	const res = await fetch(url, { signal });
	if (!res.ok) throw new Error(`${url} -> ${res.status}`);
	return res.text();
}

export const getSolution = (caseId: string) => getJson<Solution>(`/api/cases/${caseId}/solution`);

export function getSensitivity(
	caseId: string,
	bus: number,
	deltas: DemandDeltas = {},
	signal?: AbortSignal
) {
	const d = encodeDeltas(deltas);
	const query = d ? `?d=${encodeURIComponent(d)}` : '';
	return getJson<SensitivityColumn>(
		`/api/cases/${caseId}/sensitivity/lmp/d/${bus}${query}`,
		signal
	);
}

export interface SolveStreamHandlers {
	oniteration?: (it: SolveIteration) => void;
	onsolution?: (sol: Solution & { case: string; solve_ms: number }) => void;
	onsensitivity?: (col: SensitivityColumn) => void;
	onfail?: (message: string) => void;
	ondone?: () => void;
}

/** Open the SSE solve stream: exact re-solve at base + deltas, interior point
 * iterations as they happen, then the solution and (if `sensBus` is given)
 * the sensitivity column at the new operating point. Returns a closer. */
export function openSolveStream(
	caseId: string,
	deltas: DemandDeltas,
	sensBus: number | null,
	handlers: SolveStreamHandlers
): () => void {
	const params = new URLSearchParams();
	const d = encodeDeltas(deltas);
	if (d) params.set('d', d);
	if (sensBus !== null) params.set('sens', String(sensBus));
	const qs = params.toString();
	const es = new EventSource(`/api/cases/${caseId}/solve${qs ? `?${qs}` : ''}`);
	let finished = false;
	const finish = () => {
		finished = true;
		es.close();
	};
	es.addEventListener('iteration', (e) => handlers.oniteration?.(JSON.parse(e.data)));
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
