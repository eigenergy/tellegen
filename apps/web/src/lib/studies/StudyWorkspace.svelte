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
	let tab = $state<'goal' | 'states' | 'timeline'>('states');
	let editing = $state(false);
	let creating = $state(false);
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
	let demandBus = $state('');
	let demandIncrement = $state(1);
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
				['planning', 'counterfactual'].includes(e.kind) &&
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
	function activityLabel(kind: string) {
		return (
			(
				{
					inspection: 'Observation',
					sensitivity: 'Sensitivity analysis',
					planning: 'Plan trials',
					counterfactual: 'Edit and solve',
					challenge: 'Scenario assessment',
					historical_import: 'Imported activity'
				} as Record<string, string>
			)[kind] ?? kind
		);
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
		creating = false;
		editing = false;
		tab = 'states';
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
		editing = true;
		creating = false;
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

<TellegenWebMcp {workspace} studyExpanded={expanded} closeStudy={() => (expanded = false)} />
<div class="study-workspace" class:expanded style:--study-top="{ctrl.app.headerInset + 10}px">
	{#if !expanded}
		<button
			class="study-toggle"
			aria-label="Studies"
			aria-expanded="false"
			onclick={() => (expanded = true)}
			><svg
				width="16"
				height="16"
				viewBox="0 0 20 20"
				fill="none"
				stroke="currentColor"
				stroke-width="1.3"
				aria-hidden="true"
				><path d="M5 4v12m0-8h8m0 0V4m0 4v8" /><circle
					cx="5"
					cy="3"
					r="2"
					fill="var(--paper)"
				/><circle cx="5" cy="17" r="2" fill="var(--paper)" /><circle
					cx="13"
					cy="3"
					r="2"
					fill="var(--paper)"
				/><circle cx="13" cy="17" r="2" fill="var(--paper)" /></svg
			><span>Studies</span>{#if doc}<span class="study-name">{doc.title}</span>{/if}</button
		>
	{:else}
		<header class="workspace-header">
			<h2>Studies</h2>
			<button
				class="text-button"
				onclick={() => {
					creating = true;
					editing = false;
					interpretationJson = '';
					tab = 'goal';
				}}>New study</button
			><button class="close" aria-label="Close studies" onclick={() => (expanded = false)}
				><svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"
					><path d="m4 4 8 8m0-8-8 8" fill="none" stroke="currentColor" stroke-width="1.5" /></svg
				></button
			>
		</header>
		<div class="storage">
			<label
				>Saved study <select
					value={doc?.id ?? ''}
					disabled={workspace.busy}
					onchange={(e) => {
						const id = e.currentTarget.value;
						if (id) void attempt(() => workspace.open(id));
					}}
					><option value="">Open a saved study</option
					>{#each workspace.saved as saved (saved.id)}<option value={saved.id}>{saved.title}</option
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
								if (file.size > 512 * 1024 * 1024) throw new Error('Study bundle exceeds 512 MiB');
								await workspace.import(await file.text());
							});
						e.currentTarget.value = '';
					}}
				/></label
			>
			{#if doc}<button
					disabled={workspace.busy}
					onclick={() => download(workspace.export(), 'tellegen-study.json')}>Export</button
				>{/if}
		</div>
		<nav aria-label="Study sections">
			<button
				class:active={!doc || creating || tab === 'goal'}
				aria-pressed={!doc || creating || tab === 'goal'}
				onclick={() => {
					tab = 'goal';
					creating = false;
				}}>Goal</button
			>
			<button
				disabled={!doc}
				class:active={!!doc && !creating && tab === 'states'}
				aria-pressed={!!doc && !creating && tab === 'states'}
				onclick={() => {
					tab = 'states';
					creating = false;
				}}
				>States {#if doc}<span>{Object.keys(doc.states).length}</span>{/if}</button
			>
			<button
				disabled={!doc}
				class:active={!!doc && !creating && tab === 'timeline'}
				aria-pressed={!!doc && !creating && tab === 'timeline'}
				onclick={() => {
					tab = 'timeline';
					creating = false;
				}}>Timeline</button
			>
		</nav>
		<section class="workspace-content" aria-label="Study workspace">
			{#if formError || workspace.error}<p class="error" role="alert">
					{formError ?? workspace.error}
				</p>{/if}
			{#if !doc || creating || tab === 'goal'}
				{#if !doc || creating || editing}
					<h3>{doc && !creating ? 'Revise goal' : 'New study'}</h3>

					<label>Title<input bind:value={title} /></label>
					<label>Goal<textarea rows="2" bind:value={request}></textarea></label>
					<div class="form-grid">
						<label
							>Formulation<select bind:value={formulation}
								><option value="dcopf">DC OPF</option><option value="socwr">SOCWR OPF</option
								><option value="acpf">AC power flow</option></select
							></label
						><label
							>Objective<select bind:value={objectiveKind}
								><option value="price">Weighted active LMP</option><option value="voltage"
									>Voltage target</option
								></select
							></label
						>
					</div>
					<label>Buses<input bind:value={region} placeholder="Bus IDs, e.g. 2, 14, 30" /></label>
					{#if objectiveKind === 'voltage'}<label
							>Voltage target (pu)<input type="number" step="0.01" bind:value={target} /></label
						>{/if}
					<label
						>Allowed changes<select bind:value={intervention}
							><option value="capacity">Capacity upgrades</option><option value="redistribution"
								>Demand redistribution</option
							><option value="placement">Demand placement</option></select
						></label
					>
					<div class="form-grid">
						<label>Budget<input type="number" min="0" bind:value={budget} /></label><label
							>Increment<input type="number" min="0.001" bind:value={increment} /></label
						><label>Maximum elements<input type="number" min="1" bind:value={cardinality} /></label
						>{#if intervention === 'placement'}<label
								>Total added demand (MW)<input type="number" min="0" bind:value={increase} /></label
							>{/if}
					</div>
					<p class="hint">
						{intervention === 'capacity'
							? formulation === 'socwr'
								? 'Ratings in MVA.'
								: 'Ratings in MW.'
							: 'Demand in MW. Transfers count both ends toward the budget.'}
					</p>
					<details class="advanced">
						<summary>Equipment, weights and bounds</summary>
						<button
							onclick={() =>
								void attempt(async () => {
									interpret();
								})}>Resolve equipment and weights</button
						>

						<label
							>Objective and decisions (JSON)<textarea
								class="json"
								rows="8"
								bind:value={interpretationJson}
								placeholder="Resolve equipment, then review or edit the weights, bounds and candidate IDs."
							></textarea></label
						>
						<p class="hint">
							Defaults: five load buses, eight candidate elements. Resolve again after changing the
							form.
						</p>
					</details>
				{:else}
					<h3>{doc.title}</h3>
					<p class="request">{goal?.request ?? 'Imported history'}</p>
					{#if goal}<dl class="goal-facts">
							<div>
								<dt>Formulation</dt>
								<dd>
									{doc.inspected_state
										? {
												dcopf: 'DC OPF',
												dcpf: 'DC power flow',
												acpf: 'AC power flow',
												socwr: 'SOCWR OPF',
												acopf: 'AC OPF'
											}[doc.states[doc.inspected_state].formulation]
										: 'Unavailable'}
								</dd>
							</div>
							<div>
								<dt>Candidate elements</dt>
								<dd>{goal.decisions.variables.length}</dd>
							</div>
							<div>
								<dt>Maximum changes</dt>
								<dd>{goal.decisions.max_changed_elements}</dd>
							</div>
							<div>
								<dt>Budget</dt>
								<dd>{goal.decisions.total_budget}</dd>
							</div>
						</dl>
						<details>
							<summary>Objective and permitted changes</summary>
							<p>{goal.interpretation}</p>
							<pre>{JSON.stringify(
									{ objective: goal.objective, decisions: goal.decisions },
									null,
									2
								)}</pre>
						</details>
						<button onclick={loadGoal}>Edit goal</button>{/if}
				{/if}
			{:else if tab === 'states'}
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
					<button onclick={() => void attempt(() => workspace.showView())}>Show on map</button>
					<button onclick={() => workspace.closeView()}>Live case</button>
					{#if ctrl.app.studyView}<label
							>Map values<select bind:value={ctrl.app.displayMode}
								><option value="price">Active LMP</option><option value="voltage"
									>Voltage magnitude</option
								><option value="angle">Voltage angle</option></select
							></label
						>{/if}
				</div>
				<div class="toolbar">
					<label
						>Solve limit<input type="number" min="1" max="256" bind:value={solveBudget} /></label
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
						>Apply recommendation</button
					>{/if}
				<details>
					<summary>Edit demand</summary>
					<p class="hint">Edits accumulate at each bus. Transfers must balance before applying.</p>
					<div class="toolbar">
						<label>Bus<input bind:value={demandBus} placeholder="2" /></label>
						<label>Change (MW)<input type="number" bind:value={demandIncrement} /></label>
						<button
							disabled={workspace.busy ||
								!doc.inspected_state ||
								!doc.active_goal ||
								!demandBus.trim()}
							onclick={() =>
								void attempt(() =>
									operation({
										kind: 'edit_demand',
										state: doc!.inspected_state!,
										goal: doc!.active_goal!,
										changes: [{ bus: demandBus.trim(), delta_mw: demandIncrement }],
										rationale: `Adjust bus ${demandBus} demand by ${demandIncrement} MW`
									})
								)}>Solve edit</button
						>
					</div>
				</details>
				<h3>Network states</h3>
				<ul class="history">
					{#each states as [id, state] (id)}<li style:padding-left="{depth(id) * 10}px">
							<button
								class:selected={id === doc.inspected_state}
								disabled={workspace.busy}
								onclick={() => void attempt(() => operation({ kind: 'inspect', state: id }))}
								><span>{state.label}</span><span class="state-flags"
									>{#if id === doc.recommended_state}<small>Recommended</small
										>{/if}{#if id === doc.applied_state}<small>Applied</small>{/if}</span
								></button
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
									)}>Branch</button
							>
						</li>{/each}
				</ul>
				{#if Object.keys(doc.states).length > historyLimit}<button
						onclick={() => (historyLimit += 30)}>More states</button
					>{/if}
				<div class="toolbar">
					<label
						>Goal revision<select bind:value={goalForComparison}
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
				<details class="reset">
					<summary>Restore original network</summary>
					<button
						disabled={workspace.busy || !doc.base_input || !doc.inspected_state || !doc.active_goal}
						onclick={() =>
							void attempt(() =>
								operation({
									kind: 'restore_base',
									state: doc!.inspected_state!,
									goal: doc!.active_goal!,
									rationale: 'Restore the original network data'
								})
							)}>Restore base case</button
					>
					<p class="hint">
						{doc.base_input
							? 'Creates a new candidate from the original network data.'
							: 'Original network data is unavailable in this import.'}
					</p>
				</details>
			{:else}
				<h3>Activity timeline</h3>
				{#each doc.experiment_order.toReversed().slice(0, historyLimit) as id (id)}{@const e =
						doc.experiments[id]}
					<details>
						<summary
							><span>{activityLabel(e.kind)}</span><span class="event-status"
								>{e.termination.replaceAll('_', ' ')}</span
							></summary
						>
						<p>{e.rationale}</p>
						<p class="hint">{e.solve_count} solves, {e.trials.length} trials</p>
						<pre>{JSON.stringify(e, null, 2)}</pre>
						{#each e.evidence as ref (ref)}<details>
								<summary>Evidence {ref.slice(0, 10)}</summary>
								<pre>{workspace.bundle?.artifacts[ref]?.text}</pre>
							</details>{/each}
					</details>{/each}
			{/if}
		</section>
		{#if !doc || creating || (tab === 'goal' && editing)}<div class="form-actions">
				{#if !doc || creating}<button
						class="primary"
						disabled={workspace.busy || !ctrl.activeSolvable}
						onclick={() => void attempt(create)}>Create study</button
					>{/if}{#if doc?.inspected_state && !creating}<button
						disabled={workspace.busy}
						onclick={() =>
							void attempt(async () => {
								await revise();
								editing = false;
							})}>Save revision</button
					>{/if}
			</div>{/if}
		{#if doc}<div class="workspace-status">
				Revision {doc.revision}, {Object.keys(doc.states).length} saved states{#if workspace.busy}<span
						>Solving...</span
					>{:else}<span>Saved</span>{/if}
			</div>{/if}
	{/if}
</div>

<style>
	.study-workspace {
		position: fixed;
		left: 20px;
		bottom: 48px;
		z-index: 25;
		color: var(--ink);
		font: 13px/1.5 var(--font-display);
	}
	.study-workspace.expanded {
		top: var(--study-top);
		bottom: auto;
		display: flex;
		flex-direction: column;
		width: min(410px, calc(100vw - 40px));
		max-height: calc(100dvh - var(--study-top) - 104px);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 6px;
		box-shadow: var(--elev-2);
		overflow: hidden;
	}
	.study-toggle {
		display: flex;
		align-items: center;
		gap: 10px;
		max-width: 320px;
		min-height: 38px;
		background: var(--paper);
		box-shadow: var(--elev-1);
	}
	.study-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-secondary);
		max-width: 190px;
		font-size: 11px;
	}
	.workspace-header {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 12px 16px;
		flex: none;
	}
	h2 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
		flex: 1;
	}
	.workspace-header .text-button {
		padding: 4px 0;
		border: 0;
		font-size: 11px;
		color: var(--text-secondary);
	}
	.workspace-header .close {
		padding: 0;
		width: 28px;
		height: 28px;
		display: grid;
		place-items: center;
		border: 0;
	}
	.storage {
		display: flex;
		align-items: flex-end;
		gap: 8px;
		padding: 0 16px 16px;
	}
	.storage label:first-child {
		flex: 1;
		min-width: 0;
	}
	.storage label:first-child :global(select) {
		width: 100%;
	}
	.storage .file-button {
		position: relative;
		flex: none;
		overflow: hidden;
		padding: 8px;
		font-weight: 400;
		cursor: pointer;
		border: 1px solid var(--line);
		border-radius: 4px;
	}
	.file-button input {
		position: absolute;
		inset: 0;
		width: 100%;
		opacity: 0;
		cursor: pointer;
	}
	.file-button:focus-within {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.storage > button {
		padding: 8px;
	}
	nav {
		display: flex;
		gap: 24px;
		padding: 0 16px;
		border-bottom: 1px solid var(--line);
		flex: none;
	}
	nav button {
		border: 0;
		border-bottom: 2px solid transparent;
		border-radius: 0;
		padding: 8px 0 10px;
		color: var(--text-secondary);
	}
	nav button.active {
		border-bottom-color: var(--accent);
		color: var(--ink);
	}
	nav button span {
		font-size: 11px;
		margin-left: 4px;
		color: var(--text-secondary);
	}
	.workspace-content {
		padding: 20px;
		overflow: auto;
		min-height: 0;
		overscroll-behavior: contain;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin: 0 0 16px;
		min-width: 0;
		font-size: 12px;
	}
	.storage label,
	.toolbar label {
		margin: 0;
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
		background: white;
		border: 1px solid var(--line);
		padding: 8px 9px;
		border-radius: 4px;
	}
	textarea {
		resize: vertical;
	}
	button {
		padding: 8px 12px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: transparent;
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.45;
		cursor: default;
	}
	button:hover:enabled {
		background: var(--accent-soft);
	}
	button.primary {
		background: var(--ink);
		border-color: var(--ink);
		color: white;
	}
	button.primary:hover:enabled {
		background: #3c4249;
	}
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: flex-end;
		margin: 16px 0;
	}
	.toolbar label {
		flex: 1;
	}
	.toolbar label:has(input[type='number']) {
		max-width: 132px;
	}
	.form-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0 16px;
	}
	.request {
		margin: 10px 0 20px;
		font-size: 14px;
		line-height: 1.6;
	}
	.hint {
		margin: 10px 0 16px;
		color: var(--text-secondary);
		font-size: 11px;
		line-height: 1.5;
	}
	.error {
		color: var(--red);
		background: #fff0ed;
		padding: 12px;
		overflow-wrap: anywhere;
	}
	.pointers {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 16px;
		margin: 0 0 16px;
	}
	dt {
		color: var(--text-secondary);
		font-size: 11px;
		margin-bottom: 4px;
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
		font-size: 12px;
	}
	.goal-facts {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		margin-bottom: 20px;
	}
	details {
		border-top: 1px solid var(--line);
		padding: 14px 0;
	}
	summary {
		cursor: pointer;
		font-size: 12px;
	}
	summary .event-status {
		display: block;
		margin: 4px 0 0 14px;
		color: var(--text-secondary);
		font-size: 11px;
	}
	pre {
		font: 11px/1.5 var(--font-mono);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		max-height: 260px;
		overflow: auto;
	}
	.json {
		font: 11px/1.5 var(--font-mono);
	}
	h3 {
		font-size: 14px;
		font-weight: 600;
		margin: 4px 0 16px;
	}
	.history {
		padding: 0;
		list-style: none;
		margin: 0 0 20px;
	}
	.history li {
		display: flex;
		gap: 8px;
		margin: 6px 0;
	}
	.history li > button:first-child {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		text-align: left;
		min-width: 0;
	}
	.history button.selected {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.state-flags {
		display: flex;
		flex-direction: column;
		text-align: right;
	}
	.state-flags small {
		font-size: 9px;
		color: var(--text-secondary);
	}
	.branch {
		border: 0;
		font-size: 11px;
		padding: 8px 0;
		color: var(--text-secondary);
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font: 11px/1.5 var(--font-mono);
	}
	td,
	th {
		padding: 8px 4px;
		text-align: right;
		border-bottom: 1px solid var(--line);
	}
	td:first-child,
	th:first-child {
		text-align: left;
	}
	.form-actions {
		display: flex;
		gap: 8px;
		padding: 12px 20px;
		border-top: 1px solid var(--line);
		flex: none;
	}
	.workspace-status {
		display: flex;
		justify-content: space-between;
		border-top: 1px solid var(--line);
		color: var(--text-secondary);
		font-size: 10px;
		padding: 10px 16px;
		flex: none;
	}
	:global(body:has(.study-workspace.expanded) aside.panel) {
		visibility: hidden;
		pointer-events: none;
	}
	@media (max-width: 760px) {
		.study-workspace {
			left: 12px;
			top: var(--study-top);
			bottom: auto;
		}
		.study-workspace.expanded {
			width: calc(100vw - 24px);
			top: calc(var(--study-top) + 46px);
			max-height: calc(100dvh - var(--study-top) - 104px);
		}
		.study-name {
			display: none;
		}
		.workspace-content {
			padding: 16px;
		}
		:global(body:has(.study-workspace.expanded) .agent-workspace:has(.activity-panel)) {
			z-index: 26;
		}
		:global(body:has(.study-workspace.expanded) .maplibregl-ctrl-bottom-right) {
			bottom: 12px;
			opacity: 1;
			pointer-events: auto;
		}
	}
</style>
