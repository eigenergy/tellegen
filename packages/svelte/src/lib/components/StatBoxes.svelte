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
					<span class="value">{stat.value}</span>
					{#if stat.sub}<span class="sub">{stat.sub}</span>{/if}
				</dd>
			</div>
		{/each}
	</dl>
{/if}

<style>
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(112px, 1fr));
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
		margin: 2px 0 0;
		font-variant-numeric: tabular-nums lining-nums;
	}
	.value {
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
	/* A count that needs attention carries a coloured edge: amber near a limit, red over it. */
	.warning {
		box-shadow: inset 3px 0 0 var(--amber);
	}
	.failure {
		box-shadow: inset 3px 0 0 var(--red);
	}
	.failure .value {
		color: var(--text-danger);
	}
</style>
