<script lang="ts">
	import type { SolveIteration } from './api.js';

	let { iterations }: { iterations: SolveIteration[] } = $props();

	const W = 240;
	const H = 54;
	const T = 4;
	const R = 4;
	const B = 18;
	const L = 30;
	const PLOT_W = W - L - R;
	const PLOT_H = H - T - B;
	const Y_TOP = 0; // log10(1e0)
	const Y_BOT = -9; // log10(tol_feas)

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
		preserveAspectRatio="none"
		role="img"
		aria-label="residual by solver iteration"
	>
		<line class="axis" x1={L} y1={T} x2={L} y2={H - B} />
		<line class="axis" x1={L} y1={H - B} x2={W - R} y2={H - B} />
		<text
			class="axis-label y-label"
			x={-(T + PLOT_H / 2)}
			y="8"
			text-anchor="middle"
			transform="rotate(-90)"
		>
			residual
		</text>
		<text class="axis-label x-label" x={W - R} y={H - 4} text-anchor="end">iterations</text>
		<polyline {points} pathLength="1" />
	</svg>
{/if}

<style>
	svg {
		display: block;
		width: 100%;
		height: 54px;
		overflow: visible;
	}

	.axis {
		stroke: var(--line);
		stroke-width: 1;
		vector-effect: non-scaling-stroke;
	}

	.axis-label {
		fill: var(--ink-faint);
		font-family: var(--font-mono);
		font-size: 8.5px;
		letter-spacing: 0;
	}

	polyline {
		fill: none;
		stroke: var(--accent);
		stroke-width: 1.5;
		stroke-linejoin: round;
		vector-effect: non-scaling-stroke;
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
