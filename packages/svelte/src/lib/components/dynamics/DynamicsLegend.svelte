<script lang="ts">
	import { dynVoltageGradient, haloLegendRadius } from '../../dynamics/legend.js';

	let {
		theme = 'light',
		top = 112,
		atRest = false
	}: { theme?: 'light' | 'dark'; top?: number; atRest?: boolean } = $props();
	const gradient = $derived(dynVoltageGradient(theme));
	const ticks = [0.8, 0.9, 1.0, 1.1];
	const pos = (v: number) => ((v - 0.5) / (1.2 - 0.5)) * 100;
</script>

<!-- Not `.legend`: the global stylesheet's .legend is the 6px colour-ramp strip. -->
<aside class="dyn-legend" style:top="{top}px" aria-label="Dynamics map legend">
	<div class="row">
		<span class="title">Bus voltage (pu)</span>
		<div class="bar" style:background={gradient}>
			{#each ticks as t (t)}<span class="tick" style:left="{pos(t)}%">{t.toFixed(1)}</span>{/each}
		</div>
	</div>
	<div class="row halos">
		<svg width="94" height="30" aria-hidden="true">
			<circle cx="15" cy="15" r={haloLegendRadius(0)} class="rest" />
			<circle cx="45" cy="15" r={haloLegendRadius(0.01)} class="under" />
			<circle cx="75" cy="15" r={haloLegendRadius(0.1)} class="over" />
		</svg>
		<span>Ring: speed deviation, <b class="u">slow</b> or <b class="o">fast</b></span>
	</div>
	{#if atRest}
		<p class="rest-note" data-testid="dyn-legend-rest">At rest</p>
	{/if}
</aside>

<style>
	.dyn-legend {
		position: fixed;
		left: 50%;
		transform: translateX(-50%);
		z-index: 12;
		display: grid;
		gap: 4px;
		max-width: min(440px, calc(100vw - 32px));
		padding: 6px 10px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--panel);
		font-size: var(--fs-control);
		color: var(--text-secondary);
	}
	.rest-note {
		margin: 0;
		color: var(--ink);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.title {
		white-space: nowrap;
		color: var(--ink);
	}
	.bar {
		position: relative;
		flex: 1;
		height: 10px;
		min-width: 160px;
		margin-bottom: 12px;
		border-radius: var(--radius-control);
	}
	.tick {
		position: absolute;
		top: 11px;
		transform: translateX(-50%);
		font-family: var(--font-mono);
		font-size: var(--fs-meta);
	}
	.tick::before {
		content: '';
		position: absolute;
		left: 50%;
		top: -4px;
		height: 4px;
		border-left: 1px solid var(--ink-dim);
	}
	svg {
		flex: none;
	}
	circle {
		fill: none;
		stroke-width: 2;
	}
	.under {
		stroke: var(--dyn-speed-under);
	}
	.over {
		stroke: var(--dyn-speed-over);
	}
	.rest {
		stroke: var(--text-tertiary);
		opacity: 0.6;
	}
	.u {
		color: var(--dyn-speed-under);
	}
	.o {
		color: var(--dyn-speed-over);
	}
	@media (max-width: 960px) {
		.halos {
			display: none;
		}
	}
</style>
