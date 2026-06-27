<script lang="ts">
	import { onMount } from 'svelte';
	import AppFooter from '$lib/components/AppFooter.svelte';
	import AppHeader from '$lib/components/AppHeader.svelte';
	import ControlPanel from '$lib/components/ControlPanel.svelte';
	import DropZone from '$lib/components/DropZone.svelte';
	import PlacementCue from '$lib/components/PlacementCue.svelte';
	import RestoreDefaultsButton from '$lib/components/RestoreDefaultsButton.svelte';
	import SolveCard from '$lib/components/SolveCard.svelte';
	import { setAppState, setController } from '$lib/context.svelte';
	import { createController } from '$lib/controller.svelte';
	import { createAppState } from '$lib/state.svelte';
	import TellegenMap from '$lib/TellegenMap.svelte';

	const FILE_DROP_QUERY = '(hover: hover) and (pointer: fine) and (min-width: 761px)';

	const app = createAppState();
	const ctrl = createController(app);
	setAppState(app);
	setController(ctrl);

	ctrl.load();

	let dragDepth = 0;

	// Fall back to LMP coloring when the active formulation drops the selected
	// display variable (e.g. leaving SOCWR removes |V|). The one effect that
	// stays in the page shell; everything else lives on the controller.
	$effect(() => {
		if (
			ctrl.displayOptions.length > 0 &&
			!ctrl.displayOptions.some((option) => option.mode === app.displayMode)
		) {
			app.displayMode = 'lmp';
		}
	});

	onMount(() => {
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
		return () => query.removeEventListener('change', syncFileDropUi);
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
		onplacecase={ctrl.placeLocalCase}
		onmapclick={ctrl.clearSelection}
	/>

	<AppHeader />
	<ControlPanel />
	<SolveCard />
	<DropZone />
	<PlacementCue />
	<RestoreDefaultsButton />
	<AppFooter />
</main>

<style>
	main {
		position: fixed;
		inset: 0;
		overflow: hidden;
	}
</style>
