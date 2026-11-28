<script lang="ts">
	/** The key to the map's dynamics colours: bus voltage and generator speed.
	 * @block Dynamics map legend */
	import { dynVoltageGradient, haloLegendRadius } from '../../dynamics/legend.js';

	/** The key to the map's dynamics colours, in the Network panel where the steady-state key
	 * sits, so it never covers the map. */
	let { theme = 'light' }: { theme?: 'light' | 'dark' } = $props();
	const gradient = $derived(dynVoltageGradient(theme));
</script>

<aside class="dyn-legend" aria-label="Dynamics map legend">
	<p class="title">Voltage (pu)</p>
	<div class="legend" style:background={gradient}></div>
	<div class="legend-labels"><span>0.5</span><span>1.2</span></div>
	<p class="title">Generator speed</p>
	<ul class="rings">
		<li>
			<svg width="18" height="18" aria-hidden="true"
				><circle cx="9" cy="9" r="4" class="rest" /></svg
			>Nominal
		</li>
		<li>
			<svg width="18" height="18" aria-hidden="true"
				><circle cx="9" cy="9" r={Math.min(8, haloLegendRadius(0.01))} class="under" /></svg
			>Low
		</li>
		<li>
			<svg width="18" height="18" aria-hidden="true"
				><circle cx="9" cy="9" r="8" class="over" /></svg
			>High
		</li>
	</ul>
</aside>

<style>
	/* The same type and spacing as the steady-state key it replaces. */
	.title {
		margin: 10px 0 0;
		font-size: 12px;
		font-weight: 600;
		color: var(--ink);
	}
	.dyn-legend > .title:first-child {
		margin-top: 0;
	}
	.rings {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		margin: 4px 0 0;
		padding: 0;
		list-style: none;
		font-size: 12px;
		color: var(--text-secondary);
	}
	.rings li {
		display: flex;
		align-items: center;
		gap: 4px;
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
</style>
