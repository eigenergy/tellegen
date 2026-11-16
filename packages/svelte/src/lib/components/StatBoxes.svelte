<script lang="ts">
	import type { Stat } from '../diagram/verdict.js';

	/** A row of labelled numbers: the label small above, the value large below. */
	let { stats, label = 'Result' }: { stats: Stat[]; label?: string } = $props();

	/** A value too long for half the row spans the row, so it is never cut off; a box left alone
	 * in its row spans it too, so no half row stays empty. A cost such as $4,815,984/h fits in
	 * half a row: the value's type shrinks a little in a narrow box before it would be cut. */
	const LONG = 14;
	const spans = $derived.by(() => {
		let column = 0;
		return stats.map((stat, i) => {
			const wide =
				stat.value.length > LONG ||
				(column === 0 && (i === stats.length - 1 || stats[i + 1].value.length > LONG));
			column = wide ? 0 : 1 - column;
			return wide;
		});
	});
</script>

{#if stats.length}
	<dl class="stats" aria-label={label}>
		{#each stats as stat, i (stat.label)}
			<div class={['stat', stat.tone, { wide: spans[i] }]} title={stat.title}>
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
	.wide {
		grid-column: 1 / -1;
	}
	.stat {
		container-type: inline-size;
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
		font-size: clamp(15px, 13cqi, var(--fs-display));
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
	/* Colour by tone, the same as the map: red at or over a limit. A box near a limit
	   keeps the ink colour, so every label in a row reads alike. */
	.failure .value {
		color: var(--red);
	}
	/* A value that is a short status, not a number: no figure to make large. */
	.pending .value {
		font-size: var(--fs-data);
		font-weight: 400;
		line-height: calc(var(--fs-display) * var(--lh-tight));
		color: var(--text-secondary);
	}
</style>
