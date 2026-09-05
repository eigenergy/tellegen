import { studySchema, type StudyOperation } from '@tellegen/engine';
import {
	TellegenToolError,
	type StudyToolName,
	type TellegenStudyAdapter,
	type ToolPayload
} from '@tellegen/webmcp';
import type { StudyWorkspace, GoalDraft } from './workspace.svelte.js';

type Schema = Record<string, any>;
const definitions = studySchema.$defs as Record<string, Schema>;
function withDefinitions(root: Schema): Schema {
	const needed: Record<string, Schema> = {};
	function visit(value: any) {
		if (!value || typeof value !== 'object') return;
		if (typeof value.$ref === 'string') {
			const name = value.$ref.replace('#/$defs/', '');
			if (!needed[name]) {
				needed[name] = definitions[name];
				visit(needed[name]);
			}
		}
		for (const child of Object.values(value)) visit(child);
	}
	visit(root);
	return { ...root, $defs: needed };
}
const string = { type: 'string', minLength: 1, maxLength: 4096 };
const revision = { type: 'integer', minimum: 0 };
const object = (properties: Schema, required = Object.keys(properties)) => ({
	type: 'object',
	properties,
	required,
	additionalProperties: false
});
const kinds: Partial<Record<StudyToolName, string>> = {
	revise_study_goal: 'revise_goal',
	branch_study: 'branch',
	compare_study_states: 'compare',
	propose_study: 'propose',
	edit_demand: 'edit_demand',
	restore_base_case: 'restore_base',
	record_study_evidence: 'record_evidence'
};
const operationVariants = (definitions.StudyOperation.oneOf ??
	definitions.StudyOperation.anyOf) as Schema[];
function inputSchema(name: StudyToolName): Schema {
	if (name === 'create_study') {
		const create = structuredClone(definitions.CreateStudy);
		delete create.properties.input;
		delete create.properties.base_input;
		delete create.properties.id;
		create.required = create.required.filter((key: string) => key !== 'input' && key !== 'id');
		return withDefinitions(
			object({ case_id: string, expected_case_revision: string, goal: create })
		);
	}
	if (name === 'inspect_study')
		return object(
			{
				study_id: { ...string, description: 'Open this saved Study if it is not currently open.' },
				section: { enum: ['summary', 'goal', 'states', 'experiment', 'evidence'] },
				record_id: string,
				offset: revision,
				expected_revision: revision
			},
			[]
		);
	const operation = operationVariants.find((v) => v.properties.kind.const === kinds[name]);
	if (!operation) throw new Error(`Missing Rust contract for ${name}`);
	return withDefinitions(object({ study_id: string, expected_revision: revision, operation }));
}
function text(input: Record<string, unknown>, field: string): string {
	const value = input[field];
	if (typeof value !== 'string' || !value || value.length > 4096)
		throw new TellegenToolError('INVALID_INPUT', `${field} must be a nonempty string`);
	return value;
}
function asPayload(value: unknown): ToolPayload {
	return JSON.parse(JSON.stringify(value)) as ToolPayload;
}

export function createStudyAdapter(workspace: StudyWorkspace): TellegenStudyAdapter {
	return {
		inputSchema,
		async execute(name, input, signal) {
			signal.throwIfAborted();
			if (name === 'create_study') {
				if (!input.goal || typeof input.goal !== 'object')
					throw new Error('A structured goal is required');
				await workspace.create(
					input.goal as GoalDraft,
					text(input, 'case_id'),
					text(input, 'expected_case_revision'),
					signal
				);
				return asPayload(workspace.summary());
			}
			if (name === 'inspect_study') {
				if (input.study_id && input.study_id !== workspace.document?.id)
					await workspace.open(text(input, 'study_id'));
				const d = workspace.document;
				if (input.expected_revision !== undefined && input.expected_revision !== d?.revision)
					throw new Error('Study revision changed; restart the inspection');
				const section = input.section ?? 'summary';
				if (section === 'summary') return asPayload(workspace.summary());
				if (!d || !workspace.bundle) throw new Error('No Study is open');
				let record: unknown;
				if (section === 'goal')
					record =
						d.goals[typeof input.record_id === 'string' ? input.record_id : (d.active_goal ?? '')];
				else if (section === 'states')
					record = Object.entries(d.states).map(([id, state]) => ({ id, ...state }));
				else if (section === 'experiment') record = d.experiments[text(input, 'record_id')];
				else if (section === 'evidence') {
					const artifact = workspace.bundle.artifacts[text(input, 'record_id')];
					if (artifact?.kind !== 'evidence') throw new Error('Requested artifact is not evidence');
					record = artifact.text;
				} else throw new Error('Unknown Study inspection section');
				if (record === undefined) throw new Error('Requested Study record is unavailable');
				const offset = input.offset ?? 0;
				if (!Number.isSafeInteger(offset) || (offset as number) < 0)
					throw new Error('offset must be a nonnegative integer');
				const serialized = JSON.stringify(record),
					start = offset as number,
					fragment = serialized.slice(start, start + 600);
				return asPayload({
					id: d.id,
					revision: d.revision,
					section,
					encoding: 'json',
					offset,
					next_offset: start + fragment.length < serialized.length ? start + fragment.length : null,
					fragment
				});
			}
			const studyId = text(input, 'study_id');
			if (!Number.isSafeInteger(input.expected_revision) || (input.expected_revision as number) < 0)
				throw new Error('expected_revision must be a nonnegative integer');
			const operation = input.operation as StudyOperation;
			if (!operation || operation.kind !== kinds[name])
				throw new Error('Operation kind does not match the tool');
			// Only the declared operation reaches the controller; apply is a browser user action.
			const result = await workspace.execute(
				studyId,
				input.expected_revision as number,
				operation,
				signal
			);
			const d = workspace.document!;
			return asPayload({
				id: d.id,
				revision: d.revision,
				experiment: result.experiment,
				inspected_state: d.inspected_state,
				recommended_state: d.recommended_state,
				applied_state: d.applied_state,
				...(result.comparison
					? {
							comparison: {
								goal: result.comparison.goal,
								left_value: result.comparison.left_value,
								right_value: result.comparison.right_value,
								improvement: result.comparison.improvement
							}
						}
					: {})
			});
		}
	};
}
