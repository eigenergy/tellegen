<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import NotificationCenter from './components/NotificationCenter.svelte';
	import CommandPalette from './components/CommandPalette.svelte';
	import AppHeader from './components/AppHeader.svelte';
	import ControlPanel from './components/ControlPanel.svelte';
	import DropZone from './components/DropZone.svelte';
	import CollectionChooser from './components/CollectionChooser.svelte';
	import PlacementCue from './components/PlacementCue.svelte';
	import PanelHost from './components/PanelHost.svelte';
	import LogPanel from './components/LogPanel.svelte';
	import SettingsPanel from './components/SettingsPanel.svelte';
	import ShellEnvironment from './components/ShellEnvironment.svelte';
	import { getAppState, getController, getUiConfig } from './context.svelte.js';
	import TellegenMap from './TellegenMap.svelte';
	import DynamicsPanel from './components/dynamics/DynamicsPanel.svelte';
	import DynamicsTimeline from './components/dynamics/DynamicsTimeline.svelte';
	import DynamicsUnitPopup from './components/dynamics/DynamicsUnitPopup.svelte';
	import DynamicsWorkbench from './components/dynamics/DynamicsWorkbench.svelte';
	import { getPanelLayout } from './panels.svelte.js';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();
	const layout = getPanelLayout();

	let dragDepth = 0;
	/** Height kept free above the dock for the map's control row (the zoom
	 * buttons stack two high on desktop). */
	const MAP_CONTROL_ROW = 64;
	const dyn = ctrl.dynamics;
	let timelineHeight = $state(0);
	/** The live workbench replaces the playback dock while it is open. */
	const workbenchShown = $derived(
		app.mode === 'dynamics' && app.dynamicsEnabled && dyn.live.open && !!dyn.active
	);
	const dynamicsShown = $derived(
		workbenchShown ||
			(app.mode === 'dynamics' &&
				!!dyn.current?.run &&
				dyn.current.localId === app.activeLocal?.id)
	);
	$effect(() => {
		dyn.theme = config.theme;
	});
	// Turning experimental features off stops any dynamics playback.
	$effect(() => {
		if (!app.dynamicsEnabled) untrack(() => dyn.player.pause());
	});
	/** Phone with a panel open: the playback bar sits flush on the bottom edge and the
	 * sheet stands on top of it, so the bar never covers the sheet's content. */
	const sheetOpen = $derived(app.compactLayout && !!layout.drawer && !app.zen);
	// What the playback dock covers, from the bottom edge (it sits 12 px up,
	// 40 px on the compact layout, flush under an open sheet).
	$effect(() => {
		const gap = app.compactLayout ? (sheetOpen ? 0 : 40) : 12;
		app.dockInset = dynamicsShown && timelineHeight > 0 ? timelineHeight + gap : 0;
	});
	// Chrome anchored at the bottom (toasts, map controls, the host app's own
	// notices outside this shell) clears the dock through this variable.
	$effect(() => {
		const root = config.element?.style;
		if (!root) return;
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
	{#if !app.studyView && app.dynamicsEnabled}<DynamicsPanel />{/if}
	{#if workbenchShown}
		<DynamicsWorkbench bind:dockHeight={timelineHeight} />
	{:else if dynamicsShown}
		<DynamicsTimeline bind:dockHeight={timelineHeight} flush={sheetOpen} />
	{/if}
	{#if app.mode === 'dynamics' && app.dynamicsEnabled}<DynamicsUnitPopup />{/if}
	<LogPanel />
	<SettingsPanel />
	{#if !app.studyView}
		<DropZone />
		<PlacementCue />
	{/if}
</main>

<style>
	main {
		position: fixed;
		inset: 0;
		overflow: hidden;
	}
</style>
