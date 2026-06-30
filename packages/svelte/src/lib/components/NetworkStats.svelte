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
	<dl class="mono">
		<div>
			<dt>buses</dt>
			<dd>{stats.buses}</dd>
		</div>
		<div>
			<dt>branches</dt>
			<dd>{stats.branches}</dd>
		</div>
		<div>
			<dt>binding lines</dt>
			<dd>{stats.binding ?? '…'}</dd>
		</div>
		<div>
			<dt>cost</dt>
			<dd>
				{#if stats.objective === null}
					<span class="blink">solving&hellip;</span>
				{:else}
					{fmt.format(stats.objective)} $/h
				{/if}
			</dd>
		</div>
		{#if ctrl.isPerturbed(ctrl.activeSolvable) && deltaObjective !== null}
			<div class="delta">
				<dt>vs base</dt>
				<dd>{signed(deltaObjective)} $/h</dd>
			</div>
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
		color: var(--accent);
	}

	dt {
		color: var(--ink-dim);
	}

	dd {
		margin: 0;
	}
</style>
