<script lang="ts">
	import type { SolveIteration } from './api.js';

	let { iterations }: { iterations: SolveIteration[] } = $props();

	/** Drawn at its own aspect ratio, so the text keeps its shape at any panel width. */
	const W = 300;
	const H = 104;
	const T = 20;
	const R = 8;
	const B = 22;
	const L = 46;
	const PLOT_W = W - L - R;
	const PLOT_H = H - T - B;
	const Y_TOP = 0; // log10(1)
	const Y_BOT = -9; // log10(feasibility tolerance)

	const points = $derived.by(() => {
		if (iterations.length < 2) return '';
		const span = Y_TOP - Y_BOT;
		return iterations
			.map((it, k) => {
				const v = Number(it.inf_pr);
				const logv = Number.isFinite(v) && v > 0 ? Math.log10(v) : Y_BOT;
				const clamped = Math.min(Y_TOP, Math.max(Y_BOT, logv));
				const x = (L + (k / (iterations.length - 1)) * PLOT_W).toFixed(1);
				const y = (T + ((Y_TOP - clamped) / span) * PLOT_H).toFixed(1);
				return `${x},${y}`;
			})
			.join(' ');
	});
</script>

{#if points}
	<svg
		viewBox="0 0 {W} {H}"
		role="img"
		aria-label="Error by iteration, {iterations.length} iterations"
	>
		<text class="label" x={L} y="11">Error</text>
		<line class="axis" x1={L} y1={T} x2={L} y2={H - B} />
		<line class="axis" x1={L} y1={H - B} x2={W - R} y2={H - B} />
		<text class="label" x={L - 5} y={T + 4} text-anchor="end">High</text>
		<text class="label" x={L - 5} y={H - B + 4} text-anchor="end">Solved</text>
		<text class="label" x={L} y={H - 6} text-anchor="middle">1</text>
		<text class="label" x={(L + W - R) / 2} y={H - 6} text-anchor="middle">Iteration</text>
		<text class="label" x={W - R} y={H - 6} text-anchor="end">{iterations.length}</text>
		<polyline {points} pathLength="1" />
	</svg>
{/if}

<style>
	svg {
		display: block;
		width: 100%;
		height: auto;
		margin-bottom: var(--space-xs);
	}

	.axis {
		stroke: var(--line);
		stroke-width: 1;
	}

	.label {
		fill: var(--text-secondary);
		font-family: var(--font-sans);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}

	polyline {
		fill: none;
		stroke: var(--accent);
		stroke-width: 1.8;
		stroke-linejoin: round;
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
		animation: sparkline-draw var(--dur-slow) var(--ease-out) forwards;
	}

	@keyframes sparkline-draw {
		to {
			stroke-dashoffset: 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		polyline {
			animation: none;
			stroke-dashoffset: 0;
		}
	}
</style>
