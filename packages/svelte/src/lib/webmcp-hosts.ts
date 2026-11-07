import type { CapacityPlanOutcomeJson, CapacityPlanSpecJson } from '@tellegen/engine';

/** Optional host-owned persistence and approval boundaries. The adapter never creates a store,
 * writes browser storage, or approves a proposal on the host's behalf. */
export interface CapacityStudyBinding {
	studyId: string;
	revision: number;
	proposal: string;
	goal: string;
	base: string;
	state: string;
}

export interface CaseEvidenceContext {
	studyId: string;
	revision: number;
	state: string;
	goal: string | null;
	caseId: string;
	caseRevision: string;
}

export interface CapacityPlanActivity {
	id: string;
	proposalId: string | null;
	caseId: string;
	sessionId: string;
	sourceDigest: string;
	revision: string;
	formulation: string;
	spec: CapacityPlanSpecJson;
	outcome: CapacityPlanOutcomeJson;
	displayProposal: { branchId: string; deltaMw: number }[];
	decision: 'pending' | 'applied' | 'rejected' | 'expired' | 'no_change';
	startedAt: number;
	durationMs: number;
}

export interface StagedCapacityProposal {
	study?: CapacityStudyBinding;
	proposalId: string;
	activityId: string;
	caseId: string;
	sessionId: string;
	revision: string;
	changes: { branchId: string; legacyId: number; deltaMw: number }[];
	createdAt: number;
}

export interface WebMcpPlanningHost {
	readonly entries: readonly CapacityPlanActivity[];
	readonly proposal: StagedCapacityProposal | null;
	isApproved(proposal: StagedCapacityProposal): boolean;
	stage(proposal: StagedCapacityProposal): void;
	append(activity: CapacityPlanActivity): void;
	commitApplied(proposal: StagedCapacityProposal): void;
	subscribe(listener: () => void): () => void;
}

export interface WebMcpStudyHost {
	readonly document: { id: string; revision: number } | null;
	captureCaseEvidence(): CaseEvidenceContext | null;
	recordCaseEvidence(
		context: CaseEvidenceContext,
		tool: string,
		input: unknown,
		result: unknown,
		signal: AbortSignal
	): Promise<void>;
	planCapacity(
		spec: CapacityPlanSpecJson,
		caseId: string,
		revision: string,
		signal: AbortSignal,
		elements: Readonly<Record<string, number>>
	): Promise<{ outcome: CapacityPlanOutcomeJson; binding: CapacityStudyBinding }>;
	applyCapacity(
		binding: CapacityStudyBinding,
		publish: () => boolean,
		signal: AbortSignal
	): Promise<unknown>;
}
