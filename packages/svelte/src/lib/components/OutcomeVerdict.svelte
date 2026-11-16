<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { resultStats } from '../diagram/verdict.js';
	import StatBoxes from './StatBoxes.svelte';

	/** The network summary's key numbers for the active result. */
	const ctrl = getController();
	const app = getAppState();
	/** Dynamics mode on a case with dynamic data: the map shows the simulation instead. */
	const dynamics = $derived(app.mode === 'dynamics' && !!ctrl.dynamics.active);
	const stats = $derived.by(() => {
		const network = ctrl.networkStats;
		const c = ctrl.activeSolvable;
		if (!network || !c) return [];
		const solution = c.solution;
		let voltage: { min: number; max: number } | null = null;
		if (solution?.vm?.length) {
			let min = Infinity;
			let max = -Infinity;
			for (const { value } of solution.vm) {
				min = Math.min(min, value);
				max = Math.max(max, value);
			}
			voltage = { min, max };
		}
		let maxLoading: number | null = null;
		for (const flow of solution?.flows ?? [])
			if (Number.isFinite(flow.loading)) maxLoading = Math.max(maxLoading ?? 0, flow.loading);
		return resultStats({
			formulation: ctrl.activeFormulation,
			solving: c.solving,
			objective: network.objective,
			binding: network.binding,
			deltaObjective: ctrl.isPerturbed(c) ? network.deltaObjective : null,
			voltage,
			maxLoading
		});
	});
</script>

{#if !dynamics && (ctrl.activeSolvable?.solution || ctrl.activeSolvable?.solving)}
	<StatBoxes {stats} />
{/if}
