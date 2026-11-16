<script lang="ts">
	import { tick, untrack, type Snippet } from 'svelte';
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import type { LocalCase } from '../state.svelte.js';
	import PanelFrame from './PanelFrame.svelte';
	import SavedNetworkDetails from './SavedNetworkDetails.svelte';
	import ModelDetails from './ModelDetails.svelte';
	import { getPanelLayout } from '../panels.svelte.js';
	import BusPicker from './BusPicker.svelte';
	import DemandSlider from './DemandSlider.svelte';
	import DisplayControls from './DisplayControls.svelte';
	import FormulationSelector from './FormulationSelector.svelte';
	import LocalCaseDetails from './LocalCaseDetails.svelte';
	import MulticonductorDetails from './MulticonductorDetails.svelte';
	import NetworkStats from './NetworkStats.svelte';
	import PowerFlowDetails from './PowerFlowDetails.svelte';
	import DynamicsElementActions from './dynamics/DynamicsElementActions.svelte';
	import RatingSlider from './RatingSlider.svelte';
	import SensitivityReadout from './SensitivityReadout.svelte';
	import SizeLegend from './SizeLegend.svelte';
	import SolveCard from './SolveCard.svelte';
	import TopMovers from './TopMovers.svelte';

	/** Optional host content rendered under a local case's stats. */
	let { caseNote }: { caseNote?: Snippet<[LocalCase]> } = $props();

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();

	const layout = getPanelLayout();
	let open = $state(true);
	let bodyEl = $state.raw<HTMLElement>();

	/** A selection whose readout has resolved (or is resolving). */
	const busReadout = $derived(
		app.selectedBus !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)
	);
	const branchReadout = $derived(
		app.selectedBranch !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)
	);
	/** A resolved selection comes right after the result sentence, so it opens in
	 * view instead of at the foot of a long panel. Gated on networkStats because
	 * that is what renders the readout. */
	const selectionLeads = $derived(
		!!ctrl.networkStats &&
			(busReadout ||
				branchReadout ||
				(ctrl.activeFormulation === 'acpf' &&
					(app.selectedBus !== null || app.selectedBranch !== null)))
	);

	/** The diagram has its own colour tabs and key: the map's colour switch would only
	 * repeat them, so it steps aside while the diagram is shown. */
	const mapKeyOnly = $derived(
		!!ctrl.singleLineShown &&
			!(app.mode === 'dynamics' && app.activeLocal) &&
			!busReadout &&
			!branchReadout
	);
	const blockEmpty = $derived(mapKeyOnly && ctrl.activeFormulation !== 'acpf');

	/** The case list failed to load: a case file still opens. */
	function openFile() {
		const input = document.createElement('input');
		input.type = 'file';
		input.multiple = true;
		input.onchange = () => {
			if (input.files) void ctrl.ingestFiles(input.files);
		};
		input.click();
	}

	$effect(() => {
		const selection = app.selectedBus ?? app.selectedBranch;
		if (selection !== null)
			untrack(() => {
				open = true;
				// A selection's readout is in this panel: bring it to the front of its
				// dock, also when an agent made the selection, so the reader sees what
				// the agent pointed at. The Agent tab keeps its record of the call.
				const panel = layout.panels.find((candidate) => candidate.id === 'network');
				if (panel) layout.activate(panel);
				if (layout.compact) layout.drawer = 'network';
				// The readout sits near the top: scroll the panel back so it is in view.
				void tick().then(() => bodyEl?.closest('.panel-content')?.scrollTo({ top: 0 }));
			});
	});
</script>

<PanelFrame id="network" title="Network" side="left" order={10} width={320} bind:open>
	<div class="panel-body" id="control-panel-body" bind:this={bodyEl}>
		{#snippet selectionBlock()}
			{#if app.mode === 'dynamics' && app.activeLocal}
				<DynamicsElementActions />
			{:else if ctrl.activeFormulation === 'acpf'}
				{#if !mapKeyOnly}<DisplayControls />{/if}
				<PowerFlowDetails />
			{:else if busReadout}
				<SensitivityReadout />

				<DemandSlider />

				{#if ctrl.showMoverSlot}
					<TopMovers />
				{/if}
			{:else if branchReadout}
				<SensitivityReadout />

				<RatingSlider />

				{#if ctrl.showMoverSlot}
					<TopMovers />
				{/if}
			{:else if !mapKeyOnly}
				<DisplayControls />
			{/if}
		{/snippet}

		{#if app.studyView}
			<SavedNetworkDetails />
			<SolveCard embedded />
		{:else}
			{#if app.parsingFile}
				<p class="dim blink">Parsing&hellip;</p>
			{/if}

			{#snippet lead()}
				{#if selectionLeads}
					<div class="selection-lead">{@render selectionBlock()}</div>
				{/if}
			{/snippet}

			{#if app.activeLocal}
				{#if app.activeLocal.diagram && app.activeLocal.view?.buses.length}
					<div class="view-mode" aria-label="Coordinate display">
						<button
							aria-pressed={app.activeLocal.displayMode === 'geographic'}
							onclick={() => ctrl.setCaseDisplayMode(app.activeLocal!, 'geographic')}>Map</button
						>
						<button
							aria-pressed={app.activeLocal.displayMode === 'diagram'}
							onclick={() => ctrl.setCaseDisplayMode(app.activeLocal!, 'diagram')}>Diagram</button
						>
					</div>
				{/if}
				<LocalCaseDetails note={caseNote} {lead} />
			{/if}
			{#each app.hostedDistributionCases.filter((c) => c.distribution?.related_case_id === app.activeCaseId) as related (related.id)}
				<button
					class="reset mono"
					disabled={!!related.unavailable_reason || ctrl.loadingHostedDistribution === related.id}
					title={related.unavailable_reason ?? related.name}
					onclick={() => ctrl.activateHostedDistribution(related.id)}
				>
					{ctrl.loadingHostedDistribution === related.id ? 'Loading…' : 'Open distribution'}
				</button>
			{/each}
			{#if app.activeMulti}
				<MulticonductorDetails />
			{/if}
			{#if !ctrl.networkStats}
				{#if !app.activeLocal && !app.activeMulti}
					{#if ctrl.loadingHostedDistribution}
						<p class="dim blink">Loading&hellip;</p>
					{:else if ctrl.casesLoaded && app.cases.length === 0}
						<p class="dim">
							{app.hostedDistributionCases.length
								? 'No case'
								: config.loadDefaultCases
									? 'No cases'
									: 'Drop a case file'}
						</p>
						{#if config.loadDefaultCases}
							<button class="reset" onclick={ctrl.restoreDefaultCases}>Restore default cases</button
							>
						{/if}
					{:else if ctrl.casesFailed}
						<p class="load-failed" role="alert">Could not load the case list</p>
						<div class="load-actions">
							<button class="reset" onclick={() => void ctrl.load()}>Retry</button>
							<button class="reset" onclick={openFile}>Open case file</button>
						</div>
					{:else}
						<p class="dim blink">Loading&hellip;</p>
					{/if}
				{/if}
			{:else}
				{#if !app.activeLocal}
					<NetworkStats {lead} />
				{/if}

				<!-- On a phone the sheet is short: the key to the map's colours comes
				     right after the result, not below the fold. -->
				{#if app.compactLayout && !selectionLeads && !blockEmpty}
					<div class="compact-key">{@render selectionBlock()}</div>
				{/if}

				{#if ctrl.activeSolvable?.network && !app.activeMulti}
					{@const c = ctrl.activeSolvable}
					<button
						class="reset diagram-toggle"
						aria-pressed={c.schematic}
						onclick={() => {
							c.schematic = !c.schematic;
							// On a phone, the sheet would cover the diagram it opens.
							if (c.schematic && layout.compact) layout.drawer = null;
						}}>{c.schematic ? 'Show map' : 'Show diagram'}</button
					>
				{/if}

				{#if ctrl.activeSolvable}
					<FormulationSelector />
				{/if}
				<!-- The steady-state solve keeps its Details in both modes, so the panel does not jump. -->
				<SolveCard embedded />

				<ModelDetails details={ctrl.activeSolvable?.network?.model_details} />
				<BusPicker inline />

				<!-- Each section brings its own rule, and an empty one shows nothing, rule included. -->
				{#if !selectionLeads && !app.compactLayout && !blockEmpty}
					<div class="section">{@render selectionBlock()}</div>
				{/if}

				<!-- The diagram does not size buses by power, so the map's size key would mislead. -->
				{#if app.busSizing === 'power' && !ctrl.singleLineShown}
					<div class="section"><SizeLegend /></div>
				{/if}
			{/if}
		{/if}
	</div>
</PanelFrame>

<style>
	.panel-body {
		padding: 16px 18px;
	}

	/* The selection readout, set off from the result sentence above it. */
	.selection-lead {
		margin: 12px 0;
		padding: 12px 0;
		border-block: 1px solid var(--line);
	}

	/* The display key under the case stats on a phone: a gap, not touching. */
	.compact-key {
		margin-top: 12px;
	}
	.compact-key:not(:has(> *)) {
		display: none;
	}

	.section {
		margin-top: 12px;
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}
	.section:not(:has(> *)) {
		display: none;
	}

	.load-failed {
		margin: 0 0 8px;
		font-weight: 600;
		color: var(--text-danger);
	}
	.load-actions {
		display: flex;
		gap: 8px;
	}

	.view-mode {
		display: flex;
		gap: 4px;
		margin-bottom: 12px;
	}
	.view-mode button {
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		padding: 5px 10px;
		font: 12px/1.4 var(--font-display);
		color: var(--ink);
		background: var(--paper);
		cursor: pointer;
	}
	.view-mode button[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.view-mode button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
</style>
