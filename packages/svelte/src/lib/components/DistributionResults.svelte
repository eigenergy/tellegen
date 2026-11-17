<script lang="ts">
	import { getController } from '../context.svelte.js';
	import type { MulticonductorCase } from '../state.svelte.js';
	import McWorkbench from './McWorkbench.svelte';
	import McResults from './McResults.svelte';
	let { caseState }: { caseState: MulticonductorCase } = $props();
	const ctrl = getController();
</script>

{#if caseState.result && caseState.graph && caseState.overviewInputs && caseState.overviewSearch}<McWorkbench
		snapshot={caseState.overviewSnapshot}
		graph={caseState.graph}
		inputs={caseState.overviewInputs}
		search={caseState.overviewSearch}
		loading={caseState.overviewLoading}
		selectedBus={caseState.selectedBusId}
		selectedEdge={caseState.selectedEdgeId}
		onselectbus={(id) => ctrl.selectMultiBus(caseState.id, id)}
		onselectedge={(id) => ctrl.selectMultiEdge(caseState.id, id)}
		label={caseState.label}
		schematic={caseState.schematic}
		onschematic={(show) => (caseState.schematic = show)}
		hovered={caseState.hovered}
		onhover={(h) => (caseState.hovered = h)}
		onreveal={() => caseState.revealSeq++}
		onanalyse={ctrl.overviewOnRequest(caseState) ? () => ctrl.readOverview(caseState) : undefined}
	/>{/if}
{#if caseState.result}<McResults
		result={caseState.result}
		excludedIslands={caseState.mcExcludedIslands}
		detail={(query) => ctrl.multiResultDetail(caseState, query)}
		timing={caseState.mcTiming}
		elapsedMs={caseState.solveMs}
		selectedBus={caseState.selectedBusId}
		selectedEdge={caseState.selectedEdgeId}
		graph={caseState.graph}
		searchIndex={caseState.overviewSearch}
	/>{/if}
