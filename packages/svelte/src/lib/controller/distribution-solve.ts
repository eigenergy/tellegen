import {
	DEFAULT_MC_PF_OPTIONS,
	ingestDistCaseBytes,
	summarizeMcPfResult,
	validateMcPfOptions,
	type EngineTransport,
	type IngestedDistCase,
	type McModuleInput,
	type McPfOptions,
	type McPfResult,
	type McPfSummary,
	type McStudySnapshot,
	type PowerIoDiagnostic
} from '@tellegen/engine';
import { sameSettings } from '../controller/case-settings.js';
import { errorTitle, type ErrorContext } from '../log.svelte.js';
import { buildDiagramView, buildGeographicView } from '../multiconductor.js';
import {
	MulticonductorCase,
	type MultiCaseSummary,
	type MultiCoordsKind
} from '../state.svelte.js';

import type { Controller } from '../controller.svelte.js';
import { caseLabel } from './messages.js';
type DistributionHost = Pick<Controller, 'app' | 'clearSelection' | 'localSeq'>;
type DistributionDependencies = {
	readonly disposed: boolean;
	readonly hostedLoadAbort: AbortController | null;
	readonly mcTransport: Controller['mcTransport'];
	failed: (
		title: string | undefined,
		stage: string | undefined,
		cause: unknown,
		known?: Omit<ErrorContext, 'retry'> & { quiet?: boolean; retry?: (() => void) | null }
	) => void;
	logImport: (
		fileName: string,
		summary: { diagnostics?: PowerIoDiagnostic[]; warnings?: string[] },
		moduleJson?: string,
		quiet?: boolean
	) => void;
};
export function releaseMcInput(input: McModuleInput | null | undefined) {
	if (input && typeof input !== 'string') input.free();
}
export class DistributionSolveActions {
	constructor(
		private host: DistributionHost,
		private deps: DistributionDependencies
	) {}
	addMultiCase = (
		fileName: string,
		payload: IngestedDistCase,
		hosted?: {
			id: string;
			name: string;
			distribution?: MulticonductorCase['distribution'];
		},
		labelOverride?: string,
		frame = true
	) => {
		if (!hosted) this.deps.hostedLoadAbort?.abort();

		const {
			graph,
			input,
			...summary
		}: Pick<IngestedDistCase, 'graph' | 'input'> & MultiCaseSummary = payload;
		const coordsKind = payload.coords_kind as MultiCoordsKind;
		const label = labelOverride ?? caseLabel(summary.name, fileName);
		const view =
			coordsKind === 'geographic'
				? buildGeographicView(graph, payload.geo_layer)
				: buildDiagramView(graph, payload.geo_layer);
		this.deps.logImport(fileName, summary);
		const c = new MulticonductorCase({
			id: hosted?.id ?? `dist-${++this.host.localSeq}`,
			label: hosted?.name ?? label,
			hosted: !!hosted,
			fileName,
			summary,
			input,
			graph,
			coordsKind,
			view
		});
		c.distribution = hosted?.distribution;
		c.mcOptions = {
			...DEFAULT_MC_PF_OPTIONS,
			...hosted?.distribution?.pf_options
		};
		this.host.app.studyView = null;
		this.host.clearSelection();
		this.host.app.addMulti(c);
		if (frame && c.placed) this.host.app.requestFrame(c.id);
		return c;
	};

	/** Compute terminal quantities only for the retained input and current case revision. */
	solveMultiCase = async (
		c: MulticonductorCase,
		options: McPfOptions = c.mcOptions,
		signal?: AbortSignal,
		energizedOnly = c.mcEnergizedOnly
	): Promise<McPfSummary> => {
		if (signal?.aborted || this.deps.disposed)
			throw new DOMException('Calculation cancelled', 'AbortError');
		if (c.solving) throw new Error('Already running');
		options = { ...DEFAULT_MC_PF_OPTIONS, ...c.mcOptions, ...options };
		const settingsError = validateMcPfOptions(options);
		if (settingsError) throw new Error(settingsError);
		const unavailable = c.mcPfReason;
		if (unavailable) {
			this.host.app.error = unavailable;
			throw new Error(unavailable);
		}
		// The retained input keeps its energized-network selection record, and
		// there is no engine operation to remove it, so a full-network run here
		// would still exclude the islands while claiming otherwise.
		if (c.mcInputSelected && !energizedOnly) throw new Error('Already energized only');
		// Rejected requests change nothing. Accepted settings persist even if
		// the solver then fails, so a retry runs with them.
		if (!sameSettings(c.mcOptions, options)) {
			const baseline = {
				...DEFAULT_MC_PF_OPTIONS,
				...c.distribution?.pf_options
			} as Record<string, unknown>;
			c.settings.options = {
				...c.settings.options,
				mc_ac_pf: Object.fromEntries(
					Object.entries(options).filter(([key, value]) => !sameSettings(value, baseline[key]))
				)
			};
			c.settings.revision++;
		}
		c.mcOptions = { ...options };
		c.mcEnergizedOnly = energizedOnly;
		const originalInput = c.mcInput!;
		let input: McModuleInput = originalInput;
		const retainedAtStart = c.mcSession;
		const revision = c.revisionGeneration;
		const seq = ++c.solveSeq;
		const abort = new AbortController();
		const cancel = () => abort.abort();
		signal?.addEventListener('abort', cancel, { once: true });
		c.solveAbort = abort;
		c.solving = true;
		this.host.app.error = null;
		const started = performance.now();
		let nextSession: Awaited<ReturnType<NonNullable<EngineTransport['createMcPfSession']>>> | null =
			null;
		// An input this call took from the live session; freed unless it
		// becomes the case's input.
		let editedInput: McModuleInput | null = null;
		try {
			// The edited operating point stays in the engine; it is never
			// written out as IR text to be read back.
			if (retainedAtStart) input = editedInput = await retainedAtStart.input();
			if (energizedOnly) {
				if (!this.deps.mcTransport.prepareMcEnergizedNetwork)
					throw new Error('Energized-network preparation is unavailable.');
				const prepared = await this.deps.mcTransport.prepareMcEnergizedNetwork(input);
				const selectedInput = 'input' in prepared ? prepared.input : prepared.module_json;
				if (editedInput !== selectedInput) releaseMcInput(editedInput);
				input = editedInput = selectedInput;
				if (abort.signal.aborted || seq !== c.solveSeq || revision !== c.revisionGeneration)
					throw new DOMException('Calculation cancelled', 'AbortError');
				c.mcExcludedIslands = prepared.excluded_islands;
				c.mcPreparedScope = prepared.network_scope;
			}
			let snapshot = null;
			let result: McPfSummary;
			let fullResult: McPfResult | null = null;
			let loadBranches = c.mcLoadBranches;
			if (this.deps.mcTransport.createMcPfSession) {
				// The session answers with its summary; terminal and equipment
				// detail is fetched only when a view asks for it.
				nextSession = await this.deps.mcTransport.createMcPfSession(input, options, abort.signal);
				result = nextSession.initialSummary;
				loadBranches = await nextSession.loadBranches();
			} else {
				const text = typeof input === 'string' ? input : await input.moduleJson();
				snapshot = this.deps.mcTransport.solveMcStudy
					? await this.deps.mcTransport.solveMcStudy(
							text,
							crypto.randomUUID(),
							c.label,
							options,
							abort.signal
						)
					: null;
				fullResult =
					snapshot?.result ??
					(await this.deps.mcTransport.solveMcModule(text, options, abort.signal));
				result = summarizeMcPfResult(fullResult);
			}
			// dispose() aborts too; a session that arrives after it is freed below.
			if (abort.signal.aborted || this.deps.disposed)
				throw new DOMException('Calculation cancelled', 'AbortError');
			if (
				seq !== c.solveSeq ||
				(retainedAtStart ? retainedAtStart !== c.mcSession : originalInput !== c.mcInput) ||
				revision !== c.revisionGeneration ||
				!this.host.app.multiCases.includes(c)
			)
				throw new Error('Case changed during the calculation');
			if (!result.converged) throw new Error('AC power flow did not converge.');
			// A load edit staged against the previous session has nothing to
			// apply to the fresh operating point; drop its pending flush.
			if (c.mcEditTimer !== null) {
				clearTimeout(c.mcEditTimer);
				c.mcEditTimer = null;
			}
			c.mcSession?.free();
			c.mcSession = nextSession;
			nextSession = null;
			c.mcLoadBranches = loadBranches;
			c.mcLoadEdits = [];
			if (c.mcInput !== input) releaseMcInput(c.mcInput);
			c.mcInput = input;
			editedInput = null;
			c.result = result;
			c.mcFullResult = fullResult;
			c.mcTiming = null;
			c.mcSnapshot = snapshot;
			c.mcSavedAt = null;
			c.solveMs = performance.now() - started;
			c.revisionGeneration++;
			c.mcCompletedSolveSeq = seq;
			if (energizedOnly) c.mcInputSelected = true;
			return result;
		} catch (error) {
			nextSession?.free();
			releaseMcInput(editedInput);
			if (
				!abort.signal.aborted &&
				!(error instanceof DOMException && error.name === 'AbortError')
			) {
				this.deps.failed(errorTitle(error, 'Power flow failed'), 'Solve', error, {
					retry: () => void this.solveMultiCase(c, options).catch(() => {})
				});
			}
			throw error;
		} finally {
			signal?.removeEventListener('abort', cancel);
			if (c.solveSeq === seq) {
				c.solving = false;
				c.solveAbort = null;
			}
		}
	};

	/** Open saved distribution results without moving the camera or duplicating the case. */
	restoreMultiSnapshot = async (snapshot: McStudySnapshot): Promise<MulticonductorCase> => {
		const existing = this.host.app.multiCases.find((c) => c.mcSnapshot?.id === snapshot.id);
		if (existing) {
			this.deps.hostedLoadAbort?.abort();
			this.host.app.studyView = null;
			this.host.app.activeCaseId = this.host.app.activeLocalId = null;
			this.host.app.activeMultiId = existing.id;
			return existing;
		}
		const payload = await (this.deps.mcTransport.ingestDistCaseBytes ?? ingestDistCaseBytes)(
			new TextEncoder().encode(snapshot.input_module),
			'pio',
			{
				retain: true
			}
		);
		const c = this.addMultiCase(
			`${snapshot.title}.pio.json`,
			payload,
			undefined,
			snapshot.title,
			false
		);
		c.mcSnapshot = snapshot;
		c.settings.baselines = { mc_ac_pf: { ...snapshot.options } };
		c.mcOptions = { ...snapshot.options };
		c.settings.revision++;
		c.mcEnergizedOnly = !!snapshot.result.network_scope;
		c.mcPreparedScope = snapshot.result.network_scope ?? null;
		c.mcExcludedIslands = snapshot.result.excluded_islands ?? [];
		c.result = summarizeMcPfResult(snapshot.result);
		c.mcFullResult = snapshot.result;
		c.mcSavedAt = new Date().toISOString();
		c.revisionGeneration++;
		return c;
	};

	/** Materialize the current retained operating point only when a portable
	 * snapshot is actually needed (save/export/geography). */
	snapshotMultiCase = async (c: MulticonductorCase): Promise<McStudySnapshot> => {
		if (c.mcSnapshot) return c.mcSnapshot;
		const session = c.mcSession;
		if (!session || !c.result) throw new Error('Run AC power flow before saving its result.');
		if (c.solving) throw new Error('Wait for the calculation before saving its result.');
		const revision = c.revisionGeneration;
		const snapshot = await session.snapshot(crypto.randomUUID(), c.label);
		if (
			session !== c.mcSession ||
			revision !== c.revisionGeneration ||
			!this.host.app.multiCases.includes(c)
		)
			throw new Error('The case changed while preparing its result.');
		c.mcSnapshot = snapshot;
		return snapshot;
	};
}
