<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { signed, splitName } from '../format.js';
	import OutcomeVerdict from './OutcomeVerdict.svelte';

	const app = getAppState();
	const ctrl = getController();
</script>

{#if ctrl.networkStats}
	{@const stats = ctrl.networkStats}
	{@const [cname, cregion] = splitName(app.active?.name ?? '')}
	{@const deltaObjective = stats.deltaObjective}
	<h2>{cname} <span class="region">{cregion}</span></h2>
	<OutcomeVerdict />
	<dl>
		<div>
			<dt>Buses</dt>
			<dd class="mono">{stats.buses}</dd>
		</div>
		<div>
			<dt>Lines</dt>
			<dd class="mono">{stats.branches}</dd>
		</div>
		{#if ctrl.activeFormulation !== 'acpf'}
			<div>
				<dt>{ctrl.bindingRule?.label ?? 'Lines near or above rating'}</dt>
				<dd class="mono">{stats.binding ?? '…'}</dd>
			</div>
			<!-- The result sentence above states the cost; the row only marks a solve in progress. -->
			{#if stats.objective === null}
				<div>
					<dt>Generation cost</dt>
					<dd class="blink">Solving&hellip;</dd>
				</div>
			{/if}
			{#if ctrl.isPerturbed(ctrl.activeSolvable) && deltaObjective !== null}
				<div class="delta">
					<dt>Cost change from the base case ($/h)</dt>
					<dd class="mono">{signed(deltaObjective)}</dd>
				</div>
			{/if}
		{/if}
	</dl>
{/if}

<style>
	h2 {
		margin: 0 0 var(--space-lg);
		font-size: var(--fs-heading);
		font-weight: 600;
	}

	.region {
		font-weight: 400;
		color: var(--text-secondary);
	}

	dl {
		margin: 0;
		font-size: var(--fs-data);
	}

	dl div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-lg);
		padding: var(--space-3xs) 0;
	}

	dl .delta dd {
		color: var(--text-accent);
	}

	dt {
		color: var(--text-secondary);
	}

	dd {
		margin: 0;
		text-align: right;
	}
</style>
