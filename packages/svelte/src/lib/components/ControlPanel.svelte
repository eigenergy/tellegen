<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import PanelFrame from './PanelFrame.svelte';
	import SavedNetworkDetails from './SavedNetworkDetails.svelte';
	import ModelDetails from './ModelDetails.svelte';
	import { getPanelLayout } from '../panels.svelte.js';
	import AppFooter from './AppFooter.svelte';
	import BindingLines from './BindingLines.svelte';
	import BusPicker from './BusPicker.svelte';
	import DemandSlider from './DemandSlider.svelte';
	import DisplayControls from './DisplayControls.svelte';
	import FormulationSelector from './FormulationSelector.svelte';
	import LocalCaseDetails from './LocalCaseDetails.svelte';
	import MulticonductorDetails from './MulticonductorDetails.svelte';
	import NetworkStats from './NetworkStats.svelte';
	import PowerFlowDetails from './PowerFlowDetails.svelte';
	import RatingSlider from './RatingSlider.svelte';
	import SensitivityReadout from './SensitivityReadout.svelte';
	import SizeLegend from './SizeLegend.svelte';
	import TopMovers from './TopMovers.svelte';

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
	/** Compact only: a resolved selection outranks the case stats it was read
	 * against. Gated on networkStats because that is what renders the readout. */
	const selectionLeads = $derived(
		app.compactLayout &&
			!!ctrl.networkStats &&
			(busReadout ||
				branchReadout ||
				(ctrl.activeFormulation === 'acpf' &&
					(app.selectedBus !== null || app.selectedBranch !== null)))
	);

	$effect(() => {
		const selection = app.selectedBus ?? app.selectedBranch;
		if (selection !== null)
			untrack(() => {
				open = true;
				if (layout.compact) {
					layout.drawer = 'network';
					void tick().then(() => bodyEl?.closest('.panel-content')?.scrollTo({ top: 0 }));
				}
			});
	});
</script>

<PanelFrame id="network" title="Network" side="left" order={10} width={320} bind:open>
	<div class="panel-body" id="control-panel-body" bind:this={bodyEl}>
		{#snippet selectionBlock()}
			{#if ctrl.activeFormulation === 'acpf'}
				<DisplayControls />
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
			{:else}
				<DisplayControls />
			{/if}
		{/snippet}

		{#if app.studyView}
			<SavedNetworkDetails />
		{:else}
			{#if app.error}
				<p class="error mono">{app.error}</p>
				<div class="error-actions">
					<button class="reset mono" onclick={ctrl.retryError}>retry</button>
					<button class="reset mono" onclick={() => (app.error = null)}>dismiss</button>
				</div>
			{/if}
			{#if app.parsingFile}
				<p class="dim mono blink">parsing&hellip;</p>
			{/if}

			<!-- Selected equipment leads on narrow screens. -->
			{#if selectionLeads}
				{@render selectionBlock()}

				<hr />
			{/if}

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
				<LocalCaseDetails />
			{/if}
			{#if app.activeMulti}
				<MulticonductorDetails />
			{/if}
			{#if !ctrl.networkStats}
				{#if !app.error && !app.activeLocal && !app.activeMulti}
					{#if ctrl.casesLoaded && app.cases.length === 0}
						<p class="dim mono">
							{config.loadDefaultCases ? 'no default cases loaded' : 'drop a case file to begin'}
						</p>
						{#if config.loadDefaultCases}
							<button class="reset mono" onclick={ctrl.restoreDefaultCases}
								>restore default cases</button
							>
						{/if}
					{:else if ctrl.loadingBackendCase}
						<p class="dim mono blink">loading selected case&hellip;</p>
					{:else}
						<p class="dim mono blink">loading cases&hellip;</p>
					{/if}
				{/if}
			{:else}
				{#if !app.activeLocal}
					<NetworkStats />
				{/if}

				{#if ctrl.activeSolvable}
					<FormulationSelector />
				{/if}

				<ModelDetails details={ctrl.activeSolvable?.network?.model_details} />
				<BusPicker inline />

				{#if !app.placingLocalId && ctrl.activeFormulation !== 'acpf'}
					<BindingLines />
				{/if}

				{#if !selectionLeads}
					<hr />

					{@render selectionBlock()}
				{/if}

				<hr />

				<SizeLegend />
			{/if}
		{/if}
		{#if app.compactLayout && config.showFooter}
			<AppFooter inline />
		{/if}
	</div>
</PanelFrame>

<style>
	.panel-body {
		padding: 16px 18px;
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}

	.error {
		color: var(--red);
		font-size: 12px;
	}

	.view-mode {
		display: flex;
		gap: 4px;
		margin-bottom: 12px;
	}
	.view-mode button {
		border: 1px solid var(--line);
		border-radius: 4px;
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
	.error-actions {
		display: flex;
		gap: 6px;
	}
</style>
