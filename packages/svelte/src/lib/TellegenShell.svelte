<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import AppFooter from './components/AppFooter.svelte';
	import NotificationCenter from './components/NotificationCenter.svelte';
	import CommandPalette from './components/CommandPalette.svelte';
	import AppHeader from './components/AppHeader.svelte';
	import ControlPanel from './components/ControlPanel.svelte';
	import DropZone from './components/DropZone.svelte';
	import CollectionChooser from './components/CollectionChooser.svelte';
	import PlacementCue from './components/PlacementCue.svelte';
	import RestoreDefaultsButton from './components/RestoreDefaultsButton.svelte';
	import SolveCard from './components/SolveCard.svelte';
	import PanelHost from './components/PanelHost.svelte';
	import LogPanel from './components/LogPanel.svelte';
	import SettingsPanel from './components/SettingsPanel.svelte';
	import ShellEnvironment from './components/ShellEnvironment.svelte';
	import { getAppState, getController, getUiConfig } from './context.svelte.js';
	import TellegenMap from './TellegenMap.svelte';
	import DynamicsPanel from './components/dynamics/DynamicsPanel.svelte';
	import DynamicsTimeline from './components/dynamics/DynamicsTimeline.svelte';
	import DynamicsLegend from './components/dynamics/DynamicsLegend.svelte';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();

	let dragDepth = 0;
	/** Height kept free above the dock for the map's control row (the zoom
	 * buttons stack two high on desktop). */
	const MAP_CONTROL_ROW = 64;
	const dyn = ctrl.dynamics;
	let timelineHeight = $state(0);
	const dynamicsShown = $derived(
		app.mode === 'dynamics' && !!dyn.current?.run && dyn.current.localId === app.activeLocal?.id
	);
	// The legend explains the map's dynamics layers: during playback, and at
	// rest before the first run. The single-line diagram draws neither.
	const legendShown = $derived(
		app.mode === 'dynamics' && !!dyn.frame && !app.zen && !ctrl.singleLineShown
	);
	$effect(() => {
		dyn.theme = config.theme;
	});
	// What the playback dock covers, from the bottom edge (it sits 12 px up,
	// 40 px on the compact layout).
	$effect(() => {
		app.dockInset =
			dynamicsShown && timelineHeight > 0 ? timelineHeight + (app.compactLayout ? 40 : 12) : 0;
	});
	// Chrome anchored at the bottom (toasts, map controls, the host app's own
	// notices outside this shell) clears the dock through this variable.
	$effect(() => {
		const root = document.documentElement.style;
		if (app.dockInset > 0) root.setProperty('--tellegen-timeline-clear', `${app.dockInset + 8}px`);
		else root.removeProperty('--tellegen-timeline-clear');
		return () => root.removeProperty('--tellegen-timeline-clear');
	});
	// When the dock opens, frame the case in the map above it: the event that
	// was just simulated must not sit behind the traces.
	let dockFramed = false;
	$effect(() => {
		const open = app.dockInset > 0;
		if (open && !dockFramed) {
			const id = untrack(() => app.activeLocal?.id);
			if (id) void app.requestFrame(id);
		}
		dockFramed = open;
	});

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
		if (e.dataTransfer) ctrl.ingestDrop(e.dataTransfer);
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape' && !e.defaultPrevented) ctrl.clearSelection();
	}}
	ondragenter={onDragEnter}
	ondragleave={onDragLeave}
	ondragover={onDragOver}
	ondrop={onDrop}
/>

<ShellEnvironment />
<CollectionChooser />

<!-- The side panels end above the map's control row (zoom, attribution,
     diagram), which sits on the dock while it is open. -->
<main
	style:--tellegen-dock-bottom={dynamicsShown && !app.compactLayout
		? `${app.dockInset + 8 + MAP_CONTROL_ROW}px`
		: undefined}
>
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
		dynamicsLayers={dyn.mapLayers}
		dynamicsTooltip={dyn.tooltip}
	/>

	<AppHeader />
	<PanelHost />
	<NotificationCenter />
	<CommandPalette />
	<ControlPanel />
	{#if app.mode !== 'dynamics'}<SolveCard />{/if}
	{#if !app.studyView}<DynamicsPanel />{/if}
	{#if dynamicsShown}
		<DynamicsTimeline bind:dockHeight={timelineHeight} />
	{/if}
	{#if legendShown}<DynamicsLegend
			theme={config.theme}
			top={app.headerInset + 52}
			atRest={dyn.atRest}
		/>{/if}
	<LogPanel />
	<SettingsPanel />
	{#if !app.studyView}
		<DropZone />
		<PlacementCue />
		{#if !app.zen}<RestoreDefaultsButton />{/if}
	{/if}
	{#if config.showFooter && !app.compactLayout && !app.zen && !dynamicsShown}
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
