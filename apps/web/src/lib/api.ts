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
	onsolution?: (
		sol: Solution & { case: string; solve_ms: number; iterations: SolveIteration[] }
	) => void;
	onsensitivity?: (col: SensitivityColumn) => void;
	onfail?: (message: string) => void;
	ondone?: () => void;
}

/** Open the SSE solve stream: exact re-solve at base + deltas, then the
 * solution and (if `sensBus` is given) the sensitivity column at the new
 * operating point. Returns a closer. */
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
