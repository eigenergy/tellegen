<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { resultStats } from '../diagram/verdict.js';
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
		// Each branch loses what enters at one end and does not leave at the other.
		let losses: number | null = null;
		for (const flow of solution?.flows ?? []) {
			if (Number.isFinite(flow.loading)) maxLoading = Math.max(maxLoading ?? 0, flow.loading);
			if (flow.pt != null && Number.isFinite(flow.mw + flow.pt))
				losses = (losses ?? 0) + flow.mw + flow.pt;
		}
		return resultStats({
			formulation: ctrl.activeFormulation,
			solving: c.solving,
			objective: network.objective,
			binding: network.binding,
			voltage,
			maxLoading,
			losses
		});
	});
</script>

<!-- The steady-state result keeps its numbers in dynamics mode too, as on every other case. -->
{#if ctrl.activeSolvable?.solution || ctrl.activeSolvable?.solving}
	<StatBoxes {stats} />
	{#if !app.placingLocalId}<BindingLines />{/if}
{/if}
