<script lang="ts">
	import { getAppState, getController } from '$lib/context.svelte';
	import DemandSlider from './DemandSlider.svelte';
	import DisplayControls from './DisplayControls.svelte';
	import FormulationSelector from './FormulationSelector.svelte';
	import LocalCaseDetails from './LocalCaseDetails.svelte';
	import NetworkStats from './NetworkStats.svelte';
	import SensitivityReadout from './SensitivityReadout.svelte';
	import SizeLegend from './SizeLegend.svelte';
	import TopMovers from './TopMovers.svelte';

	const app = getAppState();
	const ctrl = getController();
</script>

<aside class="panel">
	{#if app.error}
		<p class="error mono">{app.error}</p>
		{#if !ctrl.casesLoaded}
			<button class="reset mono" onclick={ctrl.load}>retry</button>
		{/if}
	{/if}
	{#if app.parsingFile}
		<p class="dim mono blink">parsing&hellip;</p>
	{/if}
	{#if app.activeLocal}
		<LocalCaseDetails />
	{/if}
	{#if !ctrl.networkStats}
		{#if !app.error && !app.activeLocal}
			{#if ctrl.casesLoaded && app.cases.length === 0}
				<p class="dim mono">no default cases loaded</p>
				<button class="reset mono" onclick={ctrl.restoreDefaultCases}>restore defaults</button>
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

		<hr />

		{#if app.selectedBus !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)}
			<SensitivityReadout />

			<DemandSlider />

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
		top: 64px;
		left: 20px;
		z-index: 10;
		width: 312px;
		max-height: calc(100% - 110px);
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

	@media (max-width: 760px) {
		.panel {
			top: auto;
			left: 10px;
			right: 10px;
			bottom: 40px;
			width: auto;
			max-height: 44dvh;
			padding: 14px 16px;
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
