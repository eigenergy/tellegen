import {
	createStudy,
	errorCode,
	isPermanentEngineFailure,
	resolveSettings,
	sensitivityColumn,
	solveAcOpfModule,
	solveResponseToSolution,
	type BrowserStudy,
	type SensTarget
} from '@tellegen/engine';
import type { SensitivityColumn } from '../api.js';
import {
	acOpfSolveDetail,
	defaultCalculationSettings,
	hasCustomCalculationSettings,
	withoutIgnoredSettings
} from '../controller/case-settings.js';
import { browserSolveFailure, errorText, formulationLabel } from '../format.js';
import { errorTitle, solveEntry, type ErrorContext } from '../log.svelte.js';
import { CaseState, type SolvableCase } from '../state.svelte.js';

import type { Controller } from '../controller.svelte.js';
type SolveHost = Pick<
	Controller,
	| 'getStudy'
	| 'disposeStudy'
	| 'acceptSensitivity'
	| 'finishSolve'
	| 'ensureStudyInputJson'
	| 'runSolve'
	| 'serverSolve'
	| 'acOpfAvailable'
	| 'api'
	| 'app'
	| 'caseDeltas'
	| 'caseName'
	| 'caseRatings'
	| 'caseStudies'
	| 'hasRatingEdits'
	| 'isActiveSolveCase'
	| 'isBackendCase'
	| 'localNetwork'
	| 'previewObjective'
	| 'serverComputeOff'
	| 'studyBuilds'
	| 'studyUnavailable'
>;
type SolveDependencies = {
	readonly disposed: boolean;
	readonly acOpfWasmUrl: string | null;
	readonly acOpfSolves: Map<SolvableCase, AbortController>;
	computeOffNotice: (c: SolvableCase) => string;
	ratingEditsFallbackError: (c: SolvableCase) => string;
	fail: (message: string, retry?: (() => void) | null, known?: ErrorContext) => void;
	failed: (
		title: string | undefined,
		stage: string | undefined,
		cause: unknown,
		known?: Omit<ErrorContext, 'retry'> & { quiet?: boolean; retry?: (() => void) | null }
	) => void;
};
/** True when a failed solve reports that the operating point has no solution.
 * The engine's error code decides; the message text is read only when the
 * error carries no code. */
function isInfeasible(e: unknown): boolean {
	const code = errorCode(e);
	if (code !== undefined) return code === 'infeasible';
	return /infeasible/i.test(errorText(e));
}
/** Owns browser Study creation, cancellation, sensitivity and the server fallback. */
export class BalancedSolveActions {
	constructor(
		private host: SolveHost,
		private deps: SolveDependencies
	) {}
	maybeStartLocalSolve = (id: string) => {
		const c = this.host.app.localCases.find((lc) => lc.id === id);
		if (!c?.studyInputJson || (!c.view && !c.diagram) || !c.summary) return;
		c.network = this.host.localNetwork(c) ?? c.network ?? null;
		if (c.autoSolve && c.network && !c.solution) this.host.runSolve(c, null);
	};
	getStudy = async (c: SolvableCase, studyInputJson: string): Promise<BrowserStudy | null> => {
		// A disposed controller builds nothing that would outlive it.
		if (this.deps.disposed) return null;
		const latched = this.host.studyUnavailable.get(c);
		if (latched) {
			c.solveFallbackReason ??= latched;
			return null;
		}
		const cached = this.host.caseStudies.get(c);
		if (cached && cached.studyInputJson === studyInputJson && cached.formulation === c.formulation)
			return cached.study;
		// Coalesce concurrent builds for the same (case, module, formulation): a
		// bus-select sensitivity fetch and a solve can both reach here before either
		// createStudy resolves. Without this each builds its own wasm Study and the
		// later set() orphans the earlier one (a leaked handle), and the cache can end
		// up holding an instance the live solve never committed on.
		const inflight = this.host.studyBuilds.get(c);
		if (
			inflight &&
			inflight.studyInputJson === studyInputJson &&
			inflight.formulation === c.formulation
		)
			return inflight.promise;
		// Different params, or no build pending: drop any stale Study before rebuilding.
		if (cached) {
			cached.study.free();
			this.host.caseStudies.delete(c);
		}
		const formulation = c.formulation;
		// Identity for this build attempt: the post-await guards compare against it
		// (rather than the promise) so a superseding build or a disposal wins the cache.
		const token = {};
		const build = (async (): Promise<BrowserStudy | null> => {
			try {
				const accepted =
					(c.settings.accepted?.calculation === formulation ? c.settings.accepted : null) ??
					(c.settings.baselines[formulation]
						? await resolveSettings({
								calculation: formulation,
								baseline: c.settings.baselines[formulation]
							})
						: null);
				// Settings are always explicit, so the engine never reads a module's
				// record made for another calculation and warns on every solve.
				const study = await createStudy(studyInputJson, formulation, {
					settings: { calculation: formulation, overrides: accepted?.effective ?? {} }
				});
				const baseSolution = await study.currentSolution();
				// The case may have switched formulation or been disposed (removed, or its
				// list reloaded) while building; don't cache a now-unwanted Study, and free
				// the orphan so its wasm memory is released.
				if (c.formulation !== formulation || this.host.studyBuilds.get(c)?.token !== token) {
					study.free();
					return null;
				}
				this.host.caseStudies.set(c, {
					study,
					studyInputJson,
					formulation,
					baseSolution,
					settings: accepted ?? defaultCalculationSettings(formulation)
				});
				return study;
			} catch (e) {
				const message = errorText(e);
				// Only a genuine browser-capability failure is permanent; latch it so the
				// case stays on the fallback path. Transient errors stay retryable.
				if (isPermanentEngineFailure(message, e)) {
					this.host.studyUnavailable.set(c, 'browser study wasm is not supported by this browser');
				}
				c.solveFallbackReason ??= `browser study unavailable: ${message}`;
				return null;
			} finally {
				if (this.host.studyBuilds.get(c)?.token === token) this.host.studyBuilds.delete(c);
			}
		})();
		this.host.studyBuilds.set(c, {
			studyInputJson,
			formulation,
			token,
			promise: build
		});
		return build;
	};
	browserSensitivity = async (
		c: SolvableCase,
		studyInputJson: string,
		target: SensTarget,
		signal?: AbortSignal
	): Promise<SensitivityColumn | null> => {
		if (c.formulation === 'acopf') {
			if (!this.host.acOpfAvailable || !this.deps.acOpfWasmUrl || c.perturbed)
				throw new Error('AC OPF sensitivity needs the available solver and an unedited case');
			const index =
				'bus' in target
					? c.solution?.prices.findIndex((entry) => entry.bus === target.bus)
					: c.solution?.flows.findIndex((entry) => entry.branch === target.branch);
			if (index === undefined || index < 0) return null;
			const response = await solveAcOpfModule(
				this.deps.acOpfWasmUrl,
				studyInputJson,
				signal,
				undefined,
				[
					{
						operand: { Price: 'Active' },
						parameter: 'bus' in target ? { Demand: 'Active' } : 'LineLimit',
						indices: [index]
					}
				]
			);
			return sensitivityColumn(c.id, response.sensitivities ?? []);
		}
		const study = await this.host.getStudy(c, studyInputJson);
		return study
			? study.sensitivity(c.id, this.host.caseDeltas(c), this.host.caseRatings(c), target)
			: null;
	};
	disposeStudy = (c: SolvableCase) => {
		this.deps.acOpfSolves.get(c)?.abort();
		this.deps.acOpfSolves.delete(c);
		const cached = this.host.caseStudies.get(c);
		if (cached) {
			cached.study.free();
			this.host.caseStudies.delete(c);
		}
		// Cancel any in-flight build: when it resolves it sees its entry gone and frees.
		this.host.studyBuilds.delete(c);
	};
	acceptSensitivity = (
		c: SolvableCase,
		col: SensitivityColumn | null,
		target: SensTarget | null,
		sensitivitySeq?: number
	) => {
		if (!col || target === null) return;
		// The column must name the target it was requested for, and the target must
		// still be the live selection.
		const matches =
			'bus' in target
				? col.bus === target.bus && this.host.app.selectedBus === target.bus
				: col.branch === target.branch && this.host.app.selectedBranch === target.branch;
		if (!matches) return;
		if (!this.host.isActiveSolveCase(c)) return;
		if (sensitivitySeq !== undefined && sensitivitySeq !== (c.sensitivitySeq ?? 0)) return;
		c.sensitivity = col;
	};
	finishSolve = (c: SolvableCase, seq: number, target: SensTarget | null) => {
		if (seq !== (c.solveSeq ?? 0)) return;
		c.solving = false;
		const acceptedSettings = this.host.caseStudies.get(c)?.settings;
		if (acceptedSettings?.calculation === c.formulation) c.settings.accepted = acceptedSettings;
		// Every solve of a case restates the same findings; they count once.
		const findings = c.solution?.diagnostics ?? [];
		if (findings.length)
			this.host.app.log.record(
				findings.map((d) => solveEntry(d, { subject: this.host.caseName(c) })),
				{ repeat: false }
			);
		const selectionLive =
			target !== null &&
			this.host.isActiveSolveCase(c) &&
			('bus' in target
				? this.host.app.selectedBus === target.bus
				: this.host.app.selectedBranch === target.branch);
		if (selectionLive) {
			this.host.app.previewActive = false;
			this.host.app.previewDeltaMw = null;
			this.host.app.previewRatingMw = null;
			// The committed solution supersedes the live engine preview.
			this.host.app.previewPrices = null;
			this.host.previewObjective = null;
		}
	};
	ensureStudyInputJson = async (c: SolvableCase): Promise<string | null> => {
		if (c.studyInputJson) return c.studyInputJson;
		if (!this.host.isBackendCase(c)) return null;
		const api = this.host.api;
		if (!api) {
			c.solveFallbackReason = 'no API configured';
			return null;
		}
		try {
			const moduleJson = await api.getCaseModuleJson(c.id);
			c.studyInputJson = moduleJson;
			return moduleJson;
		} catch (e) {
			c.solveFallbackReason = `case fetch failed: ${errorText(e)}`;
			return null;
		}
	};
	runSolve = (c: SolvableCase, target: SensTarget | null) => {
		if (this.deps.disposed) return;
		if (c.formulation === 'acpf' || c.formulation === 'acopf') target = null;
		this.deps.acOpfSolves.get(c)?.abort();
		this.deps.acOpfSolves.delete(c);
		// Cancel this case's own previous server stream, if any (backend only).
		if (this.host.isBackendCase(c)) {
			c.closeStream?.();
			c.closeStream = null;
		}
		c.solveSeq = (c.solveSeq ?? 0) + 1;
		const seq = c.solveSeq;
		this.host.app.error = null;
		c.solving = true;
		c.failedCalculation = null;
		c.solveBackend = null;
		c.solveDetail = null;
		c.solveFallbackReason = null;
		c.iterations = [];
		c.solveMs = null;
		this.host.ensureStudyInputJson(c).then(async (studyInputJson) => {
			if (this.deps.disposed || seq !== (c.solveSeq ?? 0)) return;
			if (!studyInputJson) {
				c.solveFallbackReason ??= 'PowerIO module unavailable';
				// The server fallback only implements DC OPF. Preserve the requested
				// calculation when its browser input cannot be loaded.
				if (c.formulation !== 'dcopf') {
					c.solving = false;
					this.host.app.error = `${this.host.caseName(c)}: ${formulationLabel(c.formulation)} requires its PowerIO module; ${c.solveFallbackReason}`;
					return;
				}
				if (this.host.hasRatingEdits(c)) {
					c.solving = false;
					this.host.app.error = this.deps.ratingEditsFallbackError(c);
					return;
				}
				if (this.host.isBackendCase(c)) return this.host.serverSolve(c, target, seq);
				c.solving = false;
				this.host.app.error = `${c.label}: re-add the case`;
				return;
			}
			const t0 = performance.now();
			if (c.formulation === 'acopf') {
				if (!this.host.acOpfAvailable || !this.deps.acOpfWasmUrl) {
					c.solving = false;
					this.host.app.error = `${this.host.caseName(c)}: AC OPF unavailable`;
					return;
				}
				if (c.perturbed) {
					c.solving = false;
					this.host.app.error = `${this.host.caseName(c)}: AC OPF needs the unedited case`;
					return;
				}
				const cancellation = new AbortController();
				this.deps.acOpfSolves.set(c, cancellation);
				c.solveBackend = 'pounce-wasi';
				try {
					const response = await solveAcOpfModule(
						this.deps.acOpfWasmUrl,
						studyInputJson,
						cancellation.signal
					);
					if (seq !== (c.solveSeq ?? 0) || cancellation.signal.aborted) return;
					c.solution = solveResponseToSolution(response);
					c.solveDetail = acOpfSolveDetail(response);
					c.solveMs = Math.round(performance.now() - t0);
					this.host.finishSolve(c, seq, null);
				} catch (error) {
					if (seq !== (c.solveSeq ?? 0) || cancellation.signal.aborted) return;
					c.solving = false;
					this.deps.failed(errorTitle(error, 'AC OPF failed'), 'Solve', error, {
						message: `${this.host.caseName(c)}: AC OPF failed in the isolated browser worker: ${errorText(error)}`
					});
				} finally {
					if (this.deps.acOpfSolves.get(c) === cancellation) this.deps.acOpfSolves.delete(c);
				}
				return;
			}
			c.solveBackend = 'clarabel-wasm';

			// Build-once Study path: commit the new operating point (no re-parse). The
			// selected nodal value sensitivity column is computed in the same
			// solve and comes back with it, so there is no second solve to reconcile it.
			const study = await this.host.getStudy(c, studyInputJson);
			if (seq !== (c.solveSeq ?? 0)) return;
			if (study) {
				try {
					const cached = this.host.caseStudies.get(c);
					if (!c.baseSolution && cached?.study === study) c.baseSolution = cached.baseSolution;
					const committed = await study.commit(
						c.id,
						this.host.caseDeltas(c),
						this.host.caseRatings(c),
						target
					);
					const { iterations, sensitivity } = committed;
					const solution =
						cached?.study === study && cached.ignoresRecordedSettings
							? withoutIgnoredSettings(committed.solution)
							: committed.solution;
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					c.iterations = iterations;
					if (!c.baseSolution && Object.keys(this.host.caseDeltas(c)).length === 0)
						c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					// The commit carried the sensitivity column; accept it for the selection
					// through the same seq-guarded setter every sensitivity source goes through.
					if (target !== null && sensitivity) this.host.acceptSensitivity(c, sensitivity, target);
					this.host.finishSolve(c, seq, target);
					return;
				} catch (e) {
					if (seq !== (c.solveSeq ?? 0)) return;
					const msg = errorText(e);
					// For AC OPF / SOCWR (browser-only) an infeasible operating point is an
					// expected outcome — demand pushed past what the network can serve — not a
					// broken Study. Keep the Study (it solves other points) and say so plainly,
					// instead of the misleading "study unavailable".
					if (c.formulation !== 'dcopf' && isInfeasible(e)) {
						c.solving = false;
						this.host.app.previewActive = false;
						this.host.app.previewDeltaMw = null;
						this.host.app.previewRatingMw = null;
						this.host.app.error = `${this.host.caseName(c)}: ${formulationLabel(c.formulation)} has no solution at this demand`;
						return;
					}
					// Any other commit failure is unexpected; drop the Study so the next solve
					// rebuilds, then fall through to the server fallback.
					this.host.disposeStudy(c);
					c.solveFallbackReason ??= `browser study commit failed: ${msg}`;
				}
			}

			// The server stream below only solves DC OPF, so it is valid only for the
			// DC formulation. For AC OPF / SOCWR the Study is the sole solver (nothing
			// is solved on the server), so a Study failure there is terminal: surface
			// it rather than silently returning a DC solution.
			if (c.formulation !== 'dcopf') {
				if (seq !== (c.solveSeq ?? 0)) return;
				c.solving = false;
				const why = c.solveFallbackReason ?? 'browser study unavailable';
				this.host.app.error = `${this.host.caseName(c)}: ${formulationLabel(c.formulation)} runs only in the browser Study, which is unavailable: ${why}`;
				return;
			}

			// Neither the server stream nor any other path can apply rating edits;
			// falling through would silently solve at base ratings, so stop here instead.
			if (this.host.hasRatingEdits(c)) {
				if (seq !== (c.solveSeq ?? 0)) return;
				c.solving = false;
				this.host.app.error = this.deps.ratingEditsFallbackError(c);
				return;
			}

			// Fallback: the server stream for backend cases. The Study is the only
			// browser solver, so a local case with a failed Study is terminal.
			if (seq !== (c.solveSeq ?? 0)) return;
			if (this.host.isBackendCase(c)) {
				this.host.serverSolve(c, target, seq);
				return;
			}
			c.solving = false;
			const failure = browserSolveFailure(c.label, c.solveFallbackReason);
			this.deps.fail(failure.message, null, {
				detail: failure.detail,
				stage: 'Solve'
			});
		});
	};
	serverSolve = (c: CaseState, target: SensTarget | null, seq = c.solveSeq) => {
		if (hasCustomCalculationSettings(c)) {
			if (seq !== c.solveSeq) return;
			c.solving = false;
			this.deps.fail("Server can't apply settings");
			return;
		}
		// Known-disabled server compute (or no backend at all): show the notice
		// instead of opening a stream that answers 403 without a readable body
		// (EventSource hides it).
		const api = this.host.api;
		if (this.host.serverComputeOff || !api) {
			if (seq !== c.solveSeq) return;
			c.solving = false;
			if (this.host.isActiveSolveCase(c)) {
				this.host.app.previewActive = false;
				this.host.app.previewDeltaMw = null;
				this.host.app.previewRatingMw = null;
			}
			this.deps.fail(this.deps.computeOffNotice(c), () => this.host.runSolve(c, target));
			return;
		}
		c.solveBackend = 'rust-server';
		c.solveFallbackReason ??= 'browser solve unavailable';
		// The server computes bus columns only; a branch target streams no sensitivity.
		const sensBus = target && 'bus' in target ? target.bus : null;
		c.closeStream = api.openSolveStream(c.id, c.deltas, sensBus, {
			onsolution: (sol) => {
				if (seq !== c.solveSeq) return;
				c.solution = sol;
				c.iterations = sol.iterations ?? [];
				c.solveMs = sol.solve_ms;
			},
			onsensitivity: (col) => {
				if (seq !== c.solveSeq) return;
				this.host.acceptSensitivity(c, col, target);
			},
			onfail: (msg) => {
				if (seq !== c.solveSeq) return;
				c.solving = false;
				// Only the active case owns the global preview fields (mirror finishSolve); a
				// background case's fallback failure must not collapse the active case's preview.
				if (this.host.isActiveSolveCase(c)) {
					this.host.app.previewActive = false;
					this.host.app.previewDeltaMw = null;
					this.host.app.previewRatingMw = null;
				}
				this.deps.fail(
					`${this.host.caseName(c)}: ${formulationLabel(c.formulation)} ${msg}`,
					() => this.host.runSolve(c, target),
					{
						title: `${formulationLabel(c.formulation)} failed`,
						stage: 'Solve'
					}
				);
			},
			ondone: () => {
				this.host.finishSolve(c, seq, target);
			}
		});
	};
}
