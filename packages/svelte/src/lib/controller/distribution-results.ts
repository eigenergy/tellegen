import {
	mcPfResultDetail,
	type McPfDetail,
	type McPfDetailQuery,
	type McPfResult,
	type McPfResultTables
} from '@tellegen/engine';
import type { MulticonductorCase } from '../state.svelte.js';

/** Reads live or saved distribution results without accepting stale session responses. */
export class DistributionResultsActions {
	/** One bus's terminals and a page of equipment ports for the displayed
	 * operating point, from the live session or the stored full result. Null
	 * when the operating point changed before the page arrived. */
	multiResultDetail = async (
		c: MulticonductorCase,
		query: McPfDetailQuery
	): Promise<McPfDetail | null> => {
		const summary = c.result;
		if (!summary) return null;
		const session = c.mcSession;
		if (session && !c.mcFullResult) {
			const detail = await session.detail(query);
			return session === c.mcSession && c.result?.solve_count === detail.solve_count
				? detail
				: null;
		}
		const full = c.mcFullResult ?? c.mcSnapshot?.result ?? null;
		return full ? mcPfResultDetail(full, query, summary.solve_count) : null;
	};

	/** Above this many buses, reading and indexing the whole operating point takes a noticeable
	 * fraction of a second (validation/evidence/diagram), so the overview is computed on request instead of
	 * after every solve. */
	static readonly AUTO_OVERVIEW_MAX_BUSES = 25_000;
	private overviewSeq = 0;

	/** Whether this case computes its overview only on request. */
	overviewOnRequest = (c: MulticonductorCase) =>
		(c.graph?.buses.length ?? 0) > DistributionResultsActions.AUTO_OVERVIEW_MAX_BUSES;

	/** Read the whole operating point into `c.overviewResult` for the overview and the diagram. The
	 * previous result stays visible until the read lands; a newer read supersedes it. */
	readOverview = (c: MulticonductorCase) => this.startOverviewRead(c, ++this.overviewSeq);

	private startOverviewRead(c: MulticonductorCase, seq: number) {
		c.overviewLoading = true;
		this.multiOverviewResult(c).then(
			(full) => {
				if (seq !== this.overviewSeq) return;
				if (full) c.overviewResult = full;
				c.overviewLoading = false;
			},
			() => {
				if (seq === this.overviewSeq) c.overviewLoading = false;
			}
		);
	}

	/** Follow the active case's solves: read its operating point once load edits settle, or clear it
	 * for a case that computes the overview on request. Call from an effect that tracks the case's
	 * `result` and `mcFullResult`; the returned function cancels a pending read. */
	followOverview = (c: MulticonductorCase | null): (() => void) | undefined => {
		const seq = ++this.overviewSeq;
		if (!c) return;
		if (c.result?.solve_count === undefined || this.overviewOnRequest(c)) {
			c.overviewResult = null;
			c.overviewLoading = false;
			return;
		}
		c.overviewLoading = true;
		const timer = setTimeout(() => this.startOverviewRead(c, seq), 300);
		return () => {
			clearTimeout(timer);
			// A read still in flight is superseded by the next one, so this case is no longer loading.
			c.overviewLoading = false;
		};
	};

	/** The complete operating point in the form whole-network views read: the stored result, or
	 * the live session's as columns, which hold no object per terminal or port. Null when there is
	 * no result or it changed before the read finished. */
	multiOverviewResult = async (
		c: MulticonductorCase
	): Promise<McPfResult | McPfResultTables | null> => {
		if (!c.result) return null;
		const stored = c.mcFullResult ?? c.mcSnapshot?.result ?? null;
		const session = c.mcSession;
		if (stored || !session) return stored;
		if (c.mcEditRunning) return null;
		const solve = c.result.solve_count;
		const tables = await session.resultTables();
		return session === c.mcSession && c.result?.solve_count === solve ? tables : null;
	};

	/** The complete operating point for whole-network views: the stored result, or one read from
	 * the live session. Null when there is no result or it changed before the read finished. */
	multiFullResult = async (c: MulticonductorCase): Promise<McPfResult | null> => {
		if (!c.result) return null;
		const stored = c.mcFullResult ?? c.mcSnapshot?.result ?? null;
		const session = c.mcSession;
		if (stored || !session) return stored;
		if (c.mcEditRunning) return null;
		const solve = c.result.solve_count;
		const full = await session.result();
		return session === c.mcSession && c.result?.solve_count === solve ? full : null;
	};

	/** Every terminal's identity, voltage, and injected current, as numeric
	 * arrays of interleaved `[re, im]` values. */
	multiTerminalTable = async (
		c: MulticonductorCase
	): Promise<{
		ids: Array<[string, string]>;
		voltages: Float64Array;
		currents: Float64Array;
	} | null> => {
		if (!c.result) return null;
		const session = c.mcSession;
		if (session && !c.mcFullResult) {
			// Arrays carry no solve count, and a running flush may discard its solve.
			if (c.mcEditRunning) return null;
			const solve = c.result.solve_count;
			const [ids, voltages, currents] = await Promise.all([
				session.terminalIds(),
				session.terminalVoltages(),
				session.terminalCurrents()
			]);
			return session === c.mcSession && c.result?.solve_count === solve
				? { ids, voltages, currents }
				: null;
		}
		const full = c.mcFullResult ?? c.mcSnapshot?.result ?? null;
		if (!full) return null;
		const voltages = new Float64Array(2 * full.terminals.length);
		const currents = new Float64Array(2 * full.terminals.length);
		full.terminals.forEach((terminal, k) => {
			voltages[2 * k] = terminal.voltage.re;
			voltages[2 * k + 1] = terminal.voltage.im;
			currents[2 * k] = terminal.current_into_network.re;
			currents[2 * k + 1] = terminal.current_into_network.im;
		});
		return {
			ids: full.terminals.map((terminal) => [terminal.bus, terminal.terminal]),
			voltages,
			currents
		};
	};
}
