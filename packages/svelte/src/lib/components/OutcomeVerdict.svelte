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
		// AC power flow has no cost to report; its result is the voltage profile.
		const vm = ctrl.activeSolvable?.solution?.vm;
		if (ctrl.activeFormulation === 'acpf' && vm?.length) {
			let lo = Infinity;
			let hi = -Infinity;
			for (const { value } of vm) {
				lo = Math.min(lo, value);
				hi = Math.max(hi, value);
			}
			return {
				text: `Power flow solved. Voltages run from ${lo.toFixed(3)} to ${hi.toFixed(3)} pu.`,
				tone: 'ok' as const
			};
		}
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
	/* The one large type moment on the page: what the result found. */
	.outcome {
		margin: 4px 0 16px;
		font-size: var(--fs-display-lg);
		font-weight: 600;
		line-height: var(--lh-tight);
		letter-spacing: -0.01em;
		color: var(--text-primary);
		text-wrap: pretty;
	}
	@media (max-width: 760px) {
		.outcome {
			font-size: var(--fs-display);
		}
	}
</style>
