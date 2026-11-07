<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import AppFooter from './components/AppFooter.svelte';
	import NotificationCenter from './components/NotificationCenter.svelte';
	import AppHeader from './components/AppHeader.svelte';
	import ControlPanel from './components/ControlPanel.svelte';
	import DropZone from './components/DropZone.svelte';
	import PlacementCue from './components/PlacementCue.svelte';
	import RestoreDefaultsButton from './components/RestoreDefaultsButton.svelte';
	import SolveCard from './components/SolveCard.svelte';
	import PanelHost from './components/PanelHost.svelte';
	import LogPanel from './components/LogPanel.svelte';
	import ShellEnvironment from './components/ShellEnvironment.svelte';
	import { getAppState, getController, getUiConfig } from './context.svelte.js';
	import TellegenMap from './TellegenMap.svelte';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();

	let dragDepth = 0;

	// The overview and the diagram read the active case's whole operating point once per solve.
	$effect(() => {
		const c = app.activeMulti;
		void c?.result?.solve_count;
		void c?.mcFullResult;
		return untrack(() => ctrl.followOverview(c));
	});

	// ShellEnvironment drops the affordance when no fine pointer is attached;
	// forget any drag that was in flight so the next one counts from zero.
	$effect(() => {
		if (!ctrl.showFileDropUi) dragDepth = 0;
	});

	$effect(() => {
		if (app.studyView) return;
		let hasDisplayMode = false;
		for (const option of ctrl.displayOptions) {
			if (option.mode === app.displayMode) {
				hasDisplayMode = true;
				break;
			}
		}
		if (ctrl.displayOptions.length > 0 && !hasDisplayMode) {
			app.displayMode = 'price';
		}
	});

	onMount(() => {
		ctrl.probeAcOpf();
		// The provider already marked the shell loaded when default cases are
		// off, and load() itself is gated; this only frames whatever is there.
		if (config.loadDefaultCases) {
			if (!ctrl.casesLoaded) void ctrl.load();
		} else {
			void app.requestFrame('all');
		}
	});

	function dragHasFiles(e: DragEvent): boolean {
		return ctrl.showFileDropUi && (e.dataTransfer?.types.includes('Files') ?? false);
	}

	function onDragEnter(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth++;
		app.dragOver = true;
	}

	function onDragLeave(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		dragDepth = Math.max(0, dragDepth - 1);
		if (dragDepth === 0) app.dragOver = false;
	}

	function onDragOver(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
	}

	function onDrop(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth = 0;
		app.dragOver = false;
		if (e.dataTransfer) ctrl.ingestFiles(e.dataTransfer.files);
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') ctrl.clearSelection();
	}}
	ondragenter={onDragEnter}
	ondragleave={onDragLeave}
	ondragover={onDragOver}
	ondrop={onDrop}
/>

<ShellEnvironment />

<main>
	<TellegenMap
		onbusclick={ctrl.selectBus}
		onlocalbusclick={ctrl.selectLocalBus}
		onbranchclick={ctrl.selectBranch}
		onlocalbranchclick={ctrl.selectLocalBranch}
		onmultibusclick={ctrl.selectMultiBus}
		onmultiedgeclick={ctrl.selectMultiEdge}
		onplacecase={ctrl.placeCase}
		onmapclick={ctrl.clearSelection}
		theme={config.theme}
		cartoApiKey={config.cartoApiKey || undefined}
	/>

	<AppHeader />
	<PanelHost />
	<NotificationCenter />
	<ControlPanel />
	<SolveCard />
	<LogPanel />
	{#if !app.studyView}
		<DropZone />
		<PlacementCue />
		<RestoreDefaultsButton />
	{/if}
	{#if config.showFooter && !app.compactLayout}
		<AppFooter />
	{/if}
</main>

<style>
	main {
		position: fixed;
		inset: 0;
		overflow: hidden;
	}
</style>
