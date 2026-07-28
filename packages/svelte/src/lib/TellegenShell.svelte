<script lang="ts">
	import { onMount } from 'svelte';
	import AppFooter from './components/AppFooter.svelte';
	import AppHeader from './components/AppHeader.svelte';
	import BusPicker from './components/BusPicker.svelte';
	import ControlPanel from './components/ControlPanel.svelte';
	import DropZone from './components/DropZone.svelte';
	import PlacementCue from './components/PlacementCue.svelte';
	import RestoreDefaultsButton from './components/RestoreDefaultsButton.svelte';
	import SolveCard from './components/SolveCard.svelte';
	import { getAppState, getController, getUiConfig } from './context.svelte.js';
	import TellegenMap from './TellegenMap.svelte';

	const FILE_DROP_QUERY = '(hover: hover) and (pointer: fine) and (min-width: 761px)';
	const COMPACT_QUERY = '(max-width: 760px)';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();

	let dragDepth = 0;

	$effect(() => {
		let hasDisplayMode = false;
		for (const option of ctrl.displayOptions) {
			if (option.mode === app.displayMode) {
				hasDisplayMode = true;
				break;
			}
		}
		if (ctrl.displayOptions.length > 0 && !hasDisplayMode) {
			app.displayMode = 'lmp';
		}
	});

	onMount(() => {
		if (config.loadDefaultCases) {
			if (!ctrl.casesLoaded) void ctrl.load();
		} else {
			ctrl.casesLoaded = true;
			app.requestFrame('all');
		}

		const query = window.matchMedia(FILE_DROP_QUERY);
		const syncFileDropUi = () => {
			ctrl.showFileDropUi = query.matches;
			if (!ctrl.showFileDropUi) {
				dragDepth = 0;
				app.dragOver = false;
			}
		};
		syncFileDropUi();
		query.addEventListener('change', syncFileDropUi);

		const compact = window.matchMedia(COMPACT_QUERY);
		const syncCompact = () => {
			app.compactLayout = compact.matches;
			if (!compact.matches) app.sheetInset = 0;
		};
		syncCompact();
		compact.addEventListener('change', syncCompact);

		// The sheet sizes its snaps against the viewport and the map caps how far
		// it lifts the basemap chrome, so both read one measurement taken here.
		const syncViewport = () => (app.viewportHeight = window.innerHeight);
		syncViewport();
		window.addEventListener('resize', syncViewport);
		window.addEventListener('orientationchange', syncViewport);

		return () => {
			query.removeEventListener('change', syncFileDropUi);
			compact.removeEventListener('change', syncCompact);
			window.removeEventListener('resize', syncViewport);
			window.removeEventListener('orientationchange', syncViewport);
		};
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
	/>

	<AppHeader />
	<ControlPanel />
	<SolveCard />
	<!-- ControlPanel mounts the lookup and the footer inline when compact. -->
	{#if !app.compactLayout}
		<BusPicker />
	{/if}
	<DropZone />
	<PlacementCue />
	<RestoreDefaultsButton />
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
