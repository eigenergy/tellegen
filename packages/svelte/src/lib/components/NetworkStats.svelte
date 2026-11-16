<script lang="ts">
	import type { Snippet } from 'svelte';
	import { getAppState, getController } from '../context.svelte.js';
	import { splitName } from '../format.js';
	import OutcomeVerdict from './OutcomeVerdict.svelte';

	const app = getAppState();
	const ctrl = getController();

	/** Content placed right after the result numbers, such as a selection readout. */
	let { lead }: { lead?: Snippet } = $props();
</script>

{#if ctrl.networkStats}
	{@const stats = ctrl.networkStats}
	{@const [cname, cregion] = splitName(app.active?.name ?? '')}
	<h2>{cname} <span class="region">{cregion}</span></h2>
	<OutcomeVerdict />
	{@render lead?.()}
	<dl>
		<div>
			<dt>Buses</dt>
			<dd class="mono">{stats.buses.toLocaleString('en-US')}</dd>
		</div>
		<div>
			<dt>Lines</dt>
			<dd class="mono">{stats.branches.toLocaleString('en-US')}</dd>
		</div>
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

	dt {
		color: var(--text-secondary);
	}

	dd {
		margin: 0;
		text-align: right;
	}
</style>
