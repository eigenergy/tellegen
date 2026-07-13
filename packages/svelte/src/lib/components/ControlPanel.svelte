<script lang="ts">
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import BindingLines from './BindingLines.svelte';
	import DemandSlider from './DemandSlider.svelte';
	import DisplayControls from './DisplayControls.svelte';
	import FormulationSelector from './FormulationSelector.svelte';
	import LocalCaseDetails from './LocalCaseDetails.svelte';
	import MulticonductorDetails from './MulticonductorDetails.svelte';
	import NetworkStats from './NetworkStats.svelte';
	import RatingSlider from './RatingSlider.svelte';
	import SensitivityReadout from './SensitivityReadout.svelte';
	import SizeLegend from './SizeLegend.svelte';
	import TopMovers from './TopMovers.svelte';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();
</script>

<aside class="panel">
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
	{#if app.activeLocal}
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

		{#if !app.placingLocalId}
			<BindingLines />
		{/if}

		<hr />

		{#if app.selectedBus !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)}
			<SensitivityReadout />

			<DemandSlider />

			{#if ctrl.showMoverSlot}
				<TopMovers />
			{/if}
		{:else if app.selectedBranch !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)}
			<SensitivityReadout />

			<RatingSlider />

			{#if ctrl.showMoverSlot}
				<TopMovers />
			{/if}
		{:else}
			<DisplayControls />
		{/if}

		<hr />

		<SizeLegend />
	{/if}
</aside>

<style>
	.panel {
		position: absolute;
		top: 76px;
		left: 20px;
		z-index: 10;
		width: 312px;
		max-height: calc(100% - 122px);
		overflow-y: auto;
		padding: 16px 18px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.5s 0.12s ease-out both;
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

	.error-actions {
		display: flex;
		gap: 6px;
	}

	@media (max-width: 760px) {
		.panel {
			top: auto;
			left: 10px;
			right: 10px;
			bottom: 40px;
			width: auto;
			max-height: 44dvh;
			padding: 14px 16px;
			background: var(--paper);
		}
	}

	@media (max-width: 420px) {
		.panel {
			bottom: 34px;
			max-height: 46dvh;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.panel {
			animation: none;
		}
	}
</style>
