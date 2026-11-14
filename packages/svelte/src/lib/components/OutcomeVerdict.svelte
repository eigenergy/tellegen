<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { outcomeVerdict } from '../diagram/verdict.js';

	/** The network summary's lead sentence: what the active result found. */
	const ctrl = getController();
	const app = getAppState();
	/** Dynamics mode on a case with dynamic data: the steady-state result is
	 * not what the map shows. */
	const dynamics = $derived(app.mode === 'dynamics' && !!ctrl.dynamics.active);
	const verdict = $derived.by(() => {
		const stats = ctrl.networkStats;
		if (!stats) return null;
		return outcomeVerdict({
			formulation: ctrl.activeFormulation,
			objective: stats.objective,
			binding: stats.binding,
			flagged: ctrl.bindingRule?.label === 'Binding lines',
			deltaObjective: ctrl.isPerturbed(ctrl.activeSolvable) ? stats.deltaObjective : null
		});
	});
</script>

{#if dynamics}
	<p class="outcome" data-testid="outcome-dynamics">
		Dynamics mode: the map shows the dynamic simulation. Steady state has the power flow and
		dispatch results.
	</p>
{:else if verdict}
	<p class={['outcome', verdict.tone]} role="status">{verdict.text}</p>
{/if}

<style>
	.outcome {
		margin: 0 0 var(--space-lg);
		font-size: var(--fs-body);
		font-weight: 600;
		line-height: var(--lh-snug);
		color: var(--text-primary);
		text-wrap: pretty;
	}
</style>
