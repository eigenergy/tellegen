<script lang="ts" module>
	export interface TraceSeries {
		key: string;
		label: string;
		data: ArrayLike<number>;
		/** A CSS custom property (`--series-1`) or a colour. */
		color: string;
		width?: number;
		alpha?: number;
	}
	export interface TraceBand {
		lo: ArrayLike<number>;
		hi: ArrayLike<number>;
		color: string;
		label: string;
	}
</script>

<script lang="ts">
	import { minSpanFor, tickLabel, ticks, yDomain } from '../../dynamics/axis.js';
	import { m4 } from '../../dynamics/decimate.js';
	import { sampleAt } from '../../dynamics/series.js';

	let {
		time,
		series,
		band = null,
		refLines = [],
		unit,
		xRange,
		playhead,
		highlight = null,
		ariaLabel,
		height = 200,
		onseek,
		onhover,
		onpick,
		onzoom
	}: {
		time: Float64Array;
		series: TraceSeries[];
		band?: TraceBand | null;
		refLines?: { value: number; label: string }[];
		unit: string;
		xRange: [number, number];
		playhead: number;
		highlight?: string | null;
		ariaLabel: string;
		height?: number;
		onseek?: (t: number) => void;
		onhover?: (key: string | null) => void;
		onpick?: (key: string) => void;
		onzoom?: (range: [number, number]) => void;
	} = $props();

	let wrap = $state.raw<HTMLDivElement | null>(null);
	let canvas = $state.raw<HTMLCanvasElement | null>(null);
	let width = $state(0);
	let themeTick = $state(0);
	let hoverX = $state<number | null>(null);
	let drag = $state<{ x0: number; x1: number } | null>(null);
	const PAD_L = 44;
	const PAD_R = 8;
	/** Room above the plot for the unit, which heads the y ticks. */
	const PAD_T = 22;
	const PAD_B = 18;
	const plotW = $derived(Math.max(10, width - PAD_L - PAD_R));
	const plotH = $derived(Math.max(10, height - PAD_T - PAD_B));

	// The y range over what is visible.
	const yRange = $derived.by((): [number, number] => {
		const [x0, x1] = xRange;
		let lo = Infinity;
		let hi = -Infinity;
		const scan = (d: ArrayLike<number>) => {
			for (let i = 0; i < time.length; i++) {
				if (time[i] < x0 || time[i] > x1) continue;
				const v = d[i];
				if (!Number.isFinite(v)) continue;
				if (v < lo) lo = v;
				if (v > hi) hi = v;
			}
		};
		for (const s of series) scan(s.data);
		if (band) {
			scan(band.lo);
			scan(band.hi);
		}
		for (const r of refLines) {
			if (r.value >= lo - 0.2 * (hi - lo) && r.value <= hi + 0.2 * (hi - lo)) {
				lo = Math.min(lo, r.value);
				hi = Math.max(hi, r.value);
			}
		}
		return yDomain(lo, hi, minSpanFor(unit));
	});

	const xOf = (t: number) => PAD_L + ((t - xRange[0]) / (xRange[1] - xRange[0])) * plotW;
	const yOf = (v: number) => PAD_T + (1 - (v - yRange[0]) / (yRange[1] - yRange[0])) * plotH;
	const tOf = (x: number) => xRange[0] + ((x - PAD_L) / plotW) * (xRange[1] - xRange[0]);

	const yTicks = $derived(ticks(yRange[0], yRange[1], 4));
	const xTicks = $derived(ticks(xRange[0], xRange[1], Math.max(2, Math.floor(plotW / 80))));
	const fmt = tickLabel;

	function cssColor(c: string): string {
		if (!c.startsWith('--') || !wrap) return c;
		return getComputedStyle(wrap).getPropertyValue(c).trim() || '#888';
	}

	// Redraw on data, size, zoom, highlight or theme; never on playback.
	$effect(() => {
		void themeTick;
		const cv = canvas;
		if (!cv || width <= 0) return;
		const dpr = globalThis.devicePixelRatio || 1;
		cv.width = Math.round(width * dpr);
		cv.height = Math.round(height * dpr);
		const g = cv.getContext('2d');
		if (!g) return;
		g.setTransform(dpr, 0, 0, dpr, 0, 0);
		g.clearRect(0, 0, width, height);
		g.save();
		g.beginPath();
		g.rect(PAD_L, PAD_T, plotW, plotH);
		g.clip();
		const grid = cssColor('--line');
		g.strokeStyle = grid;
		g.lineWidth = 1;
		for (const v of yTicks) {
			const y = Math.round(yOf(v)) + 0.5;
			g.beginPath();
			g.moveTo(PAD_L, y);
			g.lineTo(PAD_L + plotW, y);
			g.stroke();
		}
		for (const r of refLines) {
			g.setLineDash([4, 3]);
			g.strokeStyle = cssColor('--warn');
			const y = Math.round(yOf(r.value)) + 0.5;
			g.beginPath();
			g.moveTo(PAD_L, y);
			g.lineTo(PAD_L + plotW, y);
			g.stroke();
			g.setLineDash([]);
		}
		if (band) {
			const idx = m4(time, band.lo, plotW, xRange[0], xRange[1]);
			const idxHi = m4(time, band.hi, plotW, xRange[0], xRange[1]);
			g.fillStyle = cssColor(band.color);
			g.globalAlpha = 0.18;
			g.beginPath();
			idxHi.forEach((i, k) =>
				k ? g.lineTo(xOf(time[i]), yOf(band.hi[i])) : g.moveTo(xOf(time[i]), yOf(band.hi[i]))
			);
			for (let k = idx.length - 1; k >= 0; k--) g.lineTo(xOf(time[idx[k]]), yOf(band.lo[idx[k]]));
			g.closePath();
			g.fill();
			g.globalAlpha = 1;
		}
		const ordered = [...series].sort(
			(a, b) => (a.key === highlight ? 1 : 0) - (b.key === highlight ? 1 : 0)
		);
		for (const s of ordered) {
			// -1 marks a gap (a dead bus, a tripped unit): lift the pen there.
			const idx = m4(time, s.data, plotW, xRange[0], xRange[1], true);
			if (!idx.length) continue;
			g.strokeStyle = cssColor(s.color);
			const dim = highlight !== null && s.key !== highlight;
			g.globalAlpha = dim ? Math.min(0.25, s.alpha ?? 1) : (s.alpha ?? 1);
			g.lineWidth = s.key === highlight ? (s.width ?? 1.5) + 1.5 : (s.width ?? 1.5);
			g.lineJoin = 'round';
			g.beginPath();
			let pen = false;
			for (const i of idx) {
				if (i < 0) {
					pen = false;
					continue;
				}
				const x = xOf(time[i]);
				const y = yOf(s.data[i]);
				if (pen) g.lineTo(x, y);
				else g.moveTo(x, y);
				pen = true;
			}
			g.stroke();
		}
		g.globalAlpha = 1;
		g.restore();
	});

	$effect(() => {
		if (typeof MutationObserver === 'undefined') return;
		const mo = new MutationObserver(() => themeTick++);
		mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
		const mq = matchMedia('(prefers-color-scheme: dark)');
		const bump = () => themeTick++;
		mq.addEventListener('change', bump);
		return () => {
			mo.disconnect();
			mq.removeEventListener('change', bump);
		};
	});

	/** The series nearest the pointer at `x, y` (pixels). */
	function nearest(x: number, y: number): string | null {
		const { i } = sampleAt(time, tOf(x));
		let best: string | null = null;
		let dist = 12;
		for (const s of series) {
			const v = s.data[i];
			if (!Number.isFinite(v)) continue;
			const d = Math.abs(yOf(v) - y);
			if (d < dist) {
				dist = d;
				best = s.key;
			}
		}
		return best;
	}

	const local = (e: PointerEvent | MouseEvent) => {
		const r = canvas!.getBoundingClientRect();
		return { x: e.clientX - r.left, y: e.clientY - r.top };
	};

	const playheadX = $derived(xOf(Math.min(Math.max(playhead, xRange[0]), xRange[1])));
	const cursorT = $derived(hoverX === null ? null : tOf(hoverX));
	const step = $derived(time.length > 1 ? time[1] - time[0] : 0.01);
</script>

<div
	class="chart"
	bind:this={wrap}
	bind:clientWidth={width}
	style:height="{height}px"
	role="img"
	aria-label={ariaLabel}
>
	<canvas
		bind:this={canvas}
		style:width="{width}px"
		style:height="{height}px"
		tabindex="0"
		aria-label="{ariaLabel}. Left and right arrows move the time; Enter pins the nearest series."
		onpointermove={(e) => {
			const p = local(e);
			hoverX = p.x >= PAD_L && p.x <= PAD_L + plotW ? p.x : null;
			if (drag) drag = { ...drag, x1: p.x };
			onhover?.(hoverX === null ? null : nearest(p.x, p.y));
		}}
		onpointerleave={() => {
			hoverX = null;
			onhover?.(null);
		}}
		onpointerdown={(e) => {
			const p = local(e);
			if (p.x >= PAD_L) drag = { x0: p.x, x1: p.x };
		}}
		onpointerup={(e) => {
			const p = local(e);
			const d = drag;
			drag = null;
			if (d && Math.abs(d.x1 - d.x0) > 6) {
				const a = tOf(Math.min(d.x0, d.x1));
				const b = tOf(Math.max(d.x0, d.x1));
				onzoom?.([Math.max(time[0], a), Math.min(time[time.length - 1], b)]);
				return;
			}
			const key = nearest(p.x, p.y);
			if (key) onpick?.(key);
			else onseek?.(tOf(p.x));
		}}
		onkeydown={(e) => {
			if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
				e.preventDefault();
				const d = (e.shiftKey ? 0.1 : step) * (e.key === 'ArrowLeft' ? -1 : 1);
				onseek?.(playhead + d);
			} else if (e.key === 'Enter') {
				const { i } = sampleAt(time, playhead);
				let best: string | null = null;
				let bd = Infinity;
				for (const s of series) {
					const v = s.data[i];
					const mid = (yRange[0] + yRange[1]) / 2;
					if (Number.isFinite(v) && Math.abs(v - mid) < bd) {
						bd = Math.abs(v - mid);
						best = s.key;
					}
				}
				if (best) onpick?.(best);
			}
		}}
	></canvas>
	<div class="axes" aria-hidden="true">
		{#each yTicks as v (v)}
			<span class="ytick" style:top="{yOf(v) - 7}px" style:width="{PAD_L - 4}px"
				>{fmt(v, yTicks[1] - yTicks[0])}</span
			>
		{/each}
		{#each xTicks as v (v)}
			<span class="xtick" style:left="{xOf(v)}px" style:top="{PAD_T + plotH + 2}px"
				>{fmt(v, xTicks[1] - xTicks[0])} s</span
			>
		{/each}
		<span class="unit">{unit}</span>
		{#each refLines as r (r.label)}
			{#if r.value >= yRange[0] && r.value <= yRange[1]}
				<span class="ref" style:top="{yOf(r.value) - 12}px" style:right="{PAD_R + 2}px"
					>{r.label}</span
				>
			{/if}
		{/each}
		<div
			class="playhead"
			style:transform="translateX({playheadX}px)"
			style:top="{PAD_T}px"
			style:height="{plotH}px"
		></div>
		{#if hoverX !== null && cursorT !== null}
			<div
				class="cross"
				style:transform="translateX({hoverX}px)"
				style:top="{PAD_T}px"
				style:height="{plotH}px"
			></div>
			<span class="tcur" style:left="{hoverX + 4}px" style:top="{PAD_T}px"
				>{cursorT.toFixed(3)} s</span
			>
		{/if}
		{#if drag}
			<div
				class="sel"
				style:left="{Math.min(drag.x0, drag.x1)}px"
				style:width="{Math.abs(drag.x1 - drag.x0)}px"
				style:top="{PAD_T}px"
				style:height="{plotH}px"
			></div>
		{/if}
	</div>
</div>

<style>
	.chart {
		position: relative;
		width: 100%;
	}
	canvas {
		display: block;
		cursor: crosshair;
		touch-action: pan-y;
	}
	canvas:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 1px;
	}
	.axes {
		position: absolute;
		inset: 0;
		pointer-events: none;
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--text-tertiary);
	}
	.ytick {
		position: absolute;
		left: 0;
		text-align: right;
	}
	.xtick {
		position: absolute;
		transform: translateX(-50%);
		white-space: nowrap;
	}
	/* The unit heads the y ticks, above the plot. */
	.unit {
		position: absolute;
		top: 0;
		left: 0;
		color: var(--text-secondary);
		font-family: var(--font-sans);
		white-space: nowrap;
	}
	.ref {
		position: absolute;
		color: var(--text-secondary);
	}
	.playhead,
	.cross {
		position: absolute;
		left: 0;
		width: 1px;
		will-change: transform;
	}
	.playhead {
		background: var(--accent);
		box-shadow: 0 0 0 0.5px var(--accent);
	}
	.cross {
		background: rgb(var(--ink-rgb) / 0.35);
	}
	.tcur {
		position: absolute;
		color: var(--text-secondary);
	}
	.sel {
		position: absolute;
		background: var(--accent-soft);
		border: 1px solid var(--accent);
	}
</style>
