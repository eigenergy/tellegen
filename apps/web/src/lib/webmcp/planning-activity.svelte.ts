import type { CapacityPlanOutcomeJson, CapacityPlanSpecJson } from '@tellegen/svelte';
import type { CapacityStudyBinding } from '../studies/capacity-compat.js';
import type { StudyWorkspace } from '../studies/workspace.svelte.js';

export type ProposalDecision = 'pending' | 'applied' | 'rejected' | 'expired' | 'no_change';

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
	/** Proposal identities in the public WebMCP namespace. The raw outcome is
	 * retained above with canonical PowerIO identities for traceability. */
	displayProposal: CapacityPlanDisplayChange[];
	decision: ProposalDecision;
	startedAt: number;
	durationMs: number;
}

export interface CapacityPlanDisplayChange {
	branchId: string;
	deltaMw: number;
}

export interface StagedProposalChange {
	branchId: string;
	legacyId: number;
	deltaMw: number;
}

export interface StagedCapacityProposal {
	study?: CapacityStudyBinding;
	proposalId: string;
	activityId: string;
	caseId: string;
	sessionId: string;
	revision: string;
	changes: StagedProposalChange[];
	createdAt: number;
}

interface ProposalApproval {
	proposalId: string;
	caseId: string;
	sessionId: string;
	revision: string;
}

const MAX_ACTIVITIES = 50;

/** Bounded browser state kept in memory for visible planning activity. Loading
 * a case creates a fresh runtime session. */
export class PlanningActivityStore {
	entries = $state.raw<CapacityPlanActivity[]>([]);
	proposal = $state.raw<StagedCapacityProposal | null>(null);
	approval = $state.raw<ProposalApproval | null>(null);

	#listeners = new Set<() => void>();
	constructor(readonly getWorkspace?: () => StudyWorkspace) {}
	get workspace() {
		return this.getWorkspace?.();
	}

	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	#notify(): void {
		for (const listener of [...this.#listeners]) {
			try {
				listener();
			} catch {
				// One observer must not break the store or other observers.
			}
		}
	}

	append(activity: CapacityPlanActivity): void {
		this.entries = [activity, ...this.entries].slice(0, MAX_ACTIVITIES);
		this.#notify();
	}

	stage(proposal: StagedCapacityProposal): void {
		if (this.proposal) this.#decide(this.proposal.activityId, 'expired');
		this.proposal = proposal;
		this.approval = null;
		this.#notify();
	}

	#decide(activityId: string, decision: ProposalDecision): void {
		this.entries = this.entries.map((entry) =>
			entry.id === activityId ? { ...entry, decision } : entry
		);
	}

	approve(): void {
		const staged = this.proposal;
		if (!staged) return;
		if (staged.study) this.workspace!.approveCapacity(staged.study);
		this.approval = {
			proposalId: staged.proposalId,
			caseId: staged.caseId,
			sessionId: staged.sessionId,
			revision: staged.revision
		};
		this.#notify();
	}

	isApproved(staged: StagedCapacityProposal): boolean {
		const approval = this.approval;
		return (
			(!staged.study || !!this.workspace?.capacityApproved(staged.study)) &&
			approval?.proposalId === staged.proposalId &&
			approval.caseId === staged.caseId &&
			approval.sessionId === staged.sessionId &&
			approval.revision === staged.revision
		);
	}

	rejectStaged(): void {
		const staged = this.proposal;
		if (!staged) return;
		this.proposal = null;
		this.approval = null;
		this.#decide(staged.activityId, 'rejected');
		this.#notify();
	}

	/** Consume proposal and approval only after the exact solve has succeeded. */
	commitApplied(staged: StagedCapacityProposal): void {
		if (
			this.proposal?.proposalId !== staged.proposalId ||
			this.approval?.proposalId !== staged.proposalId
		)
			return;
		this.proposal = null;
		this.approval = null;
		this.#decide(staged.activityId, 'applied');
		this.#notify();
	}

	expireIfStale(current: { caseId: string; sessionId: string; revision: string } | null): void {
		const staged = this.proposal;
		if (
			staged &&
			(!current ||
				staged.caseId !== current.caseId ||
				staged.sessionId !== current.sessionId ||
				staged.revision !== current.revision ||
				(staged.study && !this.workspace?.capacityCurrent(staged.study)))
		) {
			this.proposal = null;
			this.approval = null;
			this.#decide(staged.activityId, 'expired');
		}
		this.#notify();
	}
}
