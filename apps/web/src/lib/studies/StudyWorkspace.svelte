<script lang="ts">
	import { onMount, tick, untrack } from 'svelte';
	import {
		getController,
		PanelFrame,
		ModelDetails,
		McResults,
		getNoticeCenter,
		errorNoticeTitle
	} from '@tellegen/svelte';
	import type { StudyOperation } from '@tellegen/engine';
	import TellegenWebMcp from '../webmcp/TellegenWebMcp.svelte';
	import { caseRevision } from '../webmcp/tellegen-adapter.js';
	import { StudyWorkspace } from './workspace.svelte.js';
	import GoalDetails from './GoalDetails.svelte';
	import {
		buildGoal,
		defaultGoalForm,
		resolveEquipment,
		candidateBounds,
		selectionKey,
		busLabel,
		branchLabel,
		objectiveLabel,
		type GoalBus,
		type GoalBranch
	} from './goal-form.js';

	const ctrl = getController();
	const workspace = new StudyWorkspace(ctrl);
	const notices = getNoticeCenter();
	$effect(() => {
		const details = workspace.error;
		if (details)
			untrack(() => notices.push({ kind: 'error', title: errorNoticeTitle(details), details }));
	});
	let expanded = $state(false);
	let tab = $state<'case' | 'history' | 'plan'>('case');
	let creating = $state(false);
	let editingGoal = $state(false);
	let title = $state('Grid study');
	let form = $state(defaultGoalForm('dcopf'));
	let formError = $state<string | null>(null);
	let busSearch = $state('');
	let candidateSearch = $state('');
	let busPage = $state(0);
	let candidatePage = $state(0);
	let historyLimit = $state(30);
	let solveBudget = $state(12);
	let demandBus = $state('');
	let demandIncrement = $state(1);
	let demandSearch = $state('');
	let demandPage = $state(0);
	let goalForComparison = $state('');
	let content: HTMLElement | undefined;
	const pageSize = 20;
	const multi = $derived(ctrl.app.activeMulti);
	const mcDoc = $derived(workspace.activeMcDocument);
	const doc = $derived(workspace.document);
	const goal = $derived(workspace.goal);
	const modelDetails = $derived(
		doc && !creating ? doc.model_details : ctrl.activeSolvable?.network?.model_details
	);
	const network = $derived(
		doc && !creating ? (workspace.network ?? undefined) : ctrl.activeSolvable?.network
	);
	const supportedSave = $derived(
		!!ctrl.activeSolvable && ctrl.activeSolvable.formulation !== 'acopf'
	);
	const inspectedState = $derived(doc?.inspected_state ? doc.states[doc.inspected_state] : null);
	const formulation = $derived(
		inspectedState?.formulation ?? ctrl.activeSolvable?.formulation ?? 'dcopf'
	);
	const ratingUnit = $derived(formulation === 'socwr' ? 'MVA' : 'MW');
	const changeUnit = $derived(form.intervention === 'capacity' ? ratingUnit : 'MW');
	const areas = $derived(
		[
			...new Set(
				(network?.buses as GoalBus[] | undefined)
					?.map((bus) => bus.area)
					.filter((area) => area != null)
					.map(String) ?? []
			)
		].sort()
	);
	const equipment = $derived(
		network ? resolveEquipment(network, form) : { buses: [], candidates: [], eligible: [] }
	);
	const weights = $derived(
		new Map(equipment.buses.map(({ bus, weight }) => [selectionKey(bus), weight]))
	);
	const busRows = $derived(
		(network?.buses as GoalBus[] | undefined)?.filter(
			(bus) =>
				(form.objective === 'voltage' || bus.demand_mw > 0) &&
				(form.scope !== 'area' || String(bus.area ?? '') === form.area) &&
				`${busLabel(bus)} ${bus.uid ?? ''}`.toLowerCase().includes(busSearch.toLowerCase())
		) ?? []
	);
	const candidateRows = $derived(
		(form.scope === 'selected' ? equipment.eligible : equipment.candidates).filter((element) =>
			`${'from' in element ? branchLabel(element) : busLabel(element)} ${element.uid ?? ''}`
				.toLowerCase()
				.includes(candidateSearch.toLowerCase())
		)
	);
	const demandRows = $derived(
		workspace.demandRows.filter((row) =>
			`${row.bus} ${row.name ?? ''}`.toLowerCase().includes(demandSearch.toLowerCase())
		)
	);
	const changedDemand = $derived(
		workspace.demandRows.filter((row) => Math.abs(row.deltaMw) > 1e-8)
	);
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
			const event = doc!.experiments[id];
			return (
				['planning', 'counterfactual'].includes(event.kind) &&
				event.goal === doc!.active_goal &&
				!!doc!.recommended_state &&
				event.result_states.includes(doc!.recommended_state)
			);
		})
	);
	const consequences = $derived.by(() => {
		const comparison = workspace.comparison;
		if (!comparison) return [];
		const left = comparison.left_view?.lmp ?? comparison.left_view?.vm ?? [];
		const right = comparison.right_view?.lmp ?? comparison.right_view?.vm ?? [];
		const baseline = new Map(left.map((value) => [value.bus, value.value]));
		return right
			.map((value) => ({
				bus: value.bus,
				before: baseline.get(value.bus) ?? NaN,
				after: value.value,
				change: value.value - (baseline.get(value.bus) ?? NaN)
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
			const message = error instanceof Error ? error.message : String(error);
			formError = workspace.error === message ? null : message;
		}
	}
	async function resetScroll() {
		await tick();
		content?.scrollTo({ top: 0 });
	}
	async function loadMcExample() {
		const response = await fetch('/examples/four-wire.bmopf.json');
		if (!response.ok) throw new Error('The example could not be loaded. Try again.');
		await ctrl.ingestFiles([
			new File([await response.text()], 'four-wire.bmopf.json', { type: 'application/json' })
		]);
		creating = false;
		await resetScroll();
	}
	function newStudy() {
		creating = true;
		tab = 'case';
		goalForComparison = '';
		title = network ? `${network.name} study` : 'Grid study';
		formError = null;
		void resetScroll();
	}
	async function create() {
		const active = ctrl.activeSolvable;
		if (!active) throw new Error('Select a case first.');
		if (active.formulation === 'acopf')
			throw new Error('Saving AC OPF cases is not yet supported.');
		if (!title.trim()) throw new Error('Enter a study name.');
		await workspace.create(
			{ title: title.trim(), formulation: active.formulation },
			active.id,
			caseRevision(active)
		);
		creating = false;
		editingGoal = false;
		goalForComparison = '';
		tab = 'case';
		await resetScroll();
	}
	function startGoal() {
		form = defaultGoalForm(formulation);
		busSearch = '';
		candidateSearch = '';
		busPage = 0;
		candidatePage = 0;
		editingGoal = true;
		tab = 'plan';
		void resetScroll();
	}
	async function saveGoal() {
		if (!doc?.inspected_state || !network) return;
		const next = buildGoal(network, form, formulation);
		await operation({
			kind: 'revise_goal',
			goal: { ...next, parent: doc.active_goal, anchor_state: doc.inspected_state }
		});
		editingGoal = false;
		await resetScroll();
	}
	async function operation(op: StudyOperation) {
		if (!doc) return;
		return workspace.execute(doc.id, doc.revision, op);
	}
	function changeScope() {
		busPage = 0;
		candidatePage = 0;
	}
	function toggleSelection(kind: 'buses' | 'candidates', key: string, checked: boolean) {
		form[kind] = checked
			? [...new Set([...form[kind], key])]
			: form[kind].filter((id) => id !== key);
	}
	function addMapSelection(kind: 'buses' | 'candidates') {
		if (!network) return;
		const element =
			kind === 'candidates' && form.intervention === 'capacity'
				? network.branches.find((branch) => branch.id === ctrl.app.selectedBranch)
				: network.buses.find((bus) => bus.id === ctrl.app.selectedBus);
		if (!element) return;
		form.scope = 'selected';
		toggleSelection(kind, selectionKey(element), true);
		changeScope();
	}
	function updateBound(element: GoalBus | GoalBranch, bound: 'lower' | 'upper', value: number) {
		const key = selectionKey(element);
		form.bounds[key] = { ...form.bounds[key], [bound]: value };
	}
	function changeIntervention() {
		form.candidates = [];
		form.bounds = {};
		candidatePage = 0;
	}
	async function updateDemand() {
		const key = demandBus.trim();
		const matches =
			network?.buses.filter(
				(bus) => String(bus.id) === key || bus.uid === key || (bus as GoalBus).name === key
			) ?? [];
		if (matches.length !== 1) throw new Error('Choose one bus by its ID or a unique name.');
		if (!Number.isFinite(demandIncrement) || demandIncrement === 0)
			throw new Error('Enter a nonzero demand change in MW.');
		await workspace.editDemandFromUser([
			{ bus: matches[0].uid ?? matches[0].id, delta_mw: demandIncrement }
		]);
	}
	async function compare() {
		if (!doc?.inspected_state) return;
		const goalId = goalForComparison || doc.active_goal || null;
		const base = goalId
			? doc.goals[goalId].anchor_state
			: Object.entries(doc.states).find(([, state]) => !state.parent)?.[0];
		if (!base) throw new Error('The starting state is unavailable.');
		await operation({ kind: 'compare', goal: goalId, left: base, right: doc.inspected_state });
	}
	function download(text: string, name: string) {
		const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = name;
		link.click();
		setTimeout(() => URL.revokeObjectURL(url), 1000);
	}
	function stateLabel(id: string | null | undefined) {
		return id ? (doc?.states[id]?.label ?? id.slice(0, 10)) : 'None';
	}
	function depth(id: string) {
		let count = 0;
		let parent = doc?.states[id]?.parent;
		while (parent && count < 8) {
			count++;
			parent = doc?.states[parent]?.parent;
		}
		return count;
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
	const number = (value: number | null | undefined) =>
		value != null && Number.isFinite(value)
			? value.toLocaleString(undefined, { maximumFractionDigits: 4 })
			: 'Unavailable';
	const percent = (value: number) =>
		`${(value * 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}%`;
</script>

<TellegenWebMcp {workspace} studyExpanded={expanded} closeStudy={() => (expanded = false)} />
<PanelFrame id="studies" title="Studies" side="left" order={20} width={400} bind:open={expanded}>
	{#snippet headerActions()}<button
			class="text-button"
			disabled={workspace.busy || !!multi}
			onclick={newStudy}>New study</button
		>{/snippet}
	<div class="study-workspace">
		<div class="storage">
			<label
				><span class="sr-only">Saved study</span><select
					value={multi
						? multi.mcSavedAt && mcDoc
							? `mc:${mcDoc.id}`
							: ''
						: creating
							? ''
							: (doc?.id ?? '')}
					disabled={workspace.busy}
					onchange={(event) => {
						const id = event.currentTarget.value;
						if (id)
							void attempt(async () => {
								if (id.startsWith('mc:')) await workspace.openMulti(id.slice(3));
								else await workspace.open(id);
								creating = false;
								editingGoal = false;
								goalForComparison = '';
								tab = 'case';
								await resetScroll();
							});
					}}
				>
					<option value="">Open a saved study</option>
					{#each workspace.saved as saved (saved.id)}<option value={saved.id}>{saved.title}</option
						>{/each}
					{#each workspace.mcSaved as saved (saved.id)}<option value={`mc:${saved.id}`}
							>{saved.title}, distribution</option
						>{/each}
				</select></label
			>
			<label class="file-button"
				>Import<input
					type="file"
					accept=".json,application/json"
					disabled={workspace.busy}
					onchange={(event) => {
						const file = event.currentTarget.files?.[0];
						if (file)
							void attempt(async () => {
								if (file.size > 512 * 1024 * 1024) throw new Error('Study bundle exceeds 512 MiB.');
								const text = await file.text();
								if (JSON.parse(text)?.schema === 'tellegen-mc-pf-study')
									await workspace.importMulti(text);
								else await workspace.import(text);
								creating = false;
								editingGoal = false;
								goalForComparison = '';
								tab = 'case';
								await resetScroll();
							});
						event.currentTarget.value = '';
					}}
				/></label
			>
			{#if multi ? mcDoc : doc}<button
					disabled={workspace.busy}
					onclick={() =>
						download(multi ? workspace.exportMulti() : workspace.export(), 'tellegen-study.json')}
					>Export</button
				>{/if}
		</div>
		{#if doc && !creating && !multi}<nav aria-label="Study sections">
				{#each ['case', 'history', 'plan'] as section (section)}<button
						class:active={tab === section}
						aria-pressed={tab === section}
						onclick={() => {
							tab = section as typeof tab;
							void resetScroll();
						}}>{{ case: 'Case', history: 'History', plan: 'Plan' }[section]}</button
					>{/each}
			</nav>{/if}
		<section class="workspace-content" aria-label="Study workspace" bind:this={content}>
			{#if formError}<p class="error" role="alert">
					{formError}
				</p>{/if}
			{#if multi}
				<h3>{multi.label}</h3>
				<p class="hint">Distribution power flow</p>
				<div class="action-row">
					{#if multi.solving}<span role="status">Calculating...</span><button
							onclick={() => multi.solveAbort?.abort()}>Cancel</button
						>
					{:else}<button
							class="primary"
							disabled={!multi.mcPfSupported || workspace.busy}
							onclick={() => {
								void ctrl.solveMultiCase(multi).catch(() => {});
							}}>Run power flow</button
						>{/if}
					<button
						disabled={!multi.mcSnapshot || multi.solving || workspace.busy || !!multi.mcSavedAt}
						onclick={() => void attempt(() => workspace.saveMulti())}>Save result</button
					>
				</div>
				{#if !multi.mcPfSupported}<button
						class="text-button"
						onclick={() =>
							notices.push({
								kind: 'warning',
								title: 'AC power flow unavailable',
								details: multi.mcPfReason ?? 'This case cannot run AC power flow'
							})}>Why unavailable</button
					>{/if}
				{#if multi.result}<McResults
						result={multi.result}
						elapsedMs={multi.solveMs}
						selectedBus={multi.selectedBusId}
						selectedEdge={multi.selectedEdgeId}
						graph={multi.graph}
					/>{/if}
			{:else if !doc || creating}
				<h3>Save this case</h3>
				<p class="hint">A saved case with its changes and results.</p>
				<button class="text-button" onclick={() => void attempt(loadMcExample)}
					>Load 4-conductor example</button
				>
				<label>Study name<input bind:value={title} maxlength="200" /></label>
				{#if network}<p class="case-name">
						{network.name}<span>{network.buses.length.toLocaleString()} buses</span>
					</p>{/if}
				<button
					class="primary"
					disabled={workspace.busy || !supportedSave}
					onclick={() => void attempt(create)}>Save study</button
				>
				{#if ctrl.activeSolvable?.formulation === 'acopf'}<p class="hint compact">
						Saving AC OPF cases is not yet supported.
					</p>{/if}
			{:else if tab === 'case'}
				<h3>{doc.title}</h3>
				<dl class="facts">
					<div>
						<dt>Viewing</dt>
						<dd>{stateLabel(doc.inspected_state)}</dd>
					</div>
					<div>
						<dt>Applied</dt>
						<dd>{stateLabel(doc.applied_state)}</dd>
					</div>
					<div>
						<dt>Result</dt>
						<dd>{inspectedState?.solution ? 'Solved' : 'Not solved'}</dd>
					</div>
				</dl>
				<ModelDetails details={modelDetails} />
				<div class="toolbar">
					<button disabled={workspace.busy} onclick={() => void attempt(() => workspace.showView())}
						>{network?.coordinate_space === 'diagram' ? 'Show diagram' : 'Show on map'}</button
					>
					<button onclick={() => workspace.closeView()}>Live case</button>
				</div>
				{#if ctrl.app.studyView}<label
						>{network?.coordinate_space === 'diagram' ? 'Diagram values' : 'Map values'}<select
							bind:value={ctrl.app.displayMode}
							><option value="price" disabled={!ctrl.app.studyView.solution?.lmp?.length}
								>LMP</option
							><option value="voltage" disabled={!ctrl.app.studyView.solution?.vm?.length}
								>Voltage magnitude</option
							><option value="angle" disabled={!ctrl.app.studyView.solution?.va?.length}
								>Voltage angle</option
							></select
						></label
					>{/if}
				{#if latestProposal && doc.recommended_state !== doc.applied_state}<div
						class="recommendation"
					>
						<span>Recommended: {stateLabel(doc.recommended_state)}</span>
						<button
							class="primary"
							disabled={workspace.busy}
							onclick={() => void attempt(() => workspace.applyFromUser(latestProposal!))}
							>Apply recommendation</button
						>
					</div>{/if}
				<details class="demand-editor">
					<summary
						>Bus demand {#if changedDemand.length}<span>{changedDemand.length} changed</span
							>{/if}</summary
					>
					<div class="toolbar">
						<label
							>Bus<input
								bind:value={demandBus}
								placeholder="ID or name"
								list="study-bus-ids"
							/></label
						>
						<datalist id="study-bus-ids"
							>{#each network?.buses ?? [] as bus (bus.id)}<option value={String(bus.id)}
									>{busLabel(bus)}</option
								>{/each}</datalist
						>
						<label>Change (MW)<input type="number" bind:value={demandIncrement} /></label>
					</div>
					<button
						disabled={workspace.busy || !doc.inspected_state || !demandBus.trim()}
						onclick={() => void attempt(updateDemand)}>Update demand</button
					>
					<label class="search"
						>Find bus<input
							type="search"
							value={demandSearch}
							oninput={(event) => {
								demandSearch = event.currentTarget.value;
								demandPage = 0;
							}}
							placeholder="Bus ID or name"
						/></label
					>
					<div class="table-scroll">
						<table>
							<thead><tr><th>Bus</th><th>Base</th><th>Current</th><th>Change</th></tr></thead><tbody
							>
								{#each demandRows.slice(demandPage * pageSize, (demandPage + 1) * pageSize) as row (row.bus)}<tr
										><th title={row.name}>{row.bus}</th><td
											>{number(doc.base_input ? row.baseMw : null)}</td
										><td>{number(row.currentMw)}</td><td
											>{number(doc.base_input ? row.deltaMw : null)}</td
										></tr
									>{/each}
							</tbody>
						</table>
					</div>
					{#if demandRows.length > pageSize}<div class="pagination">
							<button disabled={demandPage === 0} onclick={() => demandPage--}>Previous</button
							><span
								>{demandPage * pageSize + 1}-{Math.min(
									(demandPage + 1) * pageSize,
									demandRows.length
								)} of {demandRows.length}</span
							><button
								disabled={(demandPage + 1) * pageSize >= demandRows.length}
								onclick={() => demandPage++}>Next</button
							>
						</div>{/if}
					<p class="hint compact">All demand values in MW.</p>
				</details>
				<div class="toolbar">
					<button
						disabled={workspace.busy || !doc.base_input || !doc.inspected_state}
						onclick={() => void attempt(() => workspace.resetFromUser())}>Reset to base case</button
					>
				</div>
				{#if !doc.base_input}<p class="hint">
						Original network data is unavailable in this import.
					</p>{/if}
				{#if !goal}<button class="text-button" onclick={startGoal}>Add a planning goal</button>{/if}
			{:else if tab === 'plan'}
				{#if editingGoal || !goal}
					{#if !editingGoal}<h3>Explore a change</h3>
						<p class="hint">Choose an objective and the equipment that can change.</p>
						<button class="primary" onclick={startGoal}>Set a goal</button>
					{:else}
						<h3>{goal ? 'New goal revision' : 'Planning goal'}</h3>
						<label
							>Objective<select
								bind:value={form.objective}
								onchange={() => {
									busPage = 0;
								}}
								><option value="price" disabled={formulation === 'acpf'}>Reduce average LMP</option
								><option value="voltage" disabled={formulation === 'dcopf'}
									>Reach a voltage target</option
								></select
							></label
						>
						{#if form.objective === 'voltage'}<label
								>Voltage target (pu)<input
									type="number"
									min="0.01"
									step="0.01"
									bind:value={form.target}
								/></label
							>{/if}
						<div class="form-grid">
							<label
								>Equipment<select bind:value={form.scope} onchange={changeScope}
									><option value="network">Whole network</option><option
										value="area"
										disabled={!areas.length}>Area{areas.length ? '' : ' (unavailable)'}</option
									><option value="selected">Selected equipment</option></select
								></label
							>
							{#if form.scope === 'area'}<label
									>Area<select bind:value={form.area} onchange={changeScope}
										><option value="">Choose an area</option>{#each areas as area (area)}<option
												value={area}>{area}</option
											>{/each}</select
									></label
								>
							{:else}<label
									>Bus weighting<select bind:value={form.weighting}
										><option value="demand">By demand</option><option value="equal">Equal</option
										></select
									></label
								>{/if}
						</div>
						<details open>
							<summary
								>Objective buses<span>{equipment.buses.length.toLocaleString()}</span></summary
							>
							{#if form.scope === 'area'}<label
									>Bus weighting<select bind:value={form.weighting}
										><option value="demand">By demand</option><option value="equal">Equal</option
										></select
									></label
								>{/if}
							<label class="search"
								>Find bus<input
									type="search"
									value={busSearch}
									oninput={(event) => {
										busSearch = event.currentTarget.value;
										busPage = 0;
									}}
									placeholder="Bus ID or name"
								/></label
							>
							{#if form.scope === 'selected'}<button
									class="text-button"
									disabled={ctrl.app.selectedBus == null}
									onclick={() => addMapSelection('buses')}
									>Add selected map bus{ctrl.app.selectedBus != null
										? ` ${ctrl.app.selectedBus}`
										: ''}</button
								>{/if}
							<div class="table-scroll">
								<table class="equipment-table">
									<thead><tr><th>Bus</th><th>Demand (MW)</th><th>Weight</th></tr></thead><tbody>
										{#each busRows.slice(busPage * pageSize, (busPage + 1) * pageSize) as bus (selectionKey(bus))}<tr
												><th>
													{#if form.scope === 'selected'}<label class="check"
															><input
																type="checkbox"
																checked={form.buses.includes(selectionKey(bus))}
																onchange={(event) =>
																	toggleSelection(
																		'buses',
																		selectionKey(bus),
																		event.currentTarget.checked
																	)}
															/><span>{busLabel(bus)}</span></label
														>{:else}{busLabel(bus)}{/if}
												</th><td>{number(bus.demand_mw)}</td><td
													>{weights.has(selectionKey(bus))
														? percent(weights.get(selectionKey(bus))!)
														: '-'}</td
												></tr
											>{/each}
									</tbody>
								</table>
							</div>
							{#if busRows.length > pageSize}<div class="pagination">
									<button disabled={busPage === 0} onclick={() => busPage--}>Previous</button><span
										>{busPage * pageSize + 1}-{Math.min((busPage + 1) * pageSize, busRows.length)} of
										{busRows.length}</span
									><button
										disabled={(busPage + 1) * pageSize >= busRows.length}
										onclick={() => busPage++}>Next</button
									>
								</div>{/if}
						</details>
						<label
							>Allowed changes<select bind:value={form.intervention} onchange={changeIntervention}
								><option value="capacity" disabled={formulation === 'acpf'}
									>Increase line capacity</option
								><option value="redistribution">Redistribute demand</option><option
									value="placement">Add demand</option
								></select
							></label
						>
						<div class="form-grid">
							<label
								>Change budget ({changeUnit})<input
									type="number"
									min="0"
									bind:value={form.budget}
								/></label
							><label
								>Increment ({changeUnit})<input
									type="number"
									min="0.001"
									step="any"
									bind:value={form.increment}
								/></label
							>
							<label
								>Maximum changed {form.intervention === 'capacity' ? 'lines' : 'buses'}<input
									type="number"
									min="1"
									step="1"
									bind:value={form.cardinality}
								/></label
							>
							{#if form.intervention === 'placement'}<label
									>Added demand (MW)<input
										type="number"
										min="0"
										bind:value={form.increase}
									/></label
								>{/if}
						</div>
						{#if form.intervention === 'redistribution'}<p class="hint compact">
								Total demand stays constant. Moving 1 MW uses 2 MW of the change budget.
							</p>{/if}
						<details>
							<summary
								>Candidate {form.intervention === 'capacity' ? 'lines' : 'buses'}<span
									>{equipment.candidates.length.toLocaleString()}</span
								></summary
							>
							{#if form.scope === 'area' && form.intervention === 'capacity'}<p
									class="hint compact"
								>
									Includes lines connected to this area.
								</p>{/if}
							<label class="search"
								>Find candidate<input
									type="search"
									value={candidateSearch}
									oninput={(event) => {
										candidateSearch = event.currentTarget.value;
										candidatePage = 0;
									}}
									placeholder={form.intervention === 'capacity'
										? 'Line ID or endpoints'
										: 'Bus ID or name'}
								/></label
							>
							{#if form.scope === 'selected'}<button
									class="text-button"
									disabled={form.intervention === 'capacity'
										? ctrl.app.selectedBranch == null
										: ctrl.app.selectedBus == null}
									onclick={() => addMapSelection('candidates')}
									>Add selected map {form.intervention === 'capacity' ? 'line' : 'bus'}</button
								>{/if}
							<div class="table-scroll">
								<table class="equipment-table bounds-table">
									<thead
										><tr
											><th>{form.intervention === 'capacity' ? 'Line' : 'Bus'}</th><th
												>Min ({changeUnit})</th
											><th>Max ({changeUnit})</th></tr
										></thead
									><tbody>
										{#each candidateRows.slice(candidatePage * pageSize, (candidatePage + 1) * pageSize) as element (selectionKey(element))}{@const bounds =
												candidateBounds(element, form)}<tr
												><th>
													{#if form.scope === 'selected'}<label class="check"
															><input
																type="checkbox"
																checked={form.candidates.includes(selectionKey(element))}
																onchange={(event) =>
																	toggleSelection(
																		'candidates',
																		selectionKey(element),
																		event.currentTarget.checked
																	)}
															/><span
																>{'from' in element
																	? branchLabel(element)
																	: busLabel(element)}</span
															></label
														>{:else}{'from' in element
															? branchLabel(element)
															: busLabel(element)}{/if}
												</th><td
													><input
														aria-label={`Minimum change for ${'from' in element ? 'line' : 'bus'} ${element.id}`}
														type="number"
														step={form.increment}
														value={bounds.lower}
														oninput={(event) =>
															updateBound(element, 'lower', event.currentTarget.valueAsNumber)}
													/></td
												><td
													><input
														aria-label={`Maximum change for ${'from' in element ? 'line' : 'bus'} ${element.id}`}
														type="number"
														step={form.increment}
														value={bounds.upper}
														oninput={(event) =>
															updateBound(element, 'upper', event.currentTarget.valueAsNumber)}
													/></td
												></tr
											>{/each}
									</tbody>
								</table>
							</div>
							{#if candidateRows.length > pageSize}<div class="pagination">
									<button disabled={candidatePage === 0} onclick={() => candidatePage--}
										>Previous</button
									><span
										>{candidatePage * pageSize + 1}-{Math.min(
											(candidatePage + 1) * pageSize,
											candidateRows.length
										)} of {candidateRows.length}</span
									><button
										disabled={(candidatePage + 1) * pageSize >= candidateRows.length}
										onclick={() => candidatePage++}>Next</button
									>
								</div>{/if}
						</details>
						<div class="toolbar">
							<button
								class="primary"
								disabled={workspace.busy || !equipment.buses.length || !equipment.candidates.length}
								onclick={() => void attempt(saveGoal)}
								>{goal ? 'Save goal revision' : 'Save goal'}</button
							><button onclick={() => (editingGoal = false)}>Cancel</button>
						</div>
					{/if}
				{:else}
					<h3>{goal.request}</h3>
					<p class="hint">{goal.interpretation}</p>
					<dl class="facts">
						<div>
							<dt>Objective</dt>
							<dd>{objectiveLabel(goal.objective)}</dd>
						</div>
						<div>
							<dt>Candidate elements</dt>
							<dd>{goal.decisions.variables.length.toLocaleString()}</dd>
						</div>
						<div>
							<dt>Maximum changes</dt>
							<dd>{goal.decisions.max_changed_elements}</dd>
						</div>
						<div>
							<dt>Change budget</dt>
							<dd>
								{number(goal.decisions.total_budget)}
								{goal.decisions.variables[0]?.intervention === 'branch_rating' ? ratingUnit : 'MW'}
							</dd>
						</div>
					</dl>
					<GoalDetails {goal} {network} {ratingUnit} />
					<button class="text-button" onclick={startGoal}>Change goal</button>
					<div class="toolbar">
						<label
							>Solve budget<input
								type="number"
								min="1"
								max="256"
								step="1"
								bind:value={solveBudget}
							/></label
						><button
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
										rationale: goal!.request
									})
								)}>Find a proposal</button
						>
					</div>
					<p class="hint compact">Review the result before applying it.</p>
					{#if latestProposal && doc.recommended_state !== doc.applied_state}<button
							disabled={workspace.busy}
							onclick={() => {
								tab = 'case';
							}}>Review recommendation</button
						>{/if}
				{/if}
			{:else}
				<h3>Saved states</h3>
				<ul class="history">
					{#each states as [id, state] (id)}<li style:padding-left="{depth(id) * 10}px">
							<button
								class:selected={id === doc.inspected_state}
								disabled={workspace.busy}
								onclick={() => void attempt(() => operation({ kind: 'inspect', state: id }))}
								><span>{state.label}</span><span class="state-flags"
									>{#if id === doc.recommended_state}<small>Recommended</small
										>{/if}{#if id === doc.applied_state}<small>Applied</small
										>{/if}{#if !state.solution}<small>Not solved</small>{/if}</span
								></button
							><button
								class="text-button"
								disabled={workspace.busy}
								onclick={() =>
									void attempt(() =>
										operation({
											kind: 'branch',
											state: id,
											rationale: `Continue from ${state.label}`
										})
									)}>Branch</button
							>
						</li>{/each}
				</ul>
				{#if Object.keys(doc.states).length > historyLimit}<button
						onclick={() => (historyLimit += 30)}>More states</button
					>{/if}
				<div class="toolbar">
					{#if goal}<label
							>Compare under goal<select bind:value={goalForComparison}
								><option value="">Active goal</option
								>{#each Object.entries(doc.goals) as [id, revision] (id)}<option value={id}
										>{revision.request}</option
									>{/each}</select
							></label
						>{/if}<button
						disabled={workspace.busy || !doc.inspected_state}
						onclick={() => void attempt(compare)}>Compare with starting point</button
					>
				</div>
				{#if workspace.comparison}<div class="comparison">
						<h3>
							{workspace.comparison.improvement != null ? 'Goal progress' : 'Network changes'}
						</h3>
						{#if workspace.comparison.improvement != null}<p>
								{number(workspace.comparison.left_value)} to {number(
									workspace.comparison.right_value
								)}, improvement {number(workspace.comparison.improvement)}
							</p>{/if}
						{#if consequences.length}<p class="hint compact">
								Largest {workspace.comparison.right_view?.lmp ? 'LMP' : 'voltage magnitude'} changes.
							</p>
							<table>
								<thead><tr><th>Bus</th><th>Before</th><th>After</th><th>Change</th></tr></thead
								><tbody
									>{#each consequences as row (row.bus)}<tr
											><th>{row.bus}</th><td>{number(row.before)}</td><td>{number(row.after)}</td
											><td>{number(row.change)}</td></tr
										>{/each}</tbody
								>
							</table>{/if}
					</div>{/if}
				<h3>Activity</h3>
				{#if !doc.experiment_order.length}<p class="hint">
						Edits, solves and observations appear here.
					</p>{/if}
				{#each doc.experiment_order.toReversed().slice(0, historyLimit) as id (id)}{@const event =
						doc.experiments[id]}
					<details>
						<summary
							>{activityLabel(event.kind)}<span>{event.termination.replaceAll('_', ' ')}</span
							></summary
						>
						<p>{event.rationale}</p>
						<p class="hint compact">{event.solve_count} solves, {event.trials.length} trials</p>
						{#each event.trials as trial, index (index)}<p class="trial">
								Trial {index + 1}: {trial.failure ??
									(trial.accepted ? 'Retained' : 'Rejected')}{#if trial.exact_value != null},
									objective {number(trial.exact_value)}{/if}
							</p>{/each}
						{#each event.evidence as ref (ref)}<details>
								<summary>Result details</summary>
								<pre>{workspace.bundle?.artifacts[ref]?.text}</pre>
							</details>{/each}
					</details>{/each}
			{/if}
		</section>
		{#if doc && !multi}<footer class="workspace-status">
				<span
					>{Object.keys(doc.states).length} saved {Object.keys(doc.states).length === 1
						? 'state'
						: 'states'}</span
				>{#if workspace.busy}<button onclick={() => workspace.cancel()}>Cancel</button>{:else}<span
						>Saved</span
					>{/if}
			</footer>{/if}
	</div>
</PanelFrame>

<style>
	.study-workspace {
		display: flex;
		flex-direction: column;
		min-height: 0;
		height: 100%;
		color: var(--ink);
		font: 13px/1.45 var(--font-display);
	}
	.storage {
		display: flex;
		align-items: flex-end;
		gap: 8px;
		padding: 10px 16px;
		flex: none;
	}
	.storage label:first-child {
		flex: 1;
		min-width: 0;
	}
	.storage label {
		margin: 0;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
		border: 0;
	}
	.storage > button {
		padding: 8px;
	}
	.file-button {
		position: relative;
		flex: none;
		overflow: hidden;
		padding: 8px;
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
	.workspace-content {
		padding: 18px 16px;
		overflow: auto;
		min-height: 0;
		overscroll-behavior: contain;
	}
	h3 {
		margin: 0 0 14px;
		font-size: 15px;
		font-weight: 600;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin: 0 0 16px;
		min-width: 0;
		font-size: 12px;
	}
	input,
	select,
	button {
		font: inherit;
		color: inherit;
	}
	input,
	select {
		min-width: 0;
		width: 100%;
		box-sizing: border-box;
		background: white;
		border: 1px solid var(--line);
		padding: 8px 9px;
		border-radius: 4px;
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
	button.text-button {
		border: 0;
		padding: 6px 0;
		color: var(--text-secondary);
		font-size: 12px;
	}
	button:focus-visible,
	input:focus-visible,
	select:focus-visible,
	summary:focus-visible,
	.file-button:focus-within {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
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
		margin: 0;
	}
	.toolbar label:has(input[type='number']) {
		max-width: 132px;
	}
	.form-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0 16px;
	}
	.hint {
		margin: 0 0 16px;
		color: var(--text-secondary);
		font-size: 12px;
		line-height: 1.5;
	}
	.hint.compact {
		margin: 10px 0;
		font-size: 11px;
	}
	.case-name {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		margin: 0 0 20px;
	}
	.case-name span,
	dt {
		color: var(--text-secondary);
	}
	.facts {
		display: grid;
		gap: 8px;
		margin: 0 0 18px;
	}
	.facts > div {
		display: grid;
		grid-template-columns: 105px minmax(0, 1fr);
		gap: 12px;
	}
	.facts dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.recommendation {
		display: grid;
		gap: 10px;
		margin: 16px 0;
	}
	details {
		border-top: 1px solid var(--line);
		padding: 12px 0;
	}
	details > p {
		overflow-wrap: anywhere;
	}
	summary {
		cursor: pointer;
		font-size: 12px;
		font-weight: 500;
	}
	summary span {
		float: right;
		max-width: 50%;
		text-align: right;
		color: var(--text-secondary);
		font-size: 11px;
		font-weight: 400;
	}
	details[open] > summary {
		margin-bottom: 14px;
	}
	.search {
		margin: 12px 0;
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 7px;
		margin: 0;
		font-weight: 400;
	}
	.check input {
		width: 15px;
		height: 15px;
		flex: none;
		padding: 0;
		accent-color: var(--accent);
	}
	.table-scroll {
		max-height: 255px;
		overflow: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 11px;
	}
	th,
	td {
		padding: 8px 5px;
		border-bottom: 1px solid var(--line);
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	th:first-child {
		text-align: left;
	}
	tbody th {
		font-weight: 400;
	}
	thead th {
		position: sticky;
		top: 0;
		background: var(--paper);
		white-space: nowrap;
		font-weight: 500;
	}
	.equipment-table th:first-child {
		min-width: 90px;
	}
	.bounds-table input {
		padding: 5px 4px;
		width: 72px;
	}
	.pagination {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		margin: 10px 0;
		font-size: 11px;
		color: var(--text-secondary);
	}
	.pagination button {
		padding: 4px 6px;
	}
	.history {
		list-style: none;
		padding: 0;
		margin: 0 0 16px;
	}
	.history li {
		display: flex;
		gap: 8px;
		margin: 5px 0;
	}
	.history li > button:first-child {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		text-align: left;
	}
	.history button.selected {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.state-flags {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 3px;
	}
	.state-flags small {
		font-size: 10px;
		color: var(--text-secondary);
	}
	.comparison {
		margin: 18px 0;
	}
	.trial {
		font-size: 11px;
	}
	pre {
		overflow: auto;
		max-height: 240px;
		font: 10px/1.5 var(--font-mono);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.error {
		color: var(--negative, #a62922);
		background: var(--paper);
		border-left: 2px solid currentColor;
		padding: 8px 12px;
	}
	.workspace-status {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 16px;
		border-top: 1px solid var(--line);
		flex: none;
		font-size: 10px;
		color: var(--text-secondary);
	}
	.workspace-status button {
		padding: 3px 6px;
	}
</style>
