<script lang="ts">
	import type { SolveIteration } from '$lib/api';

	let { iterations }: { iterations: SolveIteration[] } = $props();

	const W = 240;
	const H = 30;

	// Primal infeasibility on a FIXED log10 axis: top = 1e0 (problem scale), bottom
	// = the solver feasibility tolerance (tol_feas = 1e-9 in crates/tellegen/src/solve.rs).
	// inf_pr is Clarabel's *relative* primal residual, so it runs ~1 -> tol for every
	// well-posed solve; rescaling each trace to its own min/max made every curve fill
	// the box identically (the "same shape every solve" bug). Anchoring to fixed
	// bounds lets the descent depth and per-iterate path vary with the operating point.
	const Y_TOP = 0; // log10(1e0)
	const Y_BOT = -9; // log10(tol_feas)
	const points = $derived.by(() => {
		if (iterations.length < 2) return '';
		const n = iterations.length;
		const span = Y_TOP - Y_BOT;
		return iterations
			.map((it, k) => {
				const v = Number(it.inf_pr);
				// Floor non-finite/non-positive residuals at the tolerance so a bad
				// iterate can't poison the polyline with NaN coordinates.
				const logv = Number.isFinite(v) && v > 0 ? Math.log10(v) : Y_BOT;
				const clamped = Math.min(Y_TOP, Math.max(Y_BOT, logv));
				const x = ((k / (n - 1)) * (W - 6) + 3).toFixed(1);
				const y = (3 + ((Y_TOP - clamped) / span) * (H - 6)).toFixed(1);
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
