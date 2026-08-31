import { tick } from 'svelte';
import {
	FORMULATIONS,
	createStudy,
	type BrowserStudy,
	type CapacityPlanBusWeightJson,
	type CapacityPlanSpecJson,
	type Controller,
	type Formulation,
	type NetworkBranch,
	type NetworkBus,
	type Solution,
	type SensitivityColumn,
	type SolveIteration,
	type SolvableCase
} from '@tellegen/svelte';
import {
	TellegenToolError,
	type AnalyzeSensitivityInput,
	type ApplyCapacityPlanInput,
	type FocusNetworkInput,
	type ProposeCapacityPlanInput,
	type PreviewCaseUpdateInput,
	type QueryNetworkInput,
	type ResetCaseInput,
	type TellegenPlanningAdapter,
	type TellegenWebMcpAdapter,
	type ToolPayload,
	type UpdateCaseInput
} from '@tellegen/webmcp';
import type { PlanningActivityStore, StagedCapacityProposal } from './planning-activity.svelte.js';

const OUTPUT_ID_LENGTH = 64;
type FormulationOption = { id: Formulation; disabled?: boolean };
type QueryRow = Record<string, string | number | boolean | null> & {
	element_id: string;
	legacy_id: number;
};

function isDisplayOnlyElement(element: { editable?: boolean } | null | undefined): boolean {
	return element?.editable === false;
}

function clip(value: string, length = OUTPUT_ID_LENGTH): string {
	return value.length <= length ? value : `${value.slice(0, length - 1)}…`;
}

function finite(value: number | null | undefined): number | null {
	return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function elementId(element: { id: number; uid?: string | null }, kind: 'bus' | 'branch'): string {
	return element.uid ?? `${kind}:${element.id}`;
}

function randomId(prefix: string): string {
	const uuid = globalThis.crypto?.randomUUID?.();
	if (uuid) return `${prefix}-${uuid}`;
	if (globalThis.crypto?.getRandomValues) {
		const bytes = globalThis.crypto.getRandomValues(new Uint8Array(16));
		return `${prefix}-${[...bytes].map((value) => value.toString(16).padStart(2, '0')).join('')}`;
	}
	const id = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
	return `${prefix}-${id}`;
}

const SESSION_ID = randomId('session');
let revisionSequence = 0;
const revisions = new WeakMap<SolvableCase, { fingerprint: string; revision: string }>();

export function webMcpSessionId(): string {
	return SESSION_ID;
}

interface CaseLookup {
	network: NonNullable<SolvableCase['network']>;
	solution: Solution | null;
	buses: Map<string, NetworkBus>;
	branches: Map<string, NetworkBranch>;
	prices: Map<number, number>;
	flows: Map<number, Solution['flows'][number]>;
}

const lookups = new WeakMap<SolvableCase, CaseLookup>();
const traceDigests = new WeakMap<SolvableCase, { moduleJson: string; digest: Promise<string> }>();

function declaredSourceDigest(moduleJson: string): string | null {
	try {
		const stored = JSON.parse(moduleJson) as {
			sources?: Array<{ digest?: { algorithm?: unknown; value?: unknown } }>;
		};
		for (const source of stored.sources ?? []) {
			const digest = source.digest;
			if (
				digest?.algorithm === 'sha256' &&
				typeof digest.value === 'string' &&
				/^[0-9a-f]{64}$/.test(digest.value)
			) {
				return `sha256:${digest.value}`;
			}
		}
	} catch {
		// Study construction remains the authority for malformed module errors.
	}
	return null;
}

async function sha256(moduleJson: string): Promise<string> {
	const declared = declaredSourceDigest(moduleJson);
	if (declared) return declared;
	const subtle = globalThis.crypto?.subtle;
	if (!subtle) {
		throw new TellegenToolError(
			'TRACE_UNAVAILABLE',
			'this browser cannot compute a SHA-256 source digest'
		);
	}
	const bytes = new TextEncoder().encode(moduleJson);
	const hash = new Uint8Array(await subtle.digest('SHA-256', bytes));
	return `sha256:${[...hash].map((value) => value.toString(16).padStart(2, '0')).join('')}`;
}

async function sourceDigest(
	ctrl: Controller,
	c: SolvableCase,
	knownModuleJson?: string
): Promise<string> {
	const moduleJson = knownModuleJson ?? (await ctrl.ensureStudyInputJson(c));
	if (!moduleJson) {
		throw new TellegenToolError('CASE_DATA_UNAVAILABLE', 'the PowerIO module is unavailable');
	}
	const cached = traceDigests.get(c);
	if (cached?.moduleJson === moduleJson) return cached.digest;
	const digest = sha256(moduleJson);
	traceDigests.set(c, { moduleJson, digest });
	return digest;
}

function addAliases<T extends { id: number; uid?: string | null }>(
	map: Map<string, T>,
	element: T,
	kind: 'bus' | 'branch'
): void {
	map.set(String(element.id), element);
	map.set(`${kind}:${element.id}`, element);
	if (element.uid) map.set(element.uid, element);
}

function caseLookup(c: SolvableCase): CaseLookup {
	const network = c.network;
	if (!network)
		throw new TellegenToolError('CASE_NOT_READY', 'the active case network is still loading');
	const cached = lookups.get(c);
	if (cached?.network === network && cached.solution === c.solution) return cached;
	const buses = new Map<string, NetworkBus>();
	const branches = new Map<string, NetworkBranch>();
	for (const bus of network.buses) addAliases(buses, bus, 'bus');
	for (const branch of network.branches) addAliases(branches, branch, 'branch');
	const next: CaseLookup = {
		network,
		solution: c.solution,
		buses,
		branches,
		prices: new Map(c.solution?.prices.map((entry) => [entry.bus, entry.value]) ?? []),
		flows: new Map(c.solution?.flows.map((entry) => [entry.branch, entry]) ?? [])
	};
	lookups.set(c, next);
	return next;
}

function resolveBus(c: SolvableCase, id: string): NetworkBus {
	const bus = caseLookup(c).buses.get(id);
	if (!bus)
		throw new TellegenToolError('ELEMENT_NOT_FOUND', `bus ${clip(id)} is not in the active case`);
	return bus;
}

function resolveBranch(c: SolvableCase, id: string): NetworkBranch {
	const branch = caseLookup(c).branches.get(id);
	if (!branch) {
		throw new TellegenToolError(
			'ELEMENT_NOT_FOUND',
			`branch ${clip(id)} is not in the active case`
		);
	}
	return branch;
}

function sortedEdits(edits: Record<number, number>): Array<[number, number]> {
	return Object.entries(edits)
		.map(([id, value]) => [Number(id), value] as [number, number])
		.sort(([a], [b]) => a - b);
}

function caseFingerprint(c: SolvableCase): string {
	return JSON.stringify([
		c.id,
		c.revisionGeneration,
		c.formulation,
		sortedEdits(c.deltas),
		sortedEdits(c.ratings)
	]);
}

/** An opaque session-scoped optimistic concurrency token. It advances whenever
 * the mutable case state observed by WebMCP changes. */
export function caseRevision(c: SolvableCase): string {
	const fingerprint = caseFingerprint(c);
	const current = revisions.get(c);
	if (current?.fingerprint === fingerprint) return current.revision;
	const revision = `${SESSION_ID}:r${++revisionSequence}`;
	revisions.set(c, { fingerprint, revision });
	return revision;
}

function activeCase(ctrl: Controller, expectedCaseId?: string): SolvableCase {
	const c = ctrl.activeSolvable;
	if (!c) throw new TellegenToolError('NO_ACTIVE_CASE', 'tellegen has no active solvable case');
	if (expectedCaseId !== undefined && c.id !== expectedCaseId) {
		throw new TellegenToolError(
			'STALE_CASE',
			`active case changed; inspect_case now reports ${clip(c.id)}`
		);
	}
	if (!c.network)
		throw new TellegenToolError('CASE_NOT_READY', 'the active case network is still loading');
	return c;
}

function requireRevision(c: SolvableCase, expected: string): void {
	const current = caseRevision(c);
	if (current !== expected) {
		throw new TellegenToolError(
			'STALE_REVISION',
			`case edits changed; inspect_case now reports revision ${current}`
		);
	}
}

function solutionPayload(c: SolvableCase): ToolPayload {
	return {
		objective: finite(c.solution?.objective),
		base_objective: finite(c.baseSolution?.objective),
		objective_delta: finite(
			c.solution && c.baseSolution ? c.solution.objective - c.baseSolution.objective : null
		),
		binding_branches:
			c.solution?.flows.filter((flow: Solution['flows'][number]) => flow.loading >= 0.999).length ??
			0,
		solve_backend: c.solveBackend ?? 'none',
		solve_ms: finite(c.solveMs)
	};
}

function caseSnapshot(c: SolvableCase): ToolPayload {
	return {
		revision: caseRevision(c),
		formulation: c.formulation,
		demand_edit_count: Object.keys(c.deltas).length,
		rating_edit_count: Object.keys(c.ratings).length,
		objective: finite(c.solution?.objective),
		objective_delta: finite(
			c.solution && c.baseSolution ? c.solution.objective - c.baseSolution.objective : null
		),
		binding_branches:
			c.solution?.flows.filter((flow: Solution['flows'][number]) => flow.loading >= 0.999).length ??
			0
	};
}

async function inspect(
	ctrl: Controller,
	planning: PlanningActivityStore | undefined,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = ctrl.activeSolvable;
	if (!c) {
		return {
			active: false,
			session_id: SESSION_ID,
			message: 'Select or load a balanced network case before using case tools.'
		};
	}
	const demand = sortedEdits(c.deltas);
	const ratings = sortedEdits(c.ratings);
	const staged = planning?.proposal;
	let source_digest: string | null = null;
	try {
		source_digest = await sourceDigest(ctrl, c);
	} catch (error) {
		if (!(error instanceof TellegenToolError) || error.code !== 'CASE_DATA_UNAVAILABLE')
			throw error;
	}
	signal.throwIfAborted();
	return {
		active: true,
		session_id: SESSION_ID,
		case_id: c.id,
		source_digest,
		label: clip(ctrl.caseName(c), 80),
		revision: caseRevision(c),
		formulation: c.formulation,
		available_formulations: FORMULATIONS.filter((entry: FormulationOption) => !entry.disabled).map(
			(entry: FormulationOption) => entry.id
		),
		solving: c.solving,
		network: {
			buses: c.network?.buses.length ?? 0,
			branches: c.network?.branches.length ?? 0,
			base_mva: finite(c.network?.base_mva)
		},
		edits: {
			demand_count: demand.length,
			rating_count: ratings.length,
			demand_sample: demand
				.slice(0, 6)
				.map(([id, delta]) => ({ bus_id: String(id), delta_mw: delta })),
			rating_sample: ratings
				.slice(0, 6)
				.map(([id, delta]) => ({ branch_id: String(id), delta_mw: delta }))
		},
		selection:
			ctrl.app.selectedBus !== null
				? { kind: 'bus', element_id: elementId(resolveBus(c, String(ctrl.app.selectedBus)), 'bus') }
				: ctrl.app.selectedBranch !== null
					? {
							kind: 'branch',
							element_id: elementId(resolveBranch(c, String(ctrl.app.selectedBranch)), 'branch')
						}
					: null,
		solution: solutionPayload(c),
		staged_proposal:
			staged && staged.caseId === c.id && staged.revision === caseRevision(c)
				? {
						proposal_id: staged.proposalId,
						change_count: staged.changes.length,
						approved: planning?.isApproved(staged) ?? false
					}
				: null
	};
}

function query(ctrl: Controller, input: QueryNetworkInput): ToolPayload {
	const c = activeCase(ctrl, input.caseId);
	const lookup = caseLookup(c);
	const requested = input.elementIds
		? new Set(
				input.elementIds.map((id) =>
					input.elementKind === 'bus' ? resolveBus(c, id).id : resolveBranch(c, id).id
				)
			)
		: null;
	const sortBy = input.sortBy ?? (input.elementKind === 'bus' ? 'demand_mw' : 'loading');
	if (
		(input.elementKind === 'bus' && ['loading', 'flow_mw', 'rating_mw'].includes(sortBy)) ||
		(input.elementKind === 'branch' && ['demand_mw', 'generation_mw', 'price'].includes(sortBy))
	) {
		throw new TellegenToolError(
			'INVALID_SORT',
			`${sortBy} does not apply to ${input.elementKind} elements`
		);
	}

	const rows: QueryRow[] =
		input.elementKind === 'bus'
			? c.network!.buses.map((bus: NetworkBus) => ({
					element_id: elementId(bus, 'bus'),
					legacy_id: bus.id,
					demand_mw: finite(bus.demand_mw + (c.deltas[bus.id] ?? 0)),
					base_demand_mw: finite(bus.demand_mw),
					generation_mw: finite(bus.gen_mw),
					price: finite(lookup.prices.get(bus.id)),
					editable: !isDisplayOnlyElement(bus)
				}))
			: c.network!.branches.map((branch: NetworkBranch) => {
					const flow = lookup.flows.get(branch.id);
					return {
						element_id: elementId(branch, 'branch'),
						legacy_id: branch.id,
						from_bus: branch.from,
						to_bus: branch.to,
						rating_mw: finite(branch.rate_mw + (c.ratings[branch.id] ?? 0)),
						flow_mw: finite(flow?.mw),
						loading: finite(flow?.loading),
						editable: !isDisplayOnlyElement(branch) && branch.rate_mw > 0
					};
				});

	const filtered = requested ? rows.filter((row) => requested.has(row.legacy_id)) : rows;
	const value = (row: (typeof rows)[number]): string | number => {
		if (sortBy === 'id') return row.element_id;
		const candidate = row[sortBy as keyof typeof row];
		return typeof candidate === 'number' ? candidate : Number.NEGATIVE_INFINITY;
	};
	filtered.sort((a, b) => {
		const left = value(a);
		const right = value(b);
		const compared =
			typeof left === 'string' && typeof right === 'string'
				? left.localeCompare(right)
				: Number(left) - Number(right);
		return input.direction === 'asc' ? compared : -compared;
	});
	return {
		case_id: c.id,
		revision: caseRevision(c),
		element_kind: input.elementKind,
		total: rows.length,
		returned: Math.min(input.limit, filtered.length),
		elements: filtered.slice(0, input.limit)
	};
}

async function sensitivity(
	ctrl: Controller,
	input: AnalyzeSensitivityInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = activeCase(ctrl, input.caseId);
	const revision = caseRevision(c);
	signal.throwIfAborted();
	const resolved =
		input.target.kind === 'bus'
			? resolveBus(c, input.target.elementId)
			: resolveBranch(c, input.target.elementId);
	const target = input.target.kind === 'bus' ? { bus: resolved.id } : { branch: resolved.id };
	const studyInputJson = await ctrl.ensureStudyInputJson(c);
	if (!studyInputJson)
		throw new TellegenToolError('CASE_DATA_UNAVAILABLE', 'the PowerIO module is unavailable');
	const study = await createStudy(studyInputJson, c.formulation, {
		isolated: true,
		signal
	});
	let column: SensitivityColumn | null;
	try {
		const solved = await study.commit(c.id, c.deltas, c.ratings, target, signal);
		column = solved.sensitivity;
	} finally {
		study.free();
	}
	signal.throwIfAborted();
	requireRevision(activeCase(ctrl, input.caseId), revision);
	if (!column) {
		throw new TellegenToolError(
			'SENSITIVITY_UNAVAILABLE',
			'the browser engine did not return a sensitivity column'
		);
	}
	const values = [...column.values]
		.sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
		.slice(0, input.limit)
		.map((entry) => {
			const bus = resolveBus(c, String(entry.bus));
			return {
				bus_id: elementId(bus, 'bus'),
				legacy_id: bus.id,
				value: finite(entry.value)
			};
		});
	return {
		case_id: c.id,
		revision,
		target: {
			kind: input.target.kind,
			element_id: elementId(resolved, input.target.kind)
		},
		units: clip(column.units, 48),
		responses: values
	};
}

async function focus(
	ctrl: Controller,
	input: FocusNetworkInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = activeCase(ctrl, input.caseId);
	signal.throwIfAborted();
	let focusedId: string;
	if (input.target.kind === 'bus') {
		const bus = resolveBus(c, input.target.elementId);
		focusedId = elementId(bus, 'bus');
		if (ctrl.isBackendCase(c)) await ctrl.selectBus(c.id, bus.id);
		else await ctrl.selectLocalBus(c.id, bus.id);
	} else {
		const branch = resolveBranch(c, input.target.elementId);
		focusedId = elementId(branch, 'branch');
		if (ctrl.isBackendCase(c)) await ctrl.selectBranch(c.id, branch.id, { focus: true });
		else await ctrl.selectLocalBranch(c.id, branch.id, { focus: true });
	}
	// Selection commits synchronously when the controller call starts. From
	// that point this is a definite visible mutation, so a later cancellation
	// must not report that nothing happened.
	await tick();
	return {
		case_id: c.id,
		revision: caseRevision(c),
		focused: {
			kind: input.target.kind,
			element_id: focusedId
		},
		sensitivity_loaded: ctrl.selectedSensitivity !== null
	};
}

function validateDemand(ctrl: Controller, c: SolvableCase, id: string, value: number): NetworkBus {
	const bus = resolveBus(c, id);
	if (isDisplayOnlyElement(bus)) {
		throw new TellegenToolError('EDIT_NOT_SUPPORTED', `bus ${clip(id)} is display-only`);
	}
	const bounds = ctrl.demandBounds('full', bus, value);
	if (value < bounds.min || value > bounds.max) {
		throw new TellegenToolError(
			'EDIT_OUT_OF_RANGE',
			`bus ${clip(id)} demand delta must be from ${bounds.min} to ${bounds.max} MW`
		);
	}
	return bus;
}

function ratingDeltaBounds(branch: NetworkBranch): { min: number; max: number } {
	const span = Math.min(50, Math.max(5, 0.2 * branch.rate_mw));
	return { min: Math.max(-(branch.rate_mw - 1), -span), max: span };
}

function validateRating(c: SolvableCase, id: string, value: number): NetworkBranch {
	const branch = resolveBranch(c, id);
	if (isDisplayOnlyElement(branch) || branch.rate_mw <= 0) {
		throw new TellegenToolError('EDIT_NOT_SUPPORTED', `branch ${clip(id)} has no editable rating`);
	}
	const bounds = ratingDeltaBounds(branch);
	if (value < bounds.min || value > bounds.max) {
		throw new TellegenToolError(
			'EDIT_OUT_OF_RANGE',
			`branch ${clip(id)} rating delta must be from ${bounds.min} to ${bounds.max} MW`
		);
	}
	return branch;
}

function setEdit(edits: Record<number, number>, id: number, value: number): void {
	// Tool inputs state exact MW deltas. The 0.25 MW slider deadband belongs to
	// pointer interaction; applying it here can silently erase a valid small
	// update or an accepted capacity proposal.
	if (value === 0) delete edits[id];
	else edits[id] = value;
}

type CaseEditInput = Pick<PreviewCaseUpdateInput, 'mode' | 'demand' | 'ratings'>;

function proposedEdits(
	ctrl: Controller,
	c: SolvableCase,
	input: CaseEditInput
): { demand: Record<number, number>; ratings: Record<number, number> } {
	const demand = { ...c.deltas };
	const ratings = { ...c.ratings };
	for (const edit of input.demand) {
		const bus = resolveBus(c, edit.busId);
		const next = input.mode === 'increment' ? (demand[bus.id] ?? 0) + edit.deltaMw : edit.deltaMw;
		validateDemand(ctrl, c, edit.busId, next);
		setEdit(demand, bus.id, next);
	}
	for (const edit of input.ratings) {
		const branch = resolveBranch(c, edit.branchId);
		const next =
			input.mode === 'increment' ? (ratings[branch.id] ?? 0) + edit.deltaMw : edit.deltaMw;
		validateRating(c, edit.branchId, next);
		setEdit(ratings, branch.id, next);
	}
	return { demand, ratings };
}

async function preview(
	ctrl: Controller,
	input: PreviewCaseUpdateInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = activeCase(ctrl, input.caseId);
	requireRevision(c, input.expectedRevision);
	const revision = caseRevision(c);
	if (c.solving) {
		throw new TellegenToolError('CASE_SOLVING', 'wait for the active exact solve to finish');
	}
	signal.throwIfAborted();
	const { demand, ratings } = proposedEdits(ctrl, c, input);
	const studyInputJson = await ctrl.ensureStudyInputJson(c);
	if (!studyInputJson)
		throw new TellegenToolError('CASE_DATA_UNAVAILABLE', 'the PowerIO module is unavailable');
	// A tool preview must not move the shared Study's committed point. Build an
	// isolated workspace from the retained module, align that clone with the
	// visible operating point, then discard it after the linearization.
	const study = await createStudy(studyInputJson, c.formulation, {
		isolated: true,
		signal
	});
	let predicted: Awaited<ReturnType<BrowserStudy['preview']>>;
	try {
		await study.commit(c.id, c.deltas, c.ratings, null, signal);
		signal.throwIfAborted();
		predicted = await study.preview(demand, ratings);
	} finally {
		study.free();
	}
	signal.throwIfAborted();
	requireRevision(activeCase(ctrl, input.caseId), revision);
	const priceChanges = predicted.prices
		.filter((entry) => Number.isFinite(entry.value))
		.sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
		.slice(0, input.limit)
		.map((entry) => {
			const bus = resolveBus(c, String(entry.bus));
			return {
				bus_id: elementId(bus, 'bus'),
				legacy_id: bus.id,
				price_delta: entry.value
			};
		});
	const objectiveDelta = finite(predicted.objectiveDelta);
	return {
		case_id: c.id,
		revision,
		formulation: c.formulation,
		committed: false,
		edits: {
			demand_count: input.demand.length,
			rating_count: input.ratings.length
		},
		prediction: {
			objective: finite(
				objectiveDelta === null || !c.solution ? null : c.solution.objective + objectiveDelta
			),
			objective_delta: objectiveDelta,
			price_changes: priceChanges
		}
	};
}

interface PreparedCaseSolve {
	study: BrowserStudy;
	studyInputJson: string;
	formulation: Formulation;
	baseSolution: Solution;
	solution: Solution;
	iterations: SolveIteration[];
	sensitivity: SensitivityColumn | null;
	solveMs: number;
}

async function prepareExactSolve(
	ctrl: Controller,
	c: SolvableCase,
	demand: Record<number, number>,
	ratings: Record<number, number>,
	formulation: Formulation,
	signal: AbortSignal
): Promise<PreparedCaseSolve> {
	const studyInputJson = await ctrl.ensureStudyInputJson(c);
	if (!studyInputJson) {
		throw new TellegenToolError('CASE_DATA_UNAVAILABLE', 'the PowerIO module is unavailable');
	}
	const started = performance.now();
	let study: BrowserStudy | null = null;
	try {
		study = await createStudy(studyInputJson, formulation, {
			isolated: true,
			signal
		});
		const baseSolution = await study.currentSolution();
		signal.throwIfAborted();
		const result = await study.commit(c.id, demand, ratings, ctrl.selectionTarget, signal);
		signal.throwIfAborted();
		return {
			study,
			studyInputJson,
			formulation,
			baseSolution,
			solution: result.solution,
			iterations: result.iterations,
			sensitivity: result.sensitivity,
			solveMs: Math.round(performance.now() - started)
		};
	} catch (error) {
		study?.free();
		throw error;
	}
}

function publishExactSolve(
	ctrl: Controller,
	c: SolvableCase,
	demand: Record<number, number>,
	ratings: Record<number, number>,
	prepared: PreparedCaseSolve
): void {
	if (ctrl.isBackendCase(c)) {
		c.closeStream?.();
		c.closeStream = null;
	}
	c.solveSeq += 1;
	ctrl.disposeStudy(c);
	c.formulation = prepared.formulation;
	c.deltas = demand;
	c.ratings = ratings;
	ctrl.bumpRevision(c);
	c.baseSolution = prepared.baseSolution;
	c.solution = prepared.solution;
	c.iterations = prepared.iterations;
	c.sensitivity = null;
	c.solving = false;
	c.solveBackend = 'clarabel-wasm';
	c.solveFallbackReason = null;
	c.solveMs = prepared.solveMs;
	c.predictedObjective = null;
	ctrl.app.previewPrices = null;
	ctrl.previewObjective = null;
	ctrl.ratingSlope = null;
	ctrl.app.error = null;
	ctrl.caseStudies.set(c, {
		study: prepared.study,
		studyInputJson: prepared.studyInputJson,
		formulation: prepared.formulation,
		baseSolution: prepared.baseSolution
	});
	ctrl.studyUnavailable.delete(c);
	if (prepared.sensitivity && ctrl.selectionTarget) {
		ctrl.acceptSensitivity(c, prepared.sensitivity, ctrl.selectionTarget);
	}
}

async function update(
	ctrl: Controller,
	input: UpdateCaseInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = activeCase(ctrl, input.caseId);
	requireRevision(c, input.expectedRevision);
	if (c.solving)
		throw new TellegenToolError('CASE_SOLVING', 'wait for the active exact solve to finish');
	signal.throwIfAborted();
	const { demand, ratings } = proposedEdits(ctrl, c, input);
	const before = caseSnapshot(c);
	let formulation: Formulation | undefined;
	if (input.formulation !== undefined) {
		const option = FORMULATIONS.find(
			(entry: FormulationOption) => entry.id === input.formulation
		) as FormulationOption | undefined;
		if (!option || option.disabled) {
			throw new TellegenToolError(
				'FORMULATION_UNAVAILABLE',
				`formulation ${clip(input.formulation)} is unavailable`
			);
		}
		formulation = option.id;
	}

	const targetFormulation = formulation ?? c.formulation;
	const prepared = await prepareExactSolve(ctrl, c, demand, ratings, targetFormulation, signal);
	try {
		signal.throwIfAborted();
		requireRevision(activeCase(ctrl, input.caseId), input.expectedRevision);
		if (c.solving) {
			throw new TellegenToolError('CASE_SOLVING', 'the active case began another solve');
		}
		publishExactSolve(ctrl, c, demand, ratings, prepared);
	} catch (error) {
		prepared.study.free();
		throw error;
	}
	return {
		case_id: c.id,
		revision: caseRevision(c),
		formulation: c.formulation,
		demand_edit_count: Object.keys(c.deltas).length,
		rating_edit_count: Object.keys(c.ratings).length,
		solution: solutionPayload(c),
		before,
		after: caseSnapshot(c)
	};
}

async function reset(
	ctrl: Controller,
	input: ResetCaseInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = activeCase(ctrl, input.caseId);
	requireRevision(c, input.expectedRevision);
	if (c.solving)
		throw new TellegenToolError('CASE_SOLVING', 'wait for the active exact solve to finish');
	signal.throwIfAborted();
	const before = caseSnapshot(c);
	const prepared = await prepareExactSolve(ctrl, c, {}, {}, c.formulation, signal);
	try {
		signal.throwIfAborted();
		requireRevision(activeCase(ctrl, input.caseId), input.expectedRevision);
		if (c.solving) {
			throw new TellegenToolError('CASE_SOLVING', 'the active case began another solve');
		}
		publishExactSolve(ctrl, c, {}, {}, prepared);
	} catch (error) {
		prepared.study.free();
		throw error;
	}
	return {
		case_id: c.id,
		revision: caseRevision(c),
		formulation: c.formulation,
		demand_edit_count: 0,
		rating_edit_count: 0,
		solution: solutionPayload(c),
		before,
		after: caseSnapshot(c)
	};
}

/** Round bounded tool payload numbers without changing solver state. */
function round4(value: number | null | undefined): number | null {
	const v = finite(value ?? null);
	return v === null ? null : Math.round(v * 1e4) / 1e4;
}

function fallbackBranchRow(c: SolvableCase, branch: NetworkBranch): number | null {
	const row = branch.id - 1;
	return Number.isSafeInteger(row) && row >= 0 && c.network?.branches[row] === branch ? row : null;
}

function canonicalBranchIdentity(
	c: SolvableCase,
	branch: NetworkBranch,
	requestedId: string
): string {
	if (branch.uid) return branch.uid;
	const row = fallbackBranchRow(c, branch);
	if (row === null) {
		throw new TellegenToolError(
			'PLANNING_UNAVAILABLE',
			`branch ${clip(requestedId)} is not aligned with a canonical PowerIO row`
		);
	}
	return `branches:${row}`;
}

/** `Phi = Σ w · price` over the objective's buses at the case's current
 * solution, or null when a bus or its price cannot be resolved. */
function phiAt(c: SolvableCase, weights: CapacityPlanBusWeightJson[]): number | null {
	if (!c.solution) return null;
	const lookup = caseLookup(c);
	let phi = 0;
	for (const term of weights) {
		let bus: NetworkBus;
		try {
			bus = resolveBus(c, String(term.bus));
		} catch {
			return null;
		}
		const value = lookup.prices.get(bus.id);
		if (value === undefined) return null;
		phi += term.weight * value;
	}
	return phi;
}

async function proposeCapacityPlan(
	ctrl: Controller,
	activities: PlanningActivityStore,
	input: ProposeCapacityPlanInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const startedAt = Date.now();
	const c = activeCase(ctrl, input.caseId);
	requireRevision(c, input.expectedRevision);
	if (c.formulation !== 'dcopf') {
		throw new TellegenToolError(
			'PLANNING_UNAVAILABLE',
			'planning differentiates the DC OPF formulation only; switch the case to dcopf first'
		);
	}
	if (c.solving) {
		throw new TellegenToolError('CASE_SOLVING', 'wait for the active exact solve to finish');
	}
	signal.throwIfAborted();
	const resolvedWeightBuses = new Set<number>();
	const weights = input.objective.weights.map((entry) => {
		const bus = resolveBus(c, entry.busId);
		if (resolvedWeightBuses.has(bus.id)) {
			throw new TellegenToolError(
				'INVALID_INPUT',
				`objective weights resolve to the same bus ${clip(entry.busId)}`
			);
		}
		resolvedWeightBuses.add(bus.id);
		if (isDisplayOnlyElement(bus)) {
			throw new TellegenToolError('EDIT_NOT_SUPPORTED', `bus ${clip(entry.busId)} is display-only`);
		}
		return { bus: bus.id, weight: entry.weight };
	});
	const candidateBranches = new Map<string, NetworkBranch>();
	const candidates = input.candidates.map((id) => {
		const branch = resolveBranch(c, id);
		if (isDisplayOnlyElement(branch) || branch.rate_mw <= 0) {
			throw new TellegenToolError(
				'EDIT_NOT_SUPPORTED',
				`branch ${clip(id)} has no editable rating`
			);
		}
		const availableIncrease = ratingDeltaBounds(branch).max - (c.ratings[branch.id] ?? 0);
		if (input.maxIncreasePerBranchMw > availableIncrease) {
			throw new TellegenToolError(
				'INVALID_INPUT',
				`max_increase_per_branch_mw must be at most ${availableIncrease} MW for branch ${clip(id)}`
			);
		}
		const identity = canonicalBranchIdentity(c, branch, id);
		if (candidateBranches.has(identity)) {
			throw new TellegenToolError(
				'INVALID_INPUT',
				`candidates resolve to the same branch ${clip(id)}`
			);
		}
		candidateBranches.set(identity, branch);
		return identity;
	});
	const spec: CapacityPlanSpecJson = {
		objective: { kind: 'weighted_lmp', weights },
		candidates,
		max_increase_per_branch_mw: input.maxIncreasePerBranchMw,
		budget_mw: input.budgetMw,
		increment_mw: input.incrementMw,
		max_changed_lines: input.maxChangedLines,
		exact_solve_budget: input.exactSolveBudget
	};
	const revision = caseRevision(c);
	const studyInputJson = await ctrl.ensureStudyInputJson(c);
	if (!studyInputJson)
		throw new TellegenToolError('CASE_DATA_UNAVAILABLE', 'the PowerIO module is unavailable');
	const source_digest = await sourceDigest(ctrl, c, studyInputJson);
	signal.throwIfAborted();
	const cached = ctrl.caseStudies.get(c);
	if (!cached || cached.studyInputJson !== studyInputJson || cached.formulation !== c.formulation) {
		throw new TellegenToolError(
			'PLANNING_UNAVAILABLE',
			'the current exact solve is not ready; wait for it to finish'
		);
	}
	signal.throwIfAborted();
	// BrowserStudy.plan materializes and solves on one disposable worker. This
	// keeps cancellation away from the retained interactive Study and avoids a
	// second clone/base solve here.
	const outcome = await cached.study.plan(spec, signal);
	signal.throwIfAborted();
	const publicProposal = outcome.proposal.map((change) => {
		const branch = candidateBranches.get(change.branch);
		if (!branch) {
			throw new TellegenToolError(
				'TOOL_FAILED',
				`the engine proposed unknown branch ${clip(String(change.branch))}`
			);
		}
		return {
			branchId: elementId(branch, 'branch'),
			legacyId: branch.id,
			deltaMw: change.delta_mw
		};
	});

	const activityId = randomId('plan');
	const proposalId = randomId('proposal');
	const accepted = outcome.iterations.filter((it) => it.accepted);
	const lastAccepted = accepted[accepted.length - 1] ?? null;
	const last = outcome.iterations[outcome.iterations.length - 1] ?? null;
	const predictedPhiDelta = accepted.reduce((sum, it) => sum + it.predicted_phi_delta, 0);
	// The search ran unqueued beside the interface; a concurrent edit during it
	// makes the outcome an audit record, never a stageable proposal.
	const moved = ctrl.activeSolvable !== c || caseRevision(c) !== revision;
	const staged = !moved && outcome.proposal.length > 0;
	const activity = {
		id: activityId,
		proposalId: staged ? proposalId : null,
		caseId: c.id,
		sessionId: SESSION_ID,
		revision,
		formulation: c.formulation,
		sourceDigest: source_digest,
		spec,
		outcome,
		displayProposal: publicProposal.map(({ branchId, deltaMw }) => ({ branchId, deltaMw })),
		decision:
			moved && outcome.proposal.length > 0
				? ('expired' as const)
				: staged
					? ('pending' as const)
					: ('no_change' as const),
		startedAt,
		durationMs: Date.now() - startedAt
	};
	if (staged) {
		activities.stage({
			proposalId,
			activityId,
			caseId: c.id,
			sessionId: SESSION_ID,
			revision,
			changes: publicProposal,
			createdAt: Date.now()
		});
	}
	activities.append(activity);
	return {
		activity_id: activityId,
		proposal_id: staged ? proposalId : null,
		session_id: SESSION_ID,
		source_digest,
		revision,
		baseline_phi: round4(outcome.baseline_phi),
		final_phi: round4(outcome.final_phi),
		baseline: {
			phi: round4(outcome.baseline.phi),
			declared_objective: round4(outcome.baseline.declared_objective),
			exact_solve: outcome.baseline.exact_solve
		},
		exact_verified_result: {
			phi: round4(outcome.exact_verified_result.phi),
			declared_objective: round4(outcome.exact_verified_result.declared_objective),
			exact_solve: outcome.exact_verified_result.exact_solve
		},
		exact_phi_delta: round4(outcome.final_phi - outcome.baseline_phi),
		predicted_phi_delta: round4(predictedPhiDelta),
		first_order_error: round4(lastAccepted?.first_order_error),
		spent_budget_mw: round4(outcome.spent_budget_mw),
		exact_solves: outcome.exact_solves,
		proposal: publicProposal.slice(0, 12).map((change) => ({
			branch_id: change.branchId,
			delta_mw: round4(change.deltaMw)
		})),
		iterations: { total: outcome.iterations.length, accepted: accepted.length },
		stop_reason: last ? clip(last.reason, 140) : 'no iterations ran'
	};
}

async function applyCapacityPlan(
	ctrl: Controller,
	activities: PlanningActivityStore,
	input: ApplyCapacityPlanInput,
	signal: AbortSignal
): Promise<ToolPayload> {
	const c = activeCase(ctrl, input.caseId);
	requireRevision(c, input.expectedRevision);
	if (c.solving)
		throw new TellegenToolError('CASE_SOLVING', 'wait for the active exact solve to finish');
	signal.throwIfAborted();
	const staged = activities.proposal;
	if (!staged || staged.proposalId !== input.proposalId) {
		throw new TellegenToolError(
			'UNKNOWN_PROPOSAL',
			`proposal ${clip(input.proposalId)} is not staged; run propose_capacity_plan first`
		);
	}
	if (
		staged.caseId !== c.id ||
		staged.sessionId !== SESSION_ID ||
		staged.revision !== caseRevision(c)
	) {
		throw new TellegenToolError(
			'STALE_PROPOSAL',
			'the session or case changed after this proposal was computed; propose a new plan'
		);
	}
	// Validate the composed edits through the exact same path as update_case
	// before consuming anything, so a refusal burns neither proposal nor approval.
	const { demand, ratings } = proposedEdits(ctrl, c, {
		mode: 'increment',
		demand: [],
		ratings: staged.changes.map((change) => ({
			branchId: change.branchId,
			deltaMw: change.deltaMw
		}))
	});
	signal.throwIfAborted();
	if (!activities.isApproved(staged)) {
		throw new TellegenToolError(
			'APPROVAL_REQUIRED',
			'a human must press Approve on this proposal before apply_capacity_plan can run'
		);
	}
	const source_digest =
		activities.entries.find((entry) => entry.id === staged.activityId)?.sourceDigest ??
		(await sourceDigest(ctrl, c));
	signal.throwIfAborted();
	const before = caseSnapshot(c);
	const prepared = await prepareExactSolve(ctrl, c, demand, ratings, c.formulation, signal);
	try {
		signal.throwIfAborted();
		requireRevision(activeCase(ctrl, input.caseId), input.expectedRevision);
		if (activities.proposal?.proposalId !== staged.proposalId || !activities.isApproved(staged)) {
			throw new TellegenToolError(
				'STALE_PROPOSAL',
				'the proposal or its approval changed while the exact solve was running'
			);
		}
		if (c.solving) {
			throw new TellegenToolError('CASE_SOLVING', 'the active case began another solve');
		}
		publishExactSolve(ctrl, c, demand, ratings, prepared);
	} catch (error) {
		prepared.study.free();
		throw error;
	}
	activities.commitApplied(staged);
	const weights = activities.entries.find((entry) => entry.id === staged.activityId)?.spec.objective
		.weights;
	const exactPhi = weights ? phiAt(c, weights) : null;
	return {
		case_id: c.id,
		session_id: SESSION_ID,
		source_digest,
		revision: caseRevision(c),
		formulation: c.formulation,
		proposal_id: staged.proposalId,
		activity_id: staged.activityId,
		applied: staged.changes.slice(0, 12).map((change) => ({
			branch_id: change.branchId,
			delta_mw: round4(change.deltaMw)
		})),
		exact_phi: round4(exactPhi),
		solution: solutionPayload(c),
		before,
		after: caseSnapshot(c)
	};
}

/** Create the app adapter. Mutations are serialized and re-check revisions in queue order. */
export function createTellegenWebMcpAdapter(
	ctrl: Controller,
	activities?: PlanningActivityStore
): TellegenWebMcpAdapter {
	let mutationTail: Promise<void> = Promise.resolve();
	const enqueue = <T>(operation: () => Promise<T>): Promise<T> => {
		const next = mutationTail.then(operation, operation);
		mutationTail = next.then(
			() => undefined,
			() => undefined
		);
		return next;
	};
	let planning: TellegenPlanningAdapter | undefined;
	if (activities) {
		planning = {
			proposeCapacityPlan: (input, signal) => proposeCapacityPlan(ctrl, activities, input, signal),
			applyCapacityPlan: (input, signal) =>
				enqueue(() => applyCapacityPlan(ctrl, activities, input, signal)),
			planningAvailable() {
				const c = ctrl.activeSolvable;
				if (!c || !c.network || c.formulation !== 'dcopf' || c.solving) return false;
				const cached = ctrl.caseStudies.get(c);
				return (
					!!cached &&
					cached.formulation === c.formulation &&
					cached.studyInputJson === c.studyInputJson &&
					c.network.branches.every(
						(branch) =>
							branch.editable === false || !!branch.uid || fallbackBranchRow(c, branch) !== null
					)
				);
			},
			proposalAvailable() {
				const c = ctrl.activeSolvable;
				const staged = activities.proposal;
				return (
					!!c &&
					!!staged &&
					staged.caseId === c.id &&
					staged.sessionId === SESSION_ID &&
					staged.revision === caseRevision(c)
				);
			},
			onAvailabilityChange: (listener) => activities.subscribe(listener)
		};
	}
	return {
		...(planning ? { planning } : {}),
		inspectCase(signal) {
			signal.throwIfAborted();
			return inspect(ctrl, activities, signal);
		},
		queryNetwork(input, signal) {
			signal.throwIfAborted();
			return query(ctrl, input);
		},
		analyzeSensitivity: (input, signal) => sensitivity(ctrl, input, signal),
		focusNetwork: (input, signal) => enqueue(() => focus(ctrl, input, signal)),
		previewCaseUpdate: (input, signal) => preview(ctrl, input, signal),
		updateCase: (input, signal) => enqueue(() => update(ctrl, input, signal)),
		resetCase: (input, signal) => enqueue(() => reset(ctrl, input, signal))
	};
}
