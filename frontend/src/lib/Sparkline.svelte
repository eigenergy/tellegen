<script lang="ts">
	import type { SolveIteration } from '$lib/api';

	let { iterations }: { iterations: SolveIteration[] } = $props();

	const W = 240;
	const H = 30;

	// Primal infeasibility on a log scale: the canonical interior point
	// convergence picture, a staircase falling to the tolerance floor.
	const points = $derived.by(() => {
		if (iterations.length < 2) return '';
		const ys = iterations.map((it) => Math.log10(Math.max(it.inf_pr, 1e-14)));
		const ymin = Math.min(...ys);
		const ymax = Math.max(...ys);
		const span = ymax - ymin || 1;
		const n = iterations.length;
		return iterations
			.map((_, k) => {
				const x = ((k / (n - 1)) * (W - 6) + 3).toFixed(1);
				const y = (3 + ((ymax - ys[k]) / span) * (H - 6)).toFixed(1);
				return `${x},${y}`;
			})
			.join(' ');
	});
</script>

{#if points}
	<svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="solver convergence">
		<polyline {points} />
	</svg>
{/if}

<style>
	svg {
		display: block;
		width: 100%;
		height: 30px;
	}

	polyline {
		fill: none;
		stroke: var(--accent);
		stroke-width: 1.5;
		stroke-linejoin: round;
	}
</style>
