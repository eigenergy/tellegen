<script lang="ts">
	import { dynVoltageGradient, haloLegendRadius } from '../../dynamics/legend.js';

	let { theme = 'light', top = 112 }: { theme?: 'light' | 'dark'; top?: number } = $props();
	const gradient = $derived(dynVoltageGradient(theme));
</script>

<!-- Not `.legend`: the global stylesheet's .legend is the 6px colour-ramp strip. -->
<aside class="dyn-legend" style:top="{top}px" aria-label="Dynamics map legend">
	<div class="row">
		<span class="title">Voltage</span>
		<span class="end">0.5</span>
		<i class="bar" style:background={gradient}></i>
		<span class="end">1.2 pu</span>
	</div>
	<div class="row halos">
		<span class="title">Speed</span>
		<span class="ring"
			><svg width="18" height="18" aria-hidden="true"
				><circle cx="9" cy="9" r="4" class="rest" /></svg
			>Normal</span
		>
		<span class="ring"
			><svg width="18" height="18" aria-hidden="true"
				><circle cx="9" cy="9" r={Math.min(8, haloLegendRadius(0.01))} class="under" /></svg
			>Slow</span
		>
		<span class="ring"
			><svg width="18" height="18" aria-hidden="true"
				><circle cx="9" cy="9" r="8" class="over" /></svg
			>Fast</span
		>
	</div>
</aside>

<style>
	.dyn-legend {
		position: fixed;
		left: 50%;
		transform: translateX(-50%);
		z-index: 12;
		display: grid;
		gap: 6px;
		max-width: min(440px, calc(100vw - 32px));
		padding: 6px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		background: var(--panel);
		font-size: var(--fs-control);
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.title {
		width: 52px;
		color: var(--ink);
	}
	.bar {
		display: block;
		width: 140px;
		height: 8px;
		border-radius: var(--radius-control);
	}
	.ring {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		margin-right: 6px;
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
	}
	@media (max-width: 960px) {
		.halos {
			display: none;
		}
	}
</style>
