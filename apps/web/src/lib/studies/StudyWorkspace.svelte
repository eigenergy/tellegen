<script lang="ts">
	import { onMount } from 'svelte';
	import { getController } from '@tellegen/svelte';
	import type { StudyObjective, DecisionSpace, StudyOperation } from '@tellegen/engine';
	import TellegenWebMcp from '../webmcp/TellegenWebMcp.svelte';
	import { caseRevision } from '../webmcp/tellegen-adapter.js';
	import { StudyWorkspace, type GoalDraft } from './workspace.svelte.js';

	const ctrl = getController();
	const workspace = new StudyWorkspace(ctrl);
	let expanded = $state(false);
	let title = $state('Grid study');
	let request = $state('Lower demand-weighted prices while respecting the intervention budget.');
	let intervention = $state<'capacity' | 'redistribution' | 'placement'>('capacity');
	let objectiveKind = $state<'price' | 'voltage'>('price');
	let formulation = $state<'dcopf' | 'socwr' | 'acpf'>('dcopf');
	let budget = $state(20),
		increment = $state(5),
		cardinality = $state(2),
		increase = $state(10),
		target = $state(1);
	let region = $state('');
	let interpretationJson = $state('');
	let formError = $state<string | null>(null);
	let goalForComparison = $state('');
	let historyLimit = $state(30);
	let solveBudget = $state(12);
	const doc = $derived(workspace.document);
	const goal = $derived(workspace.goal);
	const network = $derived(ctrl.app.studyView?.network ?? ctrl.activeSolvable?.network);
	const states = $derived.by(() => {
		if (!doc) return [];
		const order = new Map<string, number>();
		for (const experiment of doc.experiment_order) {
			for (const state of doc.experiments[experiment].result_states) {
				if (!order.has(state)) order.set(state, order.size);
			}
		}
		const entries = Object.entries(doc.states).sort(
			([a], [b]) => (order.get(a) ?? Infinity) - (order.get(b) ?? Infinity) || a.localeCompare(b)
		);
		const children = new Map<string | null, typeof entries>();
		for (const entry of entries) {
			const parent = entry[1].parent ?? null;
			const siblings = children.get(parent) ?? [];
			siblings.push(entry);
			children.set(parent, siblings);
		}
		const stack = [...(children.get(null) ?? [])].reverse();
		const ordered: typeof entries = [];
		while (stack.length && ordered.length < historyLimit) {
			const entry = stack.pop()!;
			ordered.push(entry);
			for (const child of [...(children.get(entry[0]) ?? [])].reverse()) stack.push(child);
		}
		return ordered;
	});
	const latestProposal = $derived(
		doc?.experiment_order.toReversed().find((id) => {
			const e = doc!.experiments[id];
			return (
				e.kind === 'planning' &&
				e.goal === doc!.active_goal &&
				!!doc!.recommended_state &&
				e.result_states.includes(doc!.recommended_state)
			);
		})
	);
	const consequences = $derived.by(() => {
		const comparison = workspace.comparison;
		if (!comparison) return [];
		const left = comparison.left_view.lmp ?? comparison.left_view.vm ?? [];
		const right = comparison.right_view.lmp ?? comparison.right_view.vm ?? [];
		const baseline = new Map(left.map((v) => [v.bus, v.value]));
		return right
			.map((v) => ({
				bus: v.bus,
				before: baseline.get(v.bus) ?? NaN,
				after: v.value,
				change: v.value - (baseline.get(v.bus) ?? NaN)
			}))
			.sort((a, b) => Math.abs(b.change) - Math.abs(a.change))
			.slice(0, 10);
	});

	onMount(() => {
		void workspace.initialize();
		return () => workspace.dispose();
	});
	async function attempt(run: () => Promise<unknown>) {
		formError = null;
		try {
			await run();
		} catch (error) {
			formError = error instanceof Error ? error.message : String(error);
		}
	}
	function interpret() {
		if (!network) throw new Error('Load a solvable network first');
		const ids = region
			.split(',')
			.map((s) => s.trim())
			.filter(Boolean);
		const buses = ids.length
			? network.buses.filter((b) => ids.includes(String(b.id)) || (!!b.uid && ids.includes(b.uid)))
			: network.buses
					.filter((b) => b.demand_mw > 0)
					.sort((a, b) => b.demand_mw - a.demand_mw)
					.slice(0, 5);
		if (!buses.length || (ids.length && buses.length !== new Set(ids).size))
			throw new Error('Every region bus must resolve to one bus in this network');
		const weightTotal = buses.reduce((sum, b) => sum + Math.max(0, b.demand_mw), 0);
		const objective: StudyObjective =
			objectiveKind === 'price'
				? {
						kind: 'weighted_observable',
						operand: { Price: 'Active' },
						weights: buses.map((b) => ({
							element: b.uid ?? b.id,
							weight: weightTotal > 0 ? Math.max(0, b.demand_mw) / weightTotal : 1 / buses.length
						}))
					}
				: {
						kind: 'sum',
						terms: buses.map((b) => ({
							kind: 'squared_target',
							target,
							expression: {
								kind: 'weighted_observable',
								operand: { Voltage: 'Magnitude' },
								weights: [{ element: b.uid ?? b.id, weight: 1 }]
							}
						}))
					};
		const candidates =
			intervention === 'capacity'
				? network.branches
						.filter((b) => b.editable !== false && b.status === 1 && b.rate_mw > 0)
						.slice(0, 8)
				: network.buses
						.filter((b) => b.editable !== false && b.demand_mw > 0)
						.sort((a, b) => b.demand_mw - a.demand_mw)
						.slice(0, 8);
		const decisions: DecisionSpace = {
			variables: candidates.map((e) => ({
				id: `${intervention === 'capacity' ? 'rating' : 'demand'}:${e.uid ?? e.id}`,
				element: e.uid ?? e.id,
				intervention: intervention === 'capacity' ? 'branch_rating' : 'active_demand',
				lower:
					intervention === 'redistribution' && 'demand_mw' in e
						? -Math.min(budget, Math.floor(e.demand_mw / increment) * increment)
						: 0,
				upper: budget,
				increment,
				budget_weight: 1
			})),
			total_budget: budget,
			max_changed_elements: cardinality,
			demand:
				intervention === 'capacity'
					? null
					: intervention === 'placement'
						? { kind: 'placement', increase_mw: increase }
						: { kind: 'redistribution' }
		};
		interpretationJson = JSON.stringify({ objective, decisions }, null, 2);
		return { objective, decisions };
	}
	function draft(): GoalDraft {
		const fields = interpretationJson.trim()
			? (JSON.parse(interpretationJson) as { objective: StudyObjective; decisions: DecisionSpace })
			: interpret();
		return {
			title,
			request,
			formulation,
			interpretation: `${objectiveKind === 'price' ? 'Minimize weighted active LMP' : 'Minimize squared voltage deviations'} with the resolved objective and permitted interventions shown below.`,
			...fields,
			success_value: null
		};
	}
	async function create() {
		const c = ctrl.activeSolvable;
		if (!c) throw new Error('Select a solvable case first');
		await workspace.create(draft(), c.id, caseRevision(c));
		expanded = true;
	}
	async function revise() {
		if (!doc?.inspected_state) return;
		const next = draft();
		await operation({
			kind: 'revise_goal',
			goal: {
				parent: doc.active_goal,
				anchor_state: doc.inspected_state,
				request: next.request,
				interpretation: next.interpretation,
				objective: next.objective,
				decisions: next.decisions,
				success_value: next.success_value
			}
		});
	}
	async function operation(op: StudyOperation) {
		if (!doc) return;
		return workspace.execute(doc.id, doc.revision, op);
	}
	function download(text: string, name: string) {
		const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = name;
		link.click();
		setTimeout(() => URL.revokeObjectURL(url), 1000);
	}
	function loadGoal() {
		if (!goal) return;
		request = goal.request;
		interpretationJson = JSON.stringify(
			{ objective: goal.objective, decisions: goal.decisions },
			null,
			2
		);
	}
	function stateLabel(id: string | null | undefined) {
		return id ? (doc?.states[id]?.label ?? id.slice(0, 10)) : 'None';
	}
	function depth(id: string) {
		let n = 0,
			parent = doc?.states[id]?.parent;
		while (parent && n < 8) {
			n++;
			parent = doc?.states[parent]?.parent;
		}
		return n;
	}
	const number = (v: number) =>
		Number.isFinite(v) ? v.toLocaleString(undefined, { maximumFractionDigits: 5 }) : 'Unavailable';
</script>

<TellegenWebMcp {workspace} />
<div class="study-workspace" class:expanded>
	<button class="study-toggle" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
		<span>Studies</span><span>{doc ? doc.title : 'Explore a goal'} {expanded ? '-' : '+'}</span>
	</button>
	{#if expanded}
		<section aria-label="Study workspace">
			<div class="toolbar">
				<label
					>Saved study <select
						value={doc?.id ?? ''}
						disabled={workspace.busy}
						onchange={(e) => {
							const id = e.currentTarget.value;
							if (id) void attempt(() => workspace.open(id));
						}}
						><option value="">Choose a study</option
						>{#each workspace.saved as saved (saved.id)}<option value={saved.id}
								>{saved.title}, revision {saved.revision}</option
							>{/each}</select
					></label
				>
				<label class="file-button"
					>Import<input
						type="file"
						accept=".json,application/json"
						disabled={workspace.busy}
						onchange={(e) => {
							const file = e.currentTarget.files?.[0];
							if (file)
								void attempt(async () => {
									if (file.size > 512 * 1024 * 1024)
										throw new Error('Study bundle exceeds 512 MiB');
									await workspace.import(await file.text());
								});
							e.currentTarget.value = '';
						}}
					/></label
				>
				{#if doc}<button onclick={() => download(workspace.export(), 'tellegen-study.json')}
						>Export</button
					>{/if}
			</div>
			{#if formError || workspace.error}<p class="error" role="alert">
					{formError ?? workspace.error}
				</p>{/if}
			{#if doc}
				<p class="eyebrow">
					Revision {doc.revision}, {Object.keys(doc.states).length} saved states
				</p>
				<p class="request">
					{goal?.request ?? 'Historical evidence; electrical states unavailable.'}
				</p>
				<dl class="pointers">
					<div>
						<dt>Inspecting</dt>
						<dd>{stateLabel(doc.inspected_state)}</dd>
					</div>
					<div>
						<dt>Recommended</dt>
						<dd>{stateLabel(doc.recommended_state)}</dd>
					</div>
					<div>
						<dt>Applied</dt>
						<dd>{stateLabel(doc.applied_state)}</dd>
					</div>
				</dl>
				<div class="toolbar">
					<button onclick={() => void attempt(() => workspace.showView())}
						>Show saved state on map</button
					>
					<button onclick={() => workspace.closeView()}>Return to live case</button>
					{#if ctrl.app.studyView}<label
							>Map values<select bind:value={ctrl.app.displayMode}
								><option value="price">Active LMP</option><option value="voltage"
									>Voltage magnitude</option
								><option value="angle">Voltage angle</option></select
							></label
						>{/if}
				</div>
				<p class="hint">
					Saved-state map. Inspecting changes the view; the Apply button changes the Study's applied
					state.
				</p>
				<details>
					<summary>Goal interpretation and permitted interventions</summary>
					<p>{goal?.interpretation}</p>
					<pre>{JSON.stringify(goal, null, 2)}</pre>
				</details>
				<div class="toolbar">
					<label
						>Exact solve budget<input
							type="number"
							min="1"
							max="256"
							bind:value={solveBudget}
						/></label
					>
					<button
						class="primary"
						disabled={workspace.busy || !doc.inspected_state || !doc.active_goal}
						onclick={() =>
							void attempt(() =>
								operation({
									kind: 'propose',
									state: doc!.inspected_state!,
									goal: doc!.active_goal!,
									options: {
										max_solves: solveBudget,
										max_iterations: 12,
										beam_width: 3,
										min_improvement: 1e-7
									},
									rationale:
										'Explore feasible interventions from the inspected state toward the active goal'
								})
							)}>Find a proposal</button
					>
					{#if workspace.busy}<button onclick={() => workspace.cancel()}>Cancel</button>{/if}
				</div>
				{#if latestProposal && doc.recommended_state !== doc.applied_state}<button
						class="primary"
						disabled={workspace.busy}
						onclick={() => void attempt(() => workspace.applyFromUser(latestProposal!))}
						>Apply this recommendation to the Study</button
					>{/if}
				<h3>Explored states</h3>
				<ul class="history">
					{#each states as [id, state] (id)}<li style:padding-left="{depth(id) * 10}px">
							<button
								class:selected={id === doc.inspected_state}
								disabled={workspace.busy}
								onclick={() => void attempt(() => operation({ kind: 'inspect', state: id }))}
								>{state.label}{id === doc.recommended_state ? ' [recommended]' : ''}{id ===
								doc.applied_state
									? ' [applied]'
									: ''}</button
							><button
								class="branch"
								disabled={workspace.busy}
								onclick={() =>
									void attempt(() =>
										operation({
											kind: 'branch',
											state: id,
											rationale: 'Continue exploration from this saved state'
										})
									)}>Branch here</button
							>
						</li>{/each}
				</ul>
				{#if Object.keys(doc.states).length > historyLimit}<button
						onclick={() => (historyLimit += 30)}>More states</button
					>{/if}
				<div class="toolbar">
					<label
						>Compare under goal<select bind:value={goalForComparison}
							><option value="">Active goal</option
							>{#each Object.entries(doc.goals) as [id, g] (id)}<option value={id}
									>{g.request.slice(0, 60)}</option
								>{/each}</select
						></label
					><button
						disabled={workspace.busy || !doc.inspected_state || !goal}
						onclick={() =>
							void attempt(() => {
								const id = goalForComparison || doc!.active_goal!;
								return operation({
									kind: 'compare',
									goal: id,
									left: doc!.goals[id].anchor_state,
									right: doc!.inspected_state!
								});
							})}>Compare with starting point</button
					>
				</div>
				{#if workspace.comparison}<div class="comparison">
						<h3>Goal progress</h3>
						<p>
							{number(workspace.comparison.left_value)} to {number(
								workspace.comparison.right_value
							)}; improvement {number(workspace.comparison.improvement)}
						</p>
						<p class="hint">
							Largest changes across the network ({workspace.comparison.right_view.lmp
								? 'active LMP'
								: 'voltage magnitude'}).
						</p>
						<table>
							<thead><tr><th>Bus</th><th>Before</th><th>After</th><th>Change</th></tr></thead><tbody
								>{#each consequences as row (row.bus)}<tr
										><td>{row.bus}</td><td>{number(row.before)}</td><td>{number(row.after)}</td><td
											>{number(row.change)}</td
										></tr
									>{/each}</tbody
							>
						</table>
					</div>{/if}
				<h3>Experiments and evidence</h3>
				{#each doc.experiment_order.toReversed().slice(0, historyLimit) as id (id)}{@const e =
						doc.experiments[id]}
					<details>
						<summary>{e.kind}: {e.termination}, {e.solve_count} solves</summary>
						<p>{e.rationale}</p>
						<p class="hint">{e.trials.length} numerical trials belong to this operation.</p>
						<pre>{JSON.stringify(e, null, 2)}</pre>
						{#each e.evidence as ref (ref)}<details>
								<summary>Evidence {ref.slice(0, 10)}</summary>
								<pre>{workspace.bundle?.artifacts[ref]?.text}</pre>
							</details>{/each}
					</details>{/each}
			{/if}
			<details open={!doc}>
				<summary>{doc ? 'Revise this goal or start another Study' : 'Define a Study'}</summary>
				<label>Title<input bind:value={title} /></label>
				<label>User goal<textarea rows="3" bind:value={request}></textarea></label>
				<div class="form-grid">
					<label
						>Formulation<select bind:value={formulation}
							><option value="dcopf">DC OPF</option><option value="socwr">SOCWR OPF</option><option
								value="acpf">AC power flow</option
							></select
						></label
					><label
						>Objective<select bind:value={objectiveKind}
							><option value="price">Weighted active LMP</option><option value="voltage"
								>Voltage target</option
							></select
						></label
					>
				</div>
				<label
					>Region bus IDs, comma separated<input
						bind:value={region}
						placeholder="Default: five buses with greatest demand"
					/></label
				>
				{#if objectiveKind === 'voltage'}<label
						>Voltage target (pu)<input type="number" step="0.01" bind:value={target} /></label
					>{/if}
				<label
					>Permitted interventions<select bind:value={intervention}
						><option value="capacity">Capacity upgrades</option><option value="redistribution"
							>Demand redistribution</option
						><option value="placement">Demand placement</option></select
					></label
				>
				<div class="form-grid">
					<label
						>Total absolute change budget<input type="number" min="0" bind:value={budget} /></label
					><label>Increment<input type="number" min="0.001" bind:value={increment} /></label><label
						>Maximum changed elements<input type="number" min="1" bind:value={cardinality} /></label
					>{#if intervention === 'placement'}<label
							>Total added demand (MW)<input type="number" min="0" bind:value={increase} /></label
						>{/if}
				</div>
				<p class="hint">
					Capacity uses MW for DC OPF and MVA for SOCWR. Demand uses MW; transfers count both ends
					toward the absolute-change budget. AC power flow supports demand decisions and voltage
					objectives.
				</p>
				<button
					onclick={() =>
						void attempt(async () => {
							interpret();
						})}>Resolve equipment and weights</button
				>
				{#if goal}<button onclick={loadGoal}>Load current interpretation</button>{/if}
				<label
					>Resolved objective and decisions<textarea
						class="json"
						rows="8"
						bind:value={interpretationJson}
						placeholder="Resolve equipment, then review or edit the weights, bounds and candidate IDs."
					></textarea></label
				>
				<p class="hint">
					The resolved JSON is authoritative. Resolve again after changing the controls. Default
					candidates are limited to eight elements; edit this list to choose corridors or demand
					locations.
				</p>
				<div class="toolbar">
					<button
						disabled={workspace.busy || !ctrl.activeSolvable}
						onclick={() => void attempt(create)}>Create from live case</button
					>{#if doc?.inspected_state}<button
							disabled={workspace.busy}
							onclick={() => void attempt(revise)}>Save goal revision from inspected state</button
						>{/if}
				</div>
			</details>
		</section>
	{/if}
</div>

<style>
	.study-workspace {
		position: fixed;
		left: 20px;
		bottom: 22px;
		z-index: 25;
		width: min(440px, calc(100vw - 40px));
		color: var(--ink, #20242b);
		font: 12px/1.5 var(--font-mono, monospace);
		background: var(--panel, #fcfbf7);
		border: 1px solid var(--line, #d5d2ca);
		border-radius: 5px;
		box-shadow: 0 4px 18px #20242b18;
	}
	.study-toggle {
		display: flex;
		justify-content: space-between;
		width: 100%;
		padding: 12px;
		border: 0;
		background: transparent;
		text-align: left;
	}
	.study-toggle span:first-child {
		font-weight: 700;
	}
	section {
		padding: 0 14px 14px;
		max-height: min(76vh, 900px);
		overflow: auto;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin: 8px 0;
		flex: 1;
	}
	input,
	select,
	textarea,
	button {
		font: inherit;
		color: inherit;
	}
	input,
	select,
	textarea {
		min-width: 0;
		width: 100%;
		box-sizing: border-box;
		background: #fff;
		border: 1px solid var(--line, #d5d2ca);
		padding: 7px;
		border-radius: 3px;
	}
	button,
	.file-button {
		padding: 7px 9px;
		border: 1px solid var(--line, #d5d2ca);
		border-radius: 3px;
		background: transparent;
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: wait;
	}
	button:hover:enabled,
	button.selected {
		background: #e9eee8;
	}
	.primary:hover:enabled {
		background: #306255;
	}
	.primary {
		background: #244a40;
		color: #fff;
	}
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		align-items: end;
		margin: 10px 0;
	}
	.file-button {
		flex: 0;
		margin: 0;
	}
	.file-button input {
		width: 115px;
	}
	.form-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
	}
	.request {
		font-size: 15px;
		font-family: var(--font-sans, sans-serif);
	}
	.eyebrow,
	.hint,
	dt {
		color: #5d635e;
		font-size: 10px;
	}
	.error {
		color: #8d2a21;
		background: #fff0ed;
		padding: 8px;
	}
	.pointers {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 8px;
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	details {
		border-top: 1px solid var(--line, #d5d2ca);
		padding: 10px 0;
	}
	summary {
		cursor: pointer;
	}
	pre {
		font: 10px/1.5 var(--font-mono, monospace);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		max-height: 260px;
		overflow: auto;
	}
	.json {
		font-size: 10px;
	}
	h3 {
		font-size: 12px;
		margin: 18px 0 6px;
	}
	.history {
		padding: 0;
		list-style: none;
	}
	.history li {
		display: flex;
		gap: 3px;
		margin: 4px 0;
	}
	.history li > button:first-child {
		flex: 1;
		text-align: left;
	}
	.branch {
		font-size: 10px;
		white-space: nowrap;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 10px;
	}
	td,
	th {
		padding: 5px 2px;
		text-align: right;
		border-bottom: 1px solid var(--line, #d5d2ca);
	}
	:global(body:has(.study-workspace.expanded) [data-webmcp-activity]) {
		display: none;
	}
	@media (max-width: 760px) {
		.study-workspace:not(.expanded) {
			width: calc(100vw - 190px);
		}
		.study-toggle {
			gap: 8px;
		}
		.study-toggle span:last-child {
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}
		.study-workspace {
			left: 8px;
			bottom: 8px;
			width: calc(100vw - 16px);
		}
		section {
			max-height: 65vh;
		}
	}
</style>
