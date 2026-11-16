<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { flowSummary, resultStats } from '../diagram/verdict.js';
	import StatBoxes from './StatBoxes.svelte';
	import BindingLines from './BindingLines.svelte';

	/** The network summary's key numbers for the active result. */
	const ctrl = getController();
	const app = getAppState();
	const stats = $derived.by(() => {
		const network = ctrl.networkStats;
		const c = ctrl.activeSolvable;
		if (!network || !c) return [];
		const solution = c.solution;
		const summary = flowSummary(
			solution?.vm,
			solution?.flows?.map((f) => ({ loading: f.loading, from: f.mw, to: f.pt }))
		);
		return resultStats({
			formulation: ctrl.activeFormulation,
			solving: c.solving,
			objective: network.objective,
			binding: network.binding,
			...summary
		});
	});
</script>

<!-- The steady-state result keeps its numbers in dynamics mode too, as on every other case. -->
{#if ctrl.activeSolvable?.solution || ctrl.activeSolvable?.solving}
	<StatBoxes {stats} />
	{#if !app.placingLocalId}<BindingLines />{/if}
{/if}
