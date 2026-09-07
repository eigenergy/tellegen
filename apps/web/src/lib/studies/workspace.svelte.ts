import {
	IndexedDbStudyStore,
	StudyDocumentController,
	type CreateStudy,
	type StudyBundle,
	type StudyOperation,
	type StudyOperationResult,
	type Comparison,
	type Network,
	type StudyView
} from '@tellegen/engine';
import { capacityGoal, capacityOutcome, type CapacityStudyBinding } from './capacity-compat.js';
import type { CapacityPlanSpecJson } from '@tellegen/svelte';
import type { Controller } from '@tellegen/svelte';
import { caseRevision } from '../webmcp/tellegen-adapter.js';
import { trackUsage, type EventData } from '../analytics/client.js';

type CaseEvidenceContext = {
	studyId: string;
	revision: number;
	state: string;
	goal: string | null;
	caseId: string;
	caseRevision: string;
};

export type GoalDraft = Omit<
	CreateStudy,
	'input' | 'base_input' | 'id' | 'solution' | 'view' | 'display'
>;

/** One workspace session shared by browser controls and WebMCP. */
export class StudyWorkspace {
	bundle = $state.raw<StudyBundle | null>(null);
	comparison = $state.raw<Comparison | null>(null);
	saved = $state.raw<Array<{ id: string; title: string; revision: number }>>([]);
	busy = $state(false);
	error = $state<string | null>(null);
	network = $state.raw<Network | null>(null);
	#baseNetwork: Network | null = null;
	#controller: StudyDocumentController | null = null;
	#store: IndexedDbStudyStore | null = null;
	#cancel: AbortController | null = null;
	#geometry = new Map<string, Network>();
	#capacityApprovals = new Map<string, string>();
	#caseAnchor: { caseId: string; revision: string; state: string } | null = null;

	constructor(readonly grid: Controller) {}
	get store(): IndexedDbStudyStore {
		return (this.#store ??= new IndexedDbStudyStore());
	}
	get document() {
		return this.bundle?.document ?? null;
	}
	get goal() {
		const d = this.document;
		return d?.active_goal ? d.goals[d.active_goal] : null;
	}

	async refreshSaved() {
		this.saved = await this.store.list();
	}
	async initialize() {
		try {
			await this.refreshSaved();
		} catch (error) {
			this.error = String(error);
		}
	}
	async #run<T>(
		run: (signal: AbortSignal) => Promise<T>,
		signal?: AbortSignal,
		event = 'study.operation',
		metrics: EventData = {}
	): Promise<T> {
		if (this.busy)
			throw new Error('A Study operation is running; wait or cancel it before continuing');
		const started = performance.now();
		this.busy = true;
		this.error = null;
		const abort = new AbortController();
		this.#cancel = abort;
		const cancel = () => abort.abort(signal?.reason);
		if (signal?.aborted) cancel();
		else signal?.addEventListener('abort', cancel, { once: true });
		try {
			const result = await run(abort.signal);
			const experiment = (result as StudyOperationResult | undefined)?.experiment;
			const evidence = experiment ? this.document?.experiments[experiment] : undefined;
			trackUsage(event, {
				...metrics,
				result: abort.signal.aborted ? 'cancelled' : 'completed',
				duration_ms: performance.now() - started,
				solve_count: evidence?.solve_count,
				trial_count: evidence?.trials.length
			});
			return result;
		} catch (error) {
			trackUsage(event, {
				...metrics,
				result: abort.signal.aborted ? 'cancelled' : 'failed',
				duration_ms: performance.now() - started
			});
			this.error = error instanceof Error ? error.message : String(error);
			throw error;
		} finally {
			signal?.removeEventListener('abort', cancel);
			this.busy = false;
			this.#cancel = null;
		}
	}
	cancel() {
		this.#cancel?.abort(new DOMException('Study operation cancelled', 'AbortError'));
	}
	async create(
		draft: GoalDraft,
		caseId: string,
		expectedCaseRevision: string,
		signal?: AbortSignal,
		show = true
	) {
		return this.#run(
			async (abort) => {
				const c = this.grid.activeSolvable;
				if (!c || c.id !== caseId || caseRevision(c) !== expectedCaseRevision || c.solving)
					throw new Error('Case changed; inspect the current case before creating a Study');
				const base_input = await this.grid.ensureStudyInputJson(c);
				const captured = await this.grid.captureSavedCase(c);
				const geometry = c.network;
				const display = geometry
					? {
							case_id: c.id,
							camera: geometry.coordinate_space === 'diagram' ? null : this.grid.app.camera,
							diagram_camera:
								geometry.coordinate_space === 'diagram' &&
								this.grid.app.diagramCamera?.caseId === c.id
									? {
											center: this.grid.app.diagramCamera.center,
											scale: this.grid.app.diagramCamera.scale
										}
									: null,
							layers:
								'diagram' in c && c.diagram
									? [c.diagram.layer, ...(await this.grid.caseGeographyLayers(c))]
									: [],
							geo_layer: JSON.stringify({
								type: 'FeatureCollection',
								powerio_geo: {
									space: geometry.coordinate_space ?? 'geographic',
									kind: geometry.synthetic_coords ? 'synthetic' : 'source'
								},
								features: [
									...geometry.buses.map((b) => ({
										type: 'Feature',
										properties: {
											target: 'bus',
											id: String(b.id),
											...(b.uid ? { uid: b.uid } : {})
										},
										geometry: { type: 'Point', coordinates: [b.lon, b.lat] }
									})),
									...geometry.branches
										.filter((b) => b.path.length >= 2)
										.map((b) => ({
											type: 'Feature',
											properties: {
												target: 'branch',
												branch_id: String(b.id),
												...(b.uid ? { uid: b.uid } : {}),
												from: String(b.from),
												to: String(b.to)
											},
											geometry: { type: 'LineString', coordinates: b.path }
										}))
								]
							})
						}
					: undefined;
				abort.throwIfAborted();
				if (this.grid.activeSolvable !== c || caseRevision(c) !== expectedCaseRevision)
					throw new Error('Case changed while capturing the Study starting point; retry');
				const controller = await StudyDocumentController.create(
					{
						...draft,
						...captured,
						id: crypto.randomUUID(),
						base_input,
						display,
						model_details: c.network?.model_details
					},
					this.store,
					undefined,
					abort
				);
				this.#controller = controller;
				this.#caseAnchor = {
					caseId,
					revision: expectedCaseRevision,
					state: controller.bundle.document.applied_state!
				};
				this.comparison = null;
				await this.#publish(false, show);
				return this.summary();
			},
			signal,
			'study.save'
		);
	}
	async open(id: string) {
		return this.#run(
			async () => {
				this.#controller = await StudyDocumentController.open(id, this.store);
				this.#caseAnchor = null;
				this.comparison = null;
				await this.#publish(true);
			},
			undefined,
			'study.open'
		);
	}
	async import(text: string) {
		return this.#run(
			async () => {
				const imported: unknown = JSON.parse(text);
				if (imported && typeof imported === 'object' && 'document' in imported) {
					const document = imported.document;
					if (
						document &&
						typeof document === 'object' &&
						'id' in document &&
						typeof document.id === 'string' &&
						(await this.store.load(document.id))
					) {
						throw new Error('This study is already saved. Open it from Saved study.');
					}
				}
				this.#controller = await StudyDocumentController.import(text, this.store);
				this.#caseAnchor = null;
				this.comparison = null;
				await this.#publish(true);
			},
			undefined,
			'study.import'
		);
	}
	export(): string {
		if (!this.#controller) throw new Error('No Study is open');
		const output = this.#controller.export();
		trackUsage('study.export', { result: 'completed' });
		return output;
	}
	closeView() {
		this.grid.app.studyView = null;
	}
	async showView() {
		await this.#display(true);
	}
	async execute(
		studyId: string,
		revision: number,
		operation: StudyOperation,
		signal?: AbortSignal,
		show = true
	): Promise<StudyOperationResult> {
		return this.#run(
			async (abort) => {
				if (!this.#controller || this.document?.id !== studyId)
					throw new Error('Open the requested Study before continuing');
				const result = await this.#controller.execute(
					{ expected_revision: revision, operation },
					abort
				);
				if (operation.kind === 'record_evidence') {
					this.bundle = this.#controller.bundle;
				} else {
					this.comparison = result.comparison ?? null;
					await this.#publish(false, show);
				}
				return result;
			},
			signal,
			'study.operation',
			{ operation: operation.kind }
		);
	}
	async applyFromUser(proposal: string) {
		return this.#run(
			async (abort) => {
				if (!this.#controller) throw new Error('No Study is open');
				const token = this.#controller.recordUserApproval(proposal);
				const result = await this.#controller.applyApprovedProposal(token, abort);
				await this.#publish(false);
				return result;
			},
			undefined,
			'study.operation',
			{ operation: 'apply' }
		);
	}
	get demandRows() {
		const base = new Map(this.#baseNetwork?.buses.map((b) => [b.id, b.demand_mw]) ?? []);
		return (this.network?.buses ?? []).map((b) => ({
			bus: b.id,
			name: b.name ?? undefined,
			baseMw: base.get(b.id) ?? b.demand_mw,
			currentMw: b.demand_mw,
			deltaMw: b.demand_mw - (base.get(b.id) ?? b.demand_mw)
		}));
	}
	async #userEdit(operation: StudyOperation) {
		return this.#run(
			async (abort) => {
				if (!this.#controller || !this.document) throw new Error('No Study is open');
				const result = await this.#controller.execute(
					{ expected_revision: this.document.revision, operation },
					abort
				);
				this.bundle = this.#controller.bundle;
				const state = this.document!.recommended_state;
				if (result.experiment && state && this.document!.states[state].solution) {
					const token = this.#controller.recordUserApproval(result.experiment);
					await this.#controller.applyApprovedProposal(token, abort);
				}
				await this.#publish(false);
				return result;
			},
			undefined,
			'study.operation',
			{ operation: operation.kind }
		);
	}
	async editDemandFromUser(changes: Array<{ bus: number | string; delta_mw: number }>) {
		const d = this.document;
		if (!d?.inspected_state) throw new Error('No saved case is selected');
		return this.#userEdit({
			kind: 'edit_demand',
			state: d.inspected_state,
			goal: d.active_goal ?? null,
			constrain_to_goal: false,
			changes,
			rationale: 'Update bus demand'
		});
	}
	async resetFromUser() {
		const d = this.document;
		if (!d?.inspected_state) throw new Error('No saved case is selected');
		return this.#userEdit({
			kind: 'restore_base',
			state: d.inspected_state,
			goal: d.active_goal ?? null,
			rationale: 'Reset to base case'
		});
	}
	captureCaseEvidence(): CaseEvidenceContext | null {
		const c = this.grid.activeSolvable,
			anchor = this.#caseAnchor,
			d = this.document;
		if (!c || !anchor || !d || c.id !== anchor.caseId || caseRevision(c) !== anchor.revision)
			return null;
		return {
			studyId: d.id,
			revision: d.revision,
			state: anchor.state,
			goal: d.active_goal ?? null,
			caseId: c.id,
			caseRevision: anchor.revision
		};
	}
	async recordCaseEvidence(
		context: CaseEvidenceContext,
		tool: string,
		input: unknown,
		result: unknown,
		signal: AbortSignal
	) {
		const c = this.grid.activeSolvable;
		if (!c || c.id !== context.caseId || caseRevision(c) !== context.caseRevision)
			throw new Error('Case changed during inspection; retry before attaching Study evidence');
		await this.execute(
			context.studyId,
			context.revision,
			{
				kind: 'record_evidence',
				state: context.state,
				goal: context.goal,
				sensitivity: tool === 'analyze_sensitivity',
				rationale: `Inspect the captured electrical state with ${tool}`,
				evidence: { tool, input, result, case_revision: context.caseRevision }
			},
			signal
		);
	}

	async planCapacity(
		spec: CapacityPlanSpecJson,
		caseId: string,
		revision: string,
		signal: AbortSignal,
		elements: Readonly<Record<string, number>>
	) {
		await this.create(capacityGoal(spec, elements), caseId, revision, signal, false);
		const d = this.document!;
		const result = await this.execute(
			d.id,
			d.revision,
			{
				kind: 'propose',
				state: d.inspected_state!,
				goal: d.active_goal!,
				options: {
					max_solves: Math.max(
						0,
						spec.exact_solve_budget -
							Object.values(d.experiments).reduce(
								(total, activity) => total + activity.solve_count,
								0
							)
					),
					beam_width: 2,
					max_iterations: 256,
					min_improvement:
						1e-4 * (Math.max(...spec.objective.weights.map((w) => Math.abs(w.weight))) || 1)
				},
				rationale:
					'Explore capacity upgrades using the implicit weighted-price gradient and exact solves.'
			},
			signal,
			false
		);
		const current = this.document!;
		const binding: CapacityStudyBinding = {
			studyId: current.id,
			revision: current.revision,
			proposal: result.experiment!,
			goal: current.active_goal!,
			base: d.inspected_state!,
			state: current.recommended_state ?? d.inspected_state!
		};
		return { outcome: capacityOutcome(this.bundle!, result.experiment!, spec), binding };
	}
	capacityCurrent(binding: CapacityStudyBinding) {
		const d = this.document;
		return (
			!!d &&
			d.id === binding.studyId &&
			d.revision >= binding.revision &&
			d.active_goal === binding.goal &&
			d.recommended_state === binding.state &&
			d.experiments[binding.proposal]?.start_state === binding.base
		);
	}
	approveCapacity(binding: CapacityStudyBinding) {
		if (!this.capacityCurrent(binding) || this.busy)
			throw new Error('Capacity proposal changed; review the current Study');
		this.#capacityApprovals.set(
			binding.proposal,
			this.#controller!.recordUserApproval(binding.proposal)
		);
	}
	capacityApproved(binding: CapacityStudyBinding) {
		const token = this.#capacityApprovals.get(binding.proposal);
		return this.capacityCurrent(binding) && !!token && !!this.#controller?.isApprovalCurrent(token);
	}
	async applyCapacity(binding: CapacityStudyBinding, publish: () => boolean, signal: AbortSignal) {
		return this.#run(async (abort) => {
			if (!this.capacityApproved(binding))
				throw new Error('Capacity approval expired; review the current Study');
			const token = this.#capacityApprovals.get(binding.proposal)!;
			const result = await this.#controller!.applyApprovedProposal(token, abort);
			this.#capacityApprovals.delete(binding.proposal);
			// The durable Study is authoritative; the caller refreshes its matching case synchronously.
			if (publish()) {
				const c = this.grid.activeSolvable!;
				this.#caseAnchor = { caseId: c.id, revision: caseRevision(c), state: binding.state };
			}
			await this.#publish(false, false);
			return result;
		}, signal);
	}

	summary() {
		const d = this.document;
		if (!d) return { open: false, saved: this.saved.slice(0, 10) };
		const recent = d.experiment_order.slice(-2).map((id) => {
			const e = d.experiments[id];
			return {
				id,
				kind: e.kind,
				start_state: e.start_state,
				solve_count: e.solve_count,
				trial_count: e.trials.length,
				termination: e.termination
			};
		});
		return {
			id: d.id,
			title: d.title,
			revision: d.revision,
			active_goal: d.active_goal ?? null,
			inspected_state: d.inspected_state,
			recommended_state: d.recommended_state,
			applied_state: d.applied_state,
			state_count: Object.keys(d.states).length,
			experiment_count: d.experiment_order.length,
			recent_experiments: recent
		};
	}
	async #publish(frame: boolean, show = true) {
		this.bundle = this.#controller!.bundle;
		// Persistence completed; display failures must not report a failed mutation.
		try {
			if (show) await this.#display(frame);
			await this.refreshSaved();
		} catch (error) {
			this.grid.app.studyView = null;
			this.error = `Study saved, but the display could not refresh: ${String(error)}`;
		}
	}
	async #display(frame: boolean) {
		const b = this.bundle,
			d = b?.document;
		if (!b || !d?.inspected_state) {
			this.grid.app.studyView = null;
			return;
		}
		const state = d.states[d.inspected_state];
		if (state.formulation === 'dcpf' || state.formulation === 'acopf')
			throw new Error('This saved formulation is not supported by the Study viewer');
		const geo = d.display ? b.artifacts[d.display.geography].text : undefined;
		const geometryKey = state.input + ':' + (d.display?.geography ?? '');
		let network = this.#geometry.get(geometryKey);
		if (!network) {
			network = await this.grid.projectStudyInput(b.artifacts[state.input].text, geo);
			if (this.#geometry.size >= 8) this.#geometry.delete(this.#geometry.keys().next().value!);
			this.#geometry.set(geometryKey, network);
		}
		this.network = network;
		this.#baseNetwork = await this.grid.projectStudyInput(
			b.artifacts[d.base_input ?? state.input].text,
			geo
		);
		const solution = state.view ? (JSON.parse(b.artifacts[state.view].text) as StudyView) : null;
		this.grid.app.studyView = {
			id: d.inspected_state,
			label: state.label,
			network,
			solution,
			caseId: d.display?.case_id ?? d.id,
			studyId: d.id,
			revision: d.revision,
			formulation: state.formulation,
			inputJson: b.artifacts[state.input].text,
			baseDemandMw: Object.fromEntries(
				this.#baseNetwork.buses.map((b) => [String(b.id), b.demand_mw])
			)
		};
		if (!solution?.lmp?.length && solution?.vm?.length) this.grid.app.displayMode = 'voltage';
		if (frame) {
			if (network.coordinate_space === 'diagram' && d.display?.diagram_camera) {
				this.grid.app.requestDiagramCamera(d.display.case_id, {
					center: [d.display.diagram_camera.center[0], d.display.diagram_camera.center[1]],
					scale: d.display.diagram_camera.scale
				});
			} else if (network.coordinate_space !== 'diagram' && d.display?.camera)
				this.grid.app.requestCamera({
					...d.display.camera,
					center: [d.display.camera.center[0], d.display.camera.center[1]]
				});
			else void this.grid.app.requestFrame('all');
		}
	}
	dispose() {
		this.cancel();
		this.closeView();
		void this.#store?.close();
	}
}
