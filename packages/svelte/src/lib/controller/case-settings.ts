import {
	createStudy,
	resolveSettings,
	settingsCatalog,
	solveAcOpfModule,
	solveResponseToSolution,
	type BrowserStudy,
	type Calculation,
	type Formulation,
	type McPfOptions,
	type ResolvedSettings,
	type SolveResponse
} from '@tellegen/engine';
import { solveBackendLabel } from '../format.js';
import type { Solution } from '../api.js';
import type { Controller } from '../controller.svelte.js';
import { LocalCase, MulticonductorCase, type SolvableCase } from '../state.svelte.js';

export function sameSettings(a: unknown, b: unknown): boolean {
	if (a && b && typeof a === 'object' && typeof b === 'object') {
		const x = a as Record<string, unknown>,
			y = b as Record<string, unknown>;
		return (
			Object.keys(x).length === Object.keys(y).length &&
			Object.keys(x).every((k) => sameSettings(x[k], y[k]))
		);
	}
	return Object.is(a, b);
}

export function acOpfSolveDetail(response: SolveResponse): string {
	const diagnostics = response.iterations;
	return diagnostics && !Array.isArray(diagnostics) && 'nlp_iterations' in diagnostics
		? `${diagnostics.total_nlp_iterations ?? diagnostics.nlp_iterations} NLP iterations${diagnostics.recovered_by ? ' (recovered)' : ''}`
		: solveBackendLabel('pounce-wasi');
}

/** Whether this engine build knows `name` as a calculation. */
export function isCalculation(name: string): name is Calculation {
	return settingsCatalog.calculations.some((c) => c.calculation === name);
}

export function defaultCalculationSettings(calculation: Calculation): ResolvedSettings {
	const descriptor = settingsCatalog.calculations.find((c) => c.calculation === calculation);
	const effective = descriptor && 'defaults' in descriptor ? { ...descriptor.defaults } : {};
	return {
		version: 1,
		calculation,
		engine_defaults: effective,
		baseline: effective,
		overrides: {},
		effective
	};
}
/** `solution` without the `settings_ignored` notices of a Study built from a
 * module whose saved settings are for another calculation. A Study built with
 * explicit settings reports none, and the import log already named any saved
 * option this engine does not know. */
export function withoutIgnoredSettings<T extends Solution>(solution: T): T {
	const diagnostics = solution.diagnostics;
	if (!diagnostics?.some((d) => d.code === 'settings_ignored')) return solution;
	return {
		...solution,
		diagnostics: diagnostics.filter((d) => d.code !== 'settings_ignored')
	};
}
export function hasCustomCalculationSettings(c: SolvableCase): boolean {
	const accepted = c.settings.accepted;
	return !!accepted && !sameSettings(accepted.effective, accepted.engine_defaults);
}

type SettingsHost = Pick<
	Controller,
	| 'acOpfAvailable'
	| 'app'
	| 'bumpRevision'
	| 'caseDeltas'
	| 'caseRatings'
	| 'caseStudies'
	| 'disposeStudy'
	| 'ensureStudyInputJson'
	| 'finishSolve'
	| 'formulationAvailable'
	| 'solveMultiCase'
>;

/** Settings validation and explicit runs share one transaction and cancellation path. */
export class CaseSettingsActions {
	constructor(
		private readonly host: SettingsHost,
		private readonly backend: {
			url: () => string | null;
			solves: Map<SolvableCase, AbortController>;
		}
	) {}
	/** Read selected next-run settings without changing a result or starting work. */
	inspectSettings = (c: SolvableCase | MulticonductorCase) => {
		const calculation =
			c.settings.calculation ?? (c instanceof MulticonductorCase ? 'mc_ac_pf' : c.formulation);
		const descriptor = settingsCatalog.calculations.find((d) => d.calculation === calculation);
		const baseline =
			c instanceof MulticonductorCase
				? {
						...c.distribution?.pf_options,
						...c.settings.baselines[calculation]
					}
				: { ...c.settings.baselines[calculation] };
		const selected =
			c instanceof MulticonductorCase
				? { ...c.mcOptions }
				: (c.settings.options[calculation] ?? {});
		const defaults = descriptor && 'defaults' in descriptor ? descriptor.defaults : {};
		const effective =
			c instanceof MulticonductorCase
				? { ...defaults, ...baseline, ...selected }
				: (c.settings.resolved[calculation]?.effective ?? {
						...defaults,
						...baseline,
						...selected
					});
		const accepted =
			c instanceof MulticonductorCase
				? c.result?.load_model?.options
				: c.settings.accepted?.effective;
		return {
			case_id: c.id,
			settings_revision: c.settings.revision,
			calculation,
			baseline,
			selected: effective,
			accepted: accepted ?? null,
			accepted_calculation:
				c instanceof MulticonductorCase
					? c.result
						? 'mc_ac_pf'
						: null
					: c.solution
						? c.formulation
						: null,
			pending:
				!!accepted &&
				(!sameSettings(effective, accepted) ||
					(!(c instanceof MulticonductorCase) && calculation !== c.formulation)),
			fields: descriptor?.fields ?? []
		};
	};

	/** Validate the complete candidate in Rust, then commit all fields atomically. */
	updateSettings = async (
		c: SolvableCase | MulticonductorCase,
		calculation: string,
		patch: Record<string, unknown>,
		expectedRevision: number,
		reset = false
	) => {
		if (c.solving) throw new Error('Settings cannot change during a calculation.');
		if (c.settings.revision !== expectedRevision) throw new Error('Settings changed; run again');
		if (
			!isCalculation(calculation) ||
			(c instanceof MulticonductorCase
				? calculation !== 'mc_ac_pf'
				: !this.host.formulationAvailable(c, calculation as Formulation))
		)
			throw new Error('This calculation is unavailable for the selected case.');
		if (c instanceof LocalCase && c.declaredFormulation && calculation !== c.declaredFormulation)
			throw new Error('This PowerIO instance requires its declared calculation.');
		// An undefined value (a cleared field) removes that override, so the
		// setting falls back to the case baseline or engine default.
		const overrides = reset
			? {}
			: Object.fromEntries(
					Object.entries({
						...c.settings.options[calculation],
						...patch
					}).filter(([, value]) => value !== undefined)
				);
		const caseRevision = c.revisionGeneration;
		const baseline =
			c instanceof MulticonductorCase
				? {
						...c.distribution?.pf_options,
						...c.settings.baselines[calculation]
					}
				: { ...c.settings.baselines[calculation] };
		let resolved: ResolvedSettings;
		if (calculation === 'acopf' && this.host.acOpfAvailable) {
			if (Object.keys(overrides).length)
				throw new Error('The isolated AC OPF backend has no public numerical settings.');
			resolved = {
				version: 1,
				calculation,
				engine_defaults: {},
				baseline: {},
				overrides: {},
				effective: {}
			};
		} else resolved = await resolveSettings({ calculation, baseline, overrides });
		if (
			c.solving ||
			c.revisionGeneration !== caseRevision ||
			c.settings.revision !== expectedRevision
		)
			throw new Error('Settings changed; run again');
		c.settings.options = {
			...c.settings.options,
			[calculation]: { ...resolved.overrides }
		};
		c.settings.resolved = { ...c.settings.resolved, [calculation]: resolved };
		c.settings.calculation = calculation;
		c.settings.revision++;
		if (c instanceof MulticonductorCase) c.mcOptions = { ...resolved.effective } as McPfOptions;
		return this.inspectSettings(c);
	};

	canCancelSettings = (c: SolvableCase | MulticonductorCase) =>
		c instanceof MulticonductorCase ? !!c.solveAbort : this.backend.solves.has(c);
	cancelSettings = (c: SolvableCase | MulticonductorCase) => {
		if (c instanceof MulticonductorCase) c.solveAbort?.abort();
		else this.backend.solves.get(c)?.abort();
	};

	/** Explicit solve. A candidate Study replaces the accepted Study only after success. */
	runSettings = async (c: SolvableCase | MulticonductorCase, signal?: AbortSignal) => {
		signal?.throwIfAborted();
		if (c.solving) throw new Error('Already running');
		const snapshot = this.inspectSettings(c);
		const initialRevision = c.revisionGeneration;
		if (c instanceof MulticonductorCase) {
			const resolved = await resolveSettings({
				calculation: 'mc_ac_pf',
				baseline: snapshot.baseline,
				overrides: snapshot.selected
			});
			if (
				c.revisionGeneration !== initialRevision ||
				c.settings.revision !== snapshot.settings_revision
			)
				throw new Error('Settings changed; run again');
			const result = await this.host.solveMultiCase(
				c,
				resolved.effective as McPfOptions,
				signal,
				c.mcRequiresEnergizedSolve
			);
			c.settings.accepted = resolved;
			return result;
		}
		const formulation = snapshot.calculation as Formulation;
		const resolved: ResolvedSettings =
			formulation === 'acopf'
				? {
						version: 1,
						calculation: formulation,
						engine_defaults: {},
						baseline: {},
						overrides: {},
						effective: {}
					}
				: await resolveSettings({
						calculation: formulation,
						baseline: snapshot.baseline,
						overrides: c.settings.options[formulation] ?? {}
					});
		signal?.throwIfAborted();
		if (c.solving) throw new Error('Already running');
		if (
			c.revisionGeneration !== initialRevision ||
			c.settings.revision !== snapshot.settings_revision
		)
			throw new Error('Settings changed; run again');
		const sequence = ++c.solveSeq;
		const revision = c.revisionGeneration;
		const settingsRevision = c.settings.revision;
		const started = performance.now();
		const cancellation = new AbortController();
		const cancel = () => cancellation.abort();
		signal?.addEventListener('abort', cancel, { once: true });
		this.backend.solves.set(c, cancellation);
		c.solving = true;
		let study: BrowserStudy | null = null;
		try {
			const input = await this.host.ensureStudyInputJson(c);
			if (!input) throw new Error('Case input missing; add the case again');
			let solution: Solution;
			let baseSolution: Solution | null = null;
			let iterations: typeof c.iterations = [];
			let solveDetail: string | null = null;
			if (formulation === 'acopf') {
				const wasmUrl = this.backend.url();
				if (!this.host.acOpfAvailable || !wasmUrl || c.perturbed)
					throw new Error('AC OPF requires its isolated backend and the unedited base case.');
				const response = await solveAcOpfModule(wasmUrl, input, cancellation.signal);
				solution = solveResponseToSolution(response);
				solveDetail = acOpfSolveDetail(response);
				baseSolution = solution;
			} else {
				study = await createStudy(input, formulation, {
					isolated: true,
					signal: cancellation.signal,
					settings: { calculation: formulation, overrides: resolved.effective }
				});
				baseSolution = await study.currentSolution();
				const committed = await study.commit(
					c.id,
					this.host.caseDeltas(c),
					this.host.caseRatings(c),
					null,
					cancellation.signal
				);
				solution = committed.solution;
				iterations = committed.iterations;
			}
			cancellation.signal.throwIfAborted();
			if (
				sequence !== c.solveSeq ||
				revision !== c.revisionGeneration ||
				settingsRevision !== c.settings.revision
			)
				throw new Error('Case changed during the calculation');
			this.host.disposeStudy(c);
			c.formulation = formulation;
			c.failedCalculation = null;
			c.settings.accepted = resolved;
			c.settings.revision++;
			c.settings.resolved = { ...c.settings.resolved, [formulation]: resolved };
			c.solution = solution;
			c.baseSolution = baseSolution;
			c.sensitivity = null;
			c.iterations = iterations;
			c.solveDetail = solveDetail;
			c.solveBackend = formulation === 'acopf' ? 'pounce-wasi' : 'clarabel-wasm';
			c.solveMs = Math.round(performance.now() - started);
			if (study) {
				this.host.caseStudies.set(c, {
					study,
					studyInputJson: input,
					formulation,
					baseSolution: baseSolution!,
					settings: resolved
				});
				study = null;
			}
			this.host.bumpRevision(c);
			if (this.host.app.displayMode !== 'demand')
				this.host.app.displayMode =
					formulation === 'acpf' || formulation === 'acopf' ? 'voltage' : 'price';
			this.host.finishSolve(c, sequence, null);
			return solution;
		} catch (error) {
			// The result on screen stays; the status line names the calculation that failed.
			if (sequence === c.solveSeq && (error as Error)?.name !== 'AbortError')
				c.failedCalculation = formulation;
			throw error;
		} finally {
			study?.free();
			if (sequence === c.solveSeq) c.solving = false;
			if (this.backend.solves.get(c) === cancellation) this.backend.solves.delete(c);
			signal?.removeEventListener('abort', cancel);
		}
	};
}
