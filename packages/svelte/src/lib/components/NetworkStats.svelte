<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt, signed, splitName } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
</script>

{#if ctrl.networkStats}
	{@const stats = ctrl.networkStats}
	{@const [cname, cregion] = splitName(app.active?.name ?? '')}
	{@const deltaObjective = stats.deltaObjective}
	<h2>{cname} <span class="region mono">{cregion}</span></h2>
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
				<dt>Binding lines</dt>
				<dd class="mono">{stats.binding ?? '…'}</dd>
			</div>
			<div>
				<dt>OPF objective</dt>
				{#if stats.objective === null}
					<dd class="blink">Solving&hellip;</dd>
				{:else}
					<dd class="mono">{fmt.format(stats.objective)}</dd>
				{/if}
			</div>
			{#if ctrl.isPerturbed(ctrl.activeSolvable) && deltaObjective !== null}
				<div class="delta">
					<dt>Change from base</dt>
					<dd class="mono">{signed(deltaObjective)}</dd>
				</div>
			{/if}
		{/if}
	</dl>
{/if}

<style>
	h2 {
		margin: 0 0 12px;
		font-size: 16px;
		font-weight: 600;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
	}

	dl .delta dd {
		color: var(--text-accent);
	}

	dt {
		color: var(--text-secondary);
	}

	dd {
		margin: 0;
	}
</style>
