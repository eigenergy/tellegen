<script lang="ts">
	import type { Stat } from '../diagram/verdict.js';

	/** A row of labelled numbers: the label small above, the value large below. */
	let { stats, label = 'Result' }: { stats: Stat[]; label?: string } = $props();
</script>

{#if stats.length}
	<dl class="stats" aria-label={label}>
		{#each stats as stat (stat.label)}
			<div class={['stat', stat.tone]}>
				<dt>{stat.label}</dt>
				<dd>
					<span class="value" title={stat.value}>{stat.value}</span>
					{#if stat.sub}<span class="sub">{stat.sub}</span>{/if}
				</dd>
			</div>
		{/each}
	</dl>
{/if}

<style>
	/* Two equal columns wherever the boxes appear, so the Network panel and the diagram match. */
	.stats {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 8px;
		margin: 0 0 var(--space-lg);
	}
	.stat {
		min-width: 0;
		padding: 8px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-panel);
	}
	dt {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: var(--fs-label);
		color: var(--text-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	dd {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 2px 8px;
		min-width: 0;
		margin: 2px 0 0;
		font-variant-numeric: tabular-nums lining-nums;
	}
	.value {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		font-size: var(--fs-display);
		font-weight: 600;
		line-height: var(--lh-tight);
		color: var(--text-primary);
		white-space: nowrap;
	}
	.sub {
		font-size: var(--fs-label);
		color: var(--text-secondary);
		white-space: nowrap;
	}
	/* Colour by tone, the same as the map: red at or over a limit, an amber dot near one. */
	.failure .value {
		color: var(--red);
	}
	.warning dt::before {
		content: '';
		flex: none;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--amber);
	}
</style>
