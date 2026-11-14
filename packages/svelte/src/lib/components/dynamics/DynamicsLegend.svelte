<script lang="ts">
	import { dynVoltageGradient, haloLegendRadius } from '../../dynamics/legend.js';

	let { theme = 'light', top = 112 }: { theme?: 'light' | 'dark'; top?: number } = $props();
	const gradient = $derived(dynVoltageGradient(theme));
	const ticks = [0.8, 0.9, 1.0, 1.1];
	const pos = (v: number) => ((v - 0.5) / (1.2 - 0.5)) * 100;
</script>

<!-- Not `.legend`: the global stylesheet's .legend is the 6px colour-ramp strip. -->
<aside class="dyn-legend" style:top="{top}px" aria-label="Dynamics map legend">
	<div class="row">
		<span class="title">Bus |V| (pu)</span>
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
		<span
			>Ring size = speed deviation (small: 0.01 Hz, large: 0.1 Hz); <b class="u">teal</b> below,
			<b class="o">magenta</b> above nominal. Faint grey: at nominal (within 1 mHz) or held at power-flow
			output.</span
		>
	</div>
	<details>
		<summary>What am I looking at?</summary>
		<p>
			Each dot is a bus, coloured by its voltage magnitude at the time on the timeline: neutral near
			1.0 pu, warm as voltage sags, purple above 1.05 pu. Rings around generating buses grow as
			machines speed up or slow down. Press play, or drag the timeline, to watch the disturbance
			travel and settle.
		</p>
	</details>
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
		border-radius: 2px;
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
	details p {
		margin: 4px 0 0;
		line-height: 1.45;
	}
	summary {
		cursor: pointer;
	}
	@media (max-width: 960px) {
		.halos,
		details {
			display: none;
		}
	}
</style>
