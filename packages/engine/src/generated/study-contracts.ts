// Generated from Rust by study_contract and generate-study-contracts.mjs.
// Source SHA256: 098d1f7b01cb10de053377613ce83a471b7260b81c212bf30ea55ec86e3263df

export type ArtifactKind = "powerio_ir" | "evidence";

export type Bound = "Min" | "Max";

export type BranchFlow = { "branch": number; "loading": number; "pf": number; "pt"?: number | null; "qf"?: number | null; "qt"?: number | null; "uid"?: string | null; };

export type BranchScalar = { "branch": number; "value": number; };

export type BusInjection = { "bus": number; "p": number; "q": number; "uid"?: string | null; };

export type BusScalar = { "bus": number; "uid"?: string | null; "value": number; };

export type ColMeta = { "element": ElementId; "index": number; "parameter": Parameter; };

export type Comparison = { "goal": string; "improvement": number; "left": string; "left_value": number; "left_view": SolveResponse; "right": string; "right_value": number; "right_view": SolveResponse; };

export type CostTerm = "Quadratic" | "Linear";

export type CreateStudy = { "decisions": DecisionSpace; "formulation": "dcopf" | "acpf" | "socwr"; "id": string; "input": string; "interpretation": string; "objective": StudyObjective; "request": string; "success_value"?: number | null; "title": string; };

export type DecisionKind = "retain" | "reject" | "recommend" | "apply";

export type DecisionRecord = { "choice": DecisionKind; "evidence": Array<string>; "experiment": string; "rationale": string; "state"?: string | null; };

export type DecisionSpace = { "demand"?: (DemandConstraint) | (null); "max_changed_elements": number; "total_budget": number; "variables": Array<DecisionVariable>; };

export type DecisionVariable = { "budget_weight": number; "element": ElementKey; "id": string; "increment": number; "intervention": Intervention; "lower": number; "upper": number; };

export type DemandConstraint = ({ "increase_mw": number; "kind": "placement"; }) | ({ "kind": "redistribution"; });

export type ElementId = ({ "Bus": number; }) | ({ "Branch": number; }) | ({ "Generator": number; });

export type ElementKey = (number) | (string);

export type End = "From" | "To";

export type ExperimentKind = "inspection" | "sensitivity" | "planning" | "counterfactual" | "challenge" | "historical_import";

export type ExperimentRecord = { "assessed_recommendation"?: string | null; "evidence": Array<string>; "goal"?: string | null; "kind": ExperimentKind; "rationale": string; "result_states": Array<string>; "solve_count": number; "start_state"?: string | null; "termination": string; "trials": Array<TrialRecord>; };

export type ExperimentSummary = { "goal"?: string | null; "id": string; "kind": ExperimentKind; "rationale": string; "result_states": Array<string>; "solve_count": number; "start_state"?: string | null; "termination": string; "trial_count": number; };

export type GB = "Conductance" | "Susceptance";

export type GenDispatch = { "bus"?: number | null; "gen": number; "pg": number; "qg"?: number | null; };

export type GoalRevision = { "anchor_state": string; "decisions": DecisionSpace; "interpretation": string; "objective": StudyObjective; "parent"?: string | null; "request": string; "success_value"?: number | null; };

export type Intervention = "branch_rating" | "active_demand";

export type Iterations = (Array<SolveIteration>) | ({ "count": number; "residual": number; });

export type ObservableWeight = { "element": ElementKey; "weight": number; };

export type Operand = ({ "Price": Power; }) | ({ "Dispatch": Power; }) | ({ "Flow": { "end": End; "power": Power; }; }) | ({ "Voltage": VoltageKind; });

export type Parameter = ({ "Demand": Power; }) | ({ "Cost": CostTerm; }) | ("LineLimit") | ({ "SeriesAdmittance": GB; }) | ({ "ShuntAdmittance": GB; }) | ({ "VoltageBound": Bound; }) | ({ "GenBound": { "bound": Bound; "power": Power; }; }) | ({ "Transformer": TapKind; }) | ("Switching");

export type Power = "Active" | "Reactive";

export type Problem = ("dcpf") | ("dcopf") | ("acpf") | ("socwr") | ("acopf");

export type RowMeta = { "element": ElementId; "index": number; "operand": Operand; };

export type SearchOptions = { "beam_width": number; "max_iterations": number; "max_solves": number; "min_improvement": number; };

export type SensitivityMatrix = { "cols": Array<ColMeta>; "rows": Array<RowMeta>; "units": string; "values": Array<Array<number>>; };

export type SolveIteration = { "inf_du": number; "inf_pr": number; "iter": number; "objective": number; };

export type SolveResponse = { "dispatch"?: Array<GenDispatch> | null; "flows"?: Array<BranchFlow> | null; "formulation": Problem; "injections"?: Array<BusInjection> | null; "iterations"?: (Iterations) | (null); "lmp"?: Array<BusScalar> | null; "lmp_q"?: Array<BusScalar> | null; "objective"?: number | null; "sensitivities"?: Array<SensitivityMatrix>; "status": SolveStatus; "va"?: Array<BusScalar> | null; "vm"?: Array<BusScalar> | null; "w"?: Array<BusScalar> | null; "wi"?: Array<BranchScalar> | null; "wr"?: Array<BranchScalar> | null; };

export type SolveStatus = ("optimal") | ("feasible");

export type StateNode = { "formulation": Problem; "input": string; "label": string; "parent"?: string | null; "solution": string; "view": string; };

export type StudyArtifact = { "kind": ArtifactKind; "text": string; };

export type StudyBundle = { "artifacts": { [key: string]: StudyArtifact; }; "document": StudyDocument; };

export type StudyDocument = { "active_goal"?: string | null; "applied_state"?: string | null; "decisions": { [key: string]: DecisionRecord; }; "experiment_order": Array<string>; "experiments": { [key: string]: ExperimentRecord; }; "goals": { [key: string]: GoalRevision; }; "id": string; "inspected_state"?: string | null; "recommended_state"?: string | null; "revision": number; "schema": string; "states": { [key: string]: StateNode; }; "title": string; "version": number; };

export type StudyObjective = ({ "kind": "weighted_observable"; "operand": Operand; "weights": Array<ObservableWeight>; }) | ({ "kind": "sum"; "terms": Array<StudyObjective>; }) | ({ "expression": StudyObjective; "factor": number; "kind": "scale"; }) | ({ "expression": StudyObjective; "kind": "squared_target"; "target": number; }) | ({ "decision": string; "kind": "intervention_penalty"; "linear": number; "quadratic": number; });

export type StudyOperation = ({ "kind": "inspect"; "state": string; }) | ({ "kind": "branch"; "rationale": string; "state": string; }) | ({ "goal": GoalRevision; "kind": "revise_goal"; }) | ({ "goal": string; "kind": "compare"; "left": string; "right": string; }) | ({ "goal": string; "kind": "propose"; "options": SearchOptions; "rationale": string; "state": string; }) | ({ "assessed_recommendation"?: string | null; "evidence": unknown; "goal": string; "kind": "record_evidence"; "rationale": string; "sensitivity": boolean; "state": string; }) | ({ "base_state": string; "goal": string; "kind": "apply"; "proposal": string; "state": string; });

export type StudyOperationResult = { "comparison"?: (Comparison) | (null); "experiment"?: string | null; "inspected_view"?: (SolveResponse) | (null); "summary": StudySummary; };

export type StudyRequest = { "expected_revision": number; "operation": StudyOperation; };

export type StudySummary = { "active_goal"?: [string, GoalRevision] | null; "applied_state"?: string | null; "experiment_count": number; "id": string; "inspected_state"?: string | null; "recent_experiments": Array<ExperimentSummary>; "recommended_state"?: string | null; "revision": number; "state_count": number; "title": string; "unavailable_historical_states": number; };

export type TapKind = "Ratio" | "PhaseShift";

export type TrialRecord = { "accepted": boolean; "changes": Array<number>; "evidence": Array<string>; "exact_value"?: number | null; "failure"?: string | null; "predicted_value"?: number | null; "state"?: string | null; };

export type VoltageKind = "Magnitude" | "Angle" | "Squared" | "ProductReal" | "ProductImag";
