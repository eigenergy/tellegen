<script lang="ts">
	import type { RmsLinearResult } from '@tellegen/engine';

	let {
		result,
		selected = $bindable(null)
	}: { result: RmsLinearResult; selected?: number | null } = $props();

	const W = 340;
	const H = 200;
	const pad = 30;
	/** Oscillatory modes (one of each conjugate pair) and slow real ones. */
	const modes = $derived(
		result.modes
			.map((m, i) => ({ ...m, i }))
			.filter((m) => m.im >= 0 && Number.isFinite(m.re) && m.re > -40)
	);
	const fMax = $derived(Math.max(2.5, ...modes.map((m) => m.freq_hz)));
	const sMin = $derived(Math.min(-5, ...modes.map((m) => m.re)));
	const sMax = $derived(Math.max(1, ...modes.map((m) => m.re)));
	const x = (s: number) => pad + ((s - sMin) / (sMax - sMin)) * (W - pad - 8);
	const y = (f: number) => H - pad - (f / fMax) * (H - pad - 8);
	const ray = (z: number) => {
		const s = (-z / Math.sqrt(1 - z * z)) * 2 * Math.PI * fMax;
		return { x2: x(Math.max(s, sMin)), y2: y(s < sMin ? (fMax * sMin) / s : fMax) };
	};
	const table = $derived(
		[...modes]
			.filter((m) => m.freq_hz > 0.05)
			.sort((a, b) => a.damping - b.damping)
			.slice(0, 12)
	);
	const unstable = $derived(modes.filter((m) => m.re > 1e-6).length);
</script>

<figure>
	<svg
		viewBox="0 0 {W} {H}"
		role="img"
		aria-label="Eigenvalues: real part against frequency, {modes.length} modes, {unstable} with a positive real part"
	>
		<line class="axis" x1={x(0)} x2={x(0)} y1="8" y2={H - pad} />
		<line class="axis" x1={pad} x2={W - 8} y1={H - pad} y2={H - pad} />
		{#each [0.05, 0.1] as z (z)}
			{@const r = ray(z)}
			<line class="ray" x1={x(0)} y1={y(0)} x2={r.x2} y2={r.y2} />
			<text class="tick" x={r.x2 + 2} y={r.y2 + 10}>{z * 100} %</text>
		{/each}
		{#each modes as m (m.i)}
			<circle
				class={['pt', { sel: selected === m.i, bad: m.re > 1e-6 }]}
				cx={x(m.re)}
				cy={y(m.freq_hz)}
				r={selected === m.i ? 4.5 : 2.5}
				role="button"
				tabindex="-1"
				aria-label="mode {m.freq_hz.toFixed(2)} Hz, damping {(m.damping * 100).toFixed(1)} %"
				onclick={() => (selected = m.i)}
				onkeydown={(e) => e.key === 'Enter' && (selected = m.i)}
			/>
		{/each}
		<text class="tick" x={W - 8} y={H - pad + 14} text-anchor="end">σ (1/s)</text>
		<text class="tick" x={pad - 4} y="14" text-anchor="end">Hz</text>
		<text class="tick" x={x(sMin)} y={H - pad + 14}>{sMin.toFixed(0)}</text>
		<text class="tick" x={x(0)} y={H - pad + 14} text-anchor="middle">0</text>
		<text class="tick" x={pad - 4} y={y(fMax) + 4} text-anchor="end">{fMax.toFixed(1)}</text>
	</svg>
	<figcaption>
		Rays: 5 % and 10 % damping. {unstable
			? `${unstable} mode(s) with a positive real part.`
			: 'No mode with a positive real part.'}
	</figcaption>
</figure>
<table>
	<thead><tr><th>f (Hz)</th><th>ζ</th><th>Top participants</th></tr></thead>
	<tbody>
		{#each table as m (m.i)}
			<tr class={{ sel: selected === m.i }} onclick={() => (selected = m.i)}>
				<td>{m.freq_hz.toFixed(3)}</td>
				<td>{(m.damping * 100).toFixed(1)} %</td>
				<td class="parts"
					>{m.participation
						.slice(0, 3)
						.map((p) => p.state)
						.join(', ')}</td
				>
			</tr>
		{/each}
	</tbody>
</table>

<style>
	figure {
		margin: 0;
	}
	svg {
		width: 100%;
		height: auto;
	}
	.axis {
		stroke: var(--ink-faint);
		stroke-width: 1;
	}
	.ray {
		stroke: var(--warn);
		stroke-dasharray: 3 3;
	}
	.tick {
		fill: var(--text-tertiary);
		font-size: 9px;
	}
	.pt {
		fill: var(--series-1);
		cursor: pointer;
	}
	.pt.bad {
		fill: var(--text-danger);
	}
	.pt.sel {
		stroke: var(--ink);
		stroke-width: 1.5;
	}
	figcaption {
		font-size: var(--fs-control);
		color: var(--text-secondary);
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
	}
	th,
	td {
		text-align: left;
		padding: 2px 4px;
		border-bottom: 1px solid var(--line);
	}
	tr.sel {
		background: var(--accent-soft);
	}
	tbody tr {
		cursor: pointer;
	}
	.parts {
		font-family: var(--font-mono);
		font-size: var(--fs-meta);
	}
</style>
