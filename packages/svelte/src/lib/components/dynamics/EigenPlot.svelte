<script lang="ts">
	import type { RmsLinearResult } from '@tellegen/engine';
	import StatBoxes from '../StatBoxes.svelte';
	import { modeGenerators } from '../../dynamics/verdict.js';

	let {
		result,
		selected = $bindable(null)
	}: { result: RmsLinearResult; selected?: number | null } = $props();

	const W = 340;
	const H = 200;
	/** Plot area: room on the left for frequency ticks, above for the axis title, and
	 * below for the damping ticks and their title. */
	const L = 30;
	const R = 12;
	const T = 22;
	const B = 40;
	/** Oscillatory modes, one of each conjugate pair. Real modes have no frequency to plot. */
	const modes = $derived(
		result.modes
			.map((m, i) => ({ ...m, i }))
			.filter((m) => m.im >= 0 && m.freq_hz > 0.05 && Number.isFinite(m.damping))
	);
	/** Damping in percent along x, up to 30%: better damped modes sit on the right edge. */
	const CAP = 30;
	const dMin = $derived(Math.min(0, ...modes.map((m) => Math.floor(m.damping * 100))));
	const fMax = $derived(Math.ceil(Math.max(2, ...modes.map((m) => m.freq_hz))));
	const x = (pct: number) => L + ((Math.min(pct, CAP) - dMin) / (CAP - dMin)) * (W - L - R);
	const y = (f: number) => H - B - (f / fMax) * (H - B - T);
	/** Tick values, leaving out 0 when a negative minimum would crowd it. */
	const ticks = $derived(
		dMin < 0 && dMin > -4 ? [dMin, 5, 10, CAP] : [...new Set([dMin, 0, 5, 10, CAP])]
	);
	const table = $derived([...modes].sort((a, b) => a.damping - b.damping).slice(0, 12));
	const unstable = $derived(modes.filter((m) => m.re > 1e-6).length);
	const pct = (z: number) => `${(z * 100).toFixed(1)}%`;
	const stats = $derived([
		{
			label: 'Unstable modes',
			value: unstable.toLocaleString(),
			tone: unstable ? ('failure' as const) : undefined
		},
		{ label: 'Lowest damping', value: table.length ? pct(table[0].damping) : '—' }
	]);
</script>

<StatBoxes {stats} label="Modes" />
{#if modes.length}
	<figure>
		<svg
			viewBox="0 0 {W} {H}"
			role="img"
			aria-label="Modes by damping and frequency: {modes.length} modes, {unstable} unstable"
		>
			<text class="title" x={L} y="12">Frequency (Hz)</text>
			{#each [5, 10] as d (d)}
				<line class="guide" x1={x(d)} x2={x(d)} y1={T} y2={H - B} />
			{/each}
			<line class="axis" x1={L} x2={L} y1={T} y2={H - B} />
			<line class="axis" x1={L} x2={W - R} y1={H - B} y2={H - B} />
			{#each modes as m (m.i)}
				<circle
					class={['pt', { sel: selected === m.i, bad: m.re > 1e-6 }]}
					cx={x(m.damping * 100)}
					cy={y(m.freq_hz)}
					r={selected === m.i ? 4.5 : 3}
					role="button"
					tabindex="-1"
					aria-label="{m.freq_hz.toFixed(2)} Hz, damping {pct(m.damping)}"
					onclick={() => (selected = m.i)}
					onkeydown={(e) => e.key === 'Enter' && (selected = m.i)}
				/>
			{/each}
			<text class="tick" x={L - 5} y={y(fMax) + 4} text-anchor="end">{fMax}</text>
			<text class="tick" x={L - 5} y={y(0) + 4} text-anchor="end">0</text>
			{#each ticks as d (d)}
				<text
					class="tick"
					x={x(d)}
					y={H - B + 15}
					text-anchor={d === CAP ? 'end' : d === dMin && d < 0 ? 'start' : 'middle'}
					>{d}%{d === CAP ? '+' : ''}</text
				>
			{/each}
			<text class="title" x={(L + W - R) / 2} y={H - 6} text-anchor="middle">Damping</text>
		</svg>
	</figure>
	<table>
		<thead><tr><th>Frequency</th><th>Damping</th><th>Main generators</th></tr></thead>
		<tbody>
			{#each table as m (m.i)}
				<tr class={{ sel: selected === m.i }} onclick={() => (selected = m.i)}>
					<td class="num">{m.freq_hz.toFixed(2)} Hz</td>
					<td class={['num', { bad: m.re > 1e-6 }]}>{pct(m.damping)}</td>
					<td>{modeGenerators(m.participation)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/if}

<style>
	figure {
		margin: 0;
	}
	svg {
		display: block;
		width: 100%;
		height: auto;
		font-family: var(--font-sans);
	}
	.axis {
		stroke: var(--ink-faint);
		stroke-width: 1;
	}
	.guide {
		stroke: var(--amber);
		stroke-dasharray: 3 3;
	}
	.tick,
	.title {
		fill: var(--text-secondary);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}
	.pt {
		fill: var(--series-1);
		cursor: pointer;
	}
	.pt.bad {
		fill: var(--red);
	}
	.pt.sel {
		stroke: var(--ink);
		stroke-width: 1.5;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
	}
	th {
		font-weight: 600;
		color: var(--text-secondary);
		font-size: var(--fs-label);
	}
	th,
	td {
		text-align: left;
		padding: 4px 6px 4px 0;
		border-bottom: 1px solid var(--line);
		vertical-align: top;
	}
	.num {
		white-space: nowrap;
	}
	td.bad {
		color: var(--red);
	}
	tr.sel {
		background: var(--accent-soft);
	}
	tbody tr {
		cursor: pointer;
	}
</style>
