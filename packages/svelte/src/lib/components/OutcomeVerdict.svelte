<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { outcomeVerdict } from '../diagram/verdict.js';

	/** The network summary's lead sentence: what the active result found. */
	const ctrl = getController();
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

{#if verdict}
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
