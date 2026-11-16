<script lang="ts" module>
	export interface ZoomSeries {
		key: string;
		label: string;
		x: ArrayLike<number>;
		y: ArrayLike<number>;
		/** Categorical slot 1–8. */
		slot: number;
		/** SVG dash pattern for a compared recording. */
		dash?: string;
	}
</script>

<script lang="ts">
	/** A zoomable line chart for the dynamics workbench. One y-axis; series in
	 * fixed categorical order (the caller keeps a chart to eight). Drag to zoom
	 * the x range, shift-drag to pan, wheel to zoom at the cursor, double-click
	 * to reset; the x range and the crosshair are bindable so stacked charts
	 * zoom and hover together. Points are decimated per pixel column to their
	 * first, min, max and last value, so spikes survive. */

	let {
		series,
		title = '',
		unit = '',
		height = 220,
		logX = false,
		xLabel = 'time [s]',
		xDomain = $bindable(null),
		hoverX = $bindable(null),
		events = [],
		cursor = null,
		digits = 4,
		showReset = true,
		onseek
	}: {
		series: ZoomSeries[];
		title?: string;
		unit?: string;
		height?: number;
		logX?: boolean;
		xLabel?: string;
		xDomain?: [number, number] | null;
		hoverX?: number | null;
		events?: number[];
		cursor?: number | null;
		digits?: number;
		/** False for a chart whose x range follows live data. */
		showReset?: boolean;
		onseek?: (x: number) => void;
	} = $props();

	let width = $state(600);
	const pad = { l: 60, r: 14, t: 10, b: 26 };
	const tx = (v: number) => (logX ? Math.log10(Math.max(v, 1e-12)) : v);
	const itx = (u: number) => (logX ? 10 ** u : u);

	const fullX = $derived.by((): [number, number] => {
		let lo = Infinity;
		let hi = -Infinity;
		for (const s of series) {
			if (s.x.length) {
				lo = Math.min(lo, s.x[0]);
				hi = Math.max(hi, s.x[s.x.length - 1]);
			}
		}
		return Number.isFinite(lo) ? [lo, hi > lo ? hi : lo + 1] : [0, 1];
	});
	const dom = $derived(xDomain ?? fullX);
	const u0 = $derived(tx(dom[0]));
	const u1 = $derived(tx(dom[1]));
	const plotW = $derived(Math.max(10, width - pad.l - pad.r));
	const plotH = $derived(height - pad.t - pad.b);
	const px = (v: number) => pad.l + ((tx(v) - u0) / (u1 - u0 || 1)) * plotW;

	function lowerBound(x: ArrayLike<number>, v: number): number {
		let lo = 0;
		let hi = x.length;
		while (lo < hi) {
			const mid = (lo + hi) >> 1;
			if (x[mid] < v) lo = mid + 1;
			else hi = mid;
		}
		return lo;
	}

	/** Visible, decimated points per series and the y extent over them. */
	const prepared = $derived.by(() => {
		let lo = Infinity;
		let hi = -Infinity;
		const out = series.map((s) => {
			const a = Math.max(0, lowerBound(s.x, dom[0]) - 1);
			const b = Math.min(s.x.length, lowerBound(s.x, dom[1]) + 1);
			const pts: [number, number][] = [];
			let col = -1;
			let first: [number, number] | null = null;
			let mn: [number, number] | null = null;
			let mx: [number, number] | null = null;
			let last: [number, number] | null = null;
			const flush = () => {
				const group = [first, mn, mx, last].filter((p): p is [number, number] => p !== null);
				group.sort((p, q) => p[0] - q[0]);
				for (const p of group) if (pts[pts.length - 1] !== p) pts.push(p);
			};
			for (let i = a; i < b; i++) {
				const yv = s.y[i];
				if (!Number.isFinite(yv)) continue;
				const xv = s.x[i];
				const c = Math.floor(px(xv));
				const p: [number, number] = [xv, yv];
				if (c !== col) {
					if (col !== -1) flush();
					col = c;
					first = mn = mx = last = p;
				} else {
					if (yv < mn![1]) mn = p;
					if (yv > mx![1]) mx = p;
					last = p;
				}
				if (xv >= dom[0] && xv <= dom[1]) {
					if (yv < lo) lo = yv;
					if (yv > hi) hi = yv;
				}
			}
			if (col !== -1) flush();
			return { s, pts };
		});
		if (!Number.isFinite(lo)) [lo, hi] = [0, 1];
		if (hi - lo < 1e-12 * Math.max(1, Math.abs(hi))) {
			const m = Math.max(1e-6, Math.abs(hi) * 1e-3);
			lo -= m;
			hi += m;
		} else {
			const m = (hi - lo) * 0.07;
			lo -= m;
			hi += m;
		}
		return { out, y: [lo, hi] as [number, number] };
	});
	const py = (v: number) =>
		pad.t + (1 - (v - prepared.y[0]) / (prepared.y[1] - prepared.y[0] || 1)) * plotH;

	function niceTicks(lo: number, hi: number, count: number): number[] {
		const span = hi - lo;
		if (!(span > 0)) return [lo];
		const raw = span / count;
		const mag = 10 ** Math.floor(Math.log10(raw));
		const step = [1, 2, 2.5, 5, 10].map((f) => f * mag).find((s) => span / s <= count) ?? 10 * mag;
		const out: number[] = [];
		for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) out.push(+v.toPrecision(12));
		return out;
	}
	const yTicks = $derived(niceTicks(prepared.y[0], prepared.y[1], Math.max(3, Math.floor(plotH / 45))));
	const xTicks = $derived.by(() => {
		if (!logX) return niceTicks(dom[0], dom[1], Math.max(4, Math.floor(plotW / 90)));
		const out: number[] = [];
		for (let e = Math.floor(u0); e <= Math.ceil(u1); e++) {
			for (const m of [1, 2, 5]) {
				const v = m * 10 ** e;
				if (v >= dom[0] && v <= dom[1]) out.push(v);
			}
		}
		return out;
	});

	function fmt(v: number, d = digits): string {
		if (!Number.isFinite(v)) return '–';
		const a = Math.abs(v);
		return a !== 0 && (a < 1e-3 || a >= 1e5) ? v.toExponential(2) : v.toFixed(d);
	}
	function fmtX(v: number): string {
		return logX ? (v >= 10 ? v.toFixed(1) : v >= 1 ? v.toFixed(2) : v.toPrecision(2)) : `${+v.toFixed(3)}`;
	}

	// ----- interaction -----
	let svgEl: SVGSVGElement | null = $state(null);
	let brush = $state<{ a: number; b: number } | null>(null);
	let pan: { startPx: number; dom: [number, number] } | null = null;

	function xAt(clientX: number): number {
		const rect = svgEl!.getBoundingClientRect();
		const u = u0 + ((clientX - rect.left - pad.l) / plotW) * (u1 - u0);
		return itx(u);
	}
	function down(e: PointerEvent) {
		if (e.button !== 0) return;
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
		if (e.shiftKey) pan = { startPx: e.clientX, dom: [dom[0], dom[1]] };
		else brush = { a: xAt(e.clientX), b: xAt(e.clientX) };
	}
	function moveP(e: PointerEvent) {
		hoverX = xAt(e.clientX);
		if (brush) brush = { a: brush.a, b: xAt(e.clientX) };
		if (pan) {
			const du = ((e.clientX - pan.startPx) / plotW) * (tx(pan.dom[1]) - tx(pan.dom[0]));
			xDomain = [itx(tx(pan.dom[0]) - du), itx(tx(pan.dom[1]) - du)];
		}
	}
	function up(e: PointerEvent) {
		if (brush) {
			const [a, b] = [Math.min(brush.a, brush.b), Math.max(brush.a, brush.b)];
			if (Math.abs(px(b) - px(a)) > 6) xDomain = [a, b];
			else onseek?.(brush.a);
		}
		brush = null;
		pan = null;
	}
	function wheel(node: SVGSVGElement) {
		const handler = (e: WheelEvent) => {
			e.preventDefault();
			const factor = e.deltaY > 0 ? 1.25 : 0.8;
			const c = tx(xAt(e.clientX));
			const a = c - (c - u0) * factor;
			const b = c + (u1 - c) * factor;
			const [fa, fb] = [tx(fullX[0]), tx(fullX[1])];
			xDomain = b - a >= fb - fa ? null : [itx(Math.max(fa, a)), itx(Math.min(fb, b))];
		};
		node.addEventListener('wheel', handler, { passive: false });
		return { destroy: () => node.removeEventListener('wheel', handler) };
	}
	function keydown(e: KeyboardEvent) {
		const span = u1 - u0;
		if (e.key === '+' || e.key === '=') xDomain = [itx(u0 + span * 0.15), itx(u1 - span * 0.15)];
		else if (e.key === '-') xDomain = null;
		else if (e.key === 'ArrowLeft') xDomain = [itx(u0 - span * 0.1), itx(u1 - span * 0.1)];
		else if (e.key === 'ArrowRight') xDomain = [itx(u0 + span * 0.1), itx(u1 + span * 0.1)];
		else return;
		e.preventDefault();
	}

	const readout = $derived.by(() => {
		if (hoverX == null || hoverX < dom[0] || hoverX > dom[1]) return null;
		return prepared.out.map(({ s }) => {
			const i = Math.min(s.x.length - 1, lowerBound(s.x, hoverX!));
			const j = i > 0 && Math.abs(s.x[i - 1] - hoverX!) < Math.abs(s.x[i] - hoverX!) ? i - 1 : i;
			return { s, v: s.y[j] };
		});
	});
</script>

<div class="zc" bind:clientWidth={width}>
	<div class="head">
		{#if title}<span class="title">{title}</span>{/if}
		{#if unit}<span class="unit">{unit}</span>{/if}
		{#if xDomain && showReset}<button class="reset" onclick={() => (xDomain = null)}>reset zoom</button>{/if}
	</div>
	<svg
		bind:this={svgEl}
		use:wheel
		{width}
		{height}
		role="slider"
		tabindex="0"
		aria-label={`${title || series.map((s) => s.label).join(', ')}: drag to zoom, shift-drag to pan, double-click to reset, plus and minus keys zoom`}
		aria-valuenow={hoverX ?? dom[0]}
		aria-valuemin={dom[0]}
		aria-valuemax={dom[1]}
		onpointerdown={down}
		onpointermove={moveP}
		onpointerup={up}
		onpointerleave={() => (hoverX = null)}
		ondblclick={() => (xDomain = null)}
		onkeydown={keydown}
	>
		<defs>
			<clipPath id={`clip-${title}-${unit}`.replace(/[^a-zA-Z0-9-]/g, '_')}>
				<rect x={pad.l} y={pad.t} width={plotW} height={plotH} />
			</clipPath>
		</defs>
		{#each yTicks as v, ti (ti)}
			<line class="grid" x1={pad.l} x2={pad.l + plotW} y1={py(v)} y2={py(v)} />
			<text class="tick" x={pad.l - 6} y={py(v) + 3.5} text-anchor="end">{fmt(v, Math.min(digits, 4))}</text>
		{/each}
		{#each xTicks as v, ti (ti)}
			<line class="grid v" x1={px(v)} x2={px(v)} y1={pad.t} y2={pad.t + plotH} />
			<text class="tick" x={px(v)} y={height - 8} text-anchor="middle">{fmtX(v)}</text>
		{/each}
		<g clip-path={`url(#${`clip-${title}-${unit}`.replace(/[^a-zA-Z0-9-]/g, '_')})`}>
			{#each events as ev, i (i)}
				<line class="event" x1={px(ev)} x2={px(ev)} y1={pad.t} y2={pad.t + plotH} />
			{/each}
			{#each prepared.out as { s, pts } (s.key)}
				<path
					class="line"
					d={pts.map((p, i) => `${i ? 'L' : 'M'}${px(p[0]).toFixed(1)},${py(p[1]).toFixed(1)}`).join('')}
					style={`stroke: var(--series-${((s.slot - 1) % 6) + 1})`}
					stroke-dasharray={s.dash ?? null}
				/>
			{/each}
			{#if cursor != null}
				<line class="cursor" x1={px(cursor)} x2={px(cursor)} y1={pad.t} y2={pad.t + plotH} />
			{/if}
			{#if readout && hoverX != null}
				<line class="cross" x1={px(hoverX)} x2={px(hoverX)} y1={pad.t} y2={pad.t + plotH} />
				{#each readout as r (r.s.key)}
					{#if Number.isFinite(r.v)}
						<circle class="dot" cx={px(hoverX)} cy={py(r.v)} r="3.5" style={`fill: var(--series-${((r.s.slot - 1) % 6) + 1})`} />
					{/if}
				{/each}
			{/if}
			{#if brush}
				<rect
					class="brush"
					x={Math.min(px(brush.a), px(brush.b))}
					y={pad.t}
					width={Math.abs(px(brush.b) - px(brush.a))}
					height={plotH}
				/>
			{/if}
		</g>
		<text class="xl" x={pad.l + plotW} y={height - 8} text-anchor="end" dy="-12">{xLabel}</text>
	</svg>
	{#if readout && hoverX != null}
		<div class="tip" style={`left: ${Math.min(px(hoverX) + 12, width - 210)}px`}>
			<div class="tip-x">{logX ? `${fmtX(hoverX)} Hz` : `t = ${hoverX.toFixed(3)} s`}</div>
			{#each readout as r (r.s.key)}
				<div class="tip-row">
					<i style={`background: var(--series-${((r.s.slot - 1) % 6) + 1})`} class:dashed={!!r.s.dash}></i>
					<span>{r.s.label}</span><b>{fmt(r.v)}</b>
				</div>
			{/each}
		</div>
	{/if}
	{#if series.length > 1}
		<ul class="legend">
			{#each series as s (s.key)}
				<li><i style={`background: var(--series-${((s.slot - 1) % 6) + 1})`} class:dashed={!!s.dash}></i>{s.label}</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.zc {
		position: relative;
		width: 100%;
		--dyn-series-1: #2a78d6;
		--dyn-series-2: #eb6834;
		--dyn-series-3: #1baf7a;
		--dyn-series-4: #eda100;
		--dyn-series-5: #e87ba4;
		--dyn-series-6: #008300;
		--dyn-series-7: #6250d6;
		--dyn-series-8: #e34948;
	}
	:global(:root[data-theme='dark']) .zc {
		--dyn-series-1: #3987e5;
		--dyn-series-2: #d95926;
		--dyn-series-3: #199e70;
		--dyn-series-4: #c98500;
		--dyn-series-5: #d55181;
		--dyn-series-6: #008300;
		--dyn-series-7: #9085e9;
		--dyn-series-8: #e66767;
	}
	.head {
		display: flex;
		gap: 8px;
		align-items: baseline;
		padding-left: 60px;
		min-height: 18px;
	}
	.title {
		font-size: var(--fs-label);
		font-weight: 500;
		color: var(--text-primary);
	}
	.unit {
		font-size: var(--fs-control);
		color: var(--text-tertiary);
	}
	.reset {
		margin-left: auto;
		border: none;
		background: none;
		color: var(--text-accent);
		cursor: pointer;
		font-size: var(--fs-control);
	}
	svg {
		display: block;
		cursor: crosshair;
		touch-action: none;
		user-select: none;
	}
	svg:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.grid {
		stroke: var(--border-default);
		stroke-width: 0.6;
	}
	.grid.v {
		stroke-dasharray: 2 3;
	}
	.tick,
	.xl {
		font-size: 10px;
		fill: var(--text-tertiary);
		font-family: var(--font-mono);
	}
	.line {
		fill: none;
		stroke-width: 1.7;
		stroke-linejoin: round;
	}
	.event {
		stroke: var(--text-tertiary);
		stroke-dasharray: 2 3;
	}
	.cursor {
		stroke: var(--accent);
		stroke-width: 1.4;
	}
	.cross {
		stroke: var(--text-secondary);
		stroke-width: 0.8;
	}
	.dot {
		stroke: var(--surface-menu);
		stroke-width: 2;
	}
	.brush {
		fill: var(--accent-soft);
		stroke: var(--accent);
		stroke-width: 1;
	}
	.tip {
		position: absolute;
		top: 24px;
		min-width: 190px;
		padding: 6px 8px;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		font-size: 11px;
		pointer-events: none;
		box-shadow: 0 2px 8px rgb(0 0 0 / 0.14);
		z-index: 3;
	}
	.tip-x {
		color: var(--text-secondary);
		font-family: var(--font-mono);
		margin-bottom: 3px;
	}
	.tip-row {
		display: grid;
		grid-template-columns: 12px 1fr auto;
		gap: 6px;
		align-items: center;
		color: var(--text-primary);
	}
	.tip-row b {
		font-family: var(--font-mono);
		font-weight: 500;
	}
	i {
		width: 12px;
		height: 3px;
		border-radius: var(--radius-xs);
		display: inline-block;
	}
	i.dashed {
		background-image: linear-gradient(90deg, transparent 50%, var(--surface-menu) 50%) !important;
		background-size: 6px 3px;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 3px 14px;
		list-style: none;
		margin: 2px 0 0;
		padding: 0 0 0 60px;
		font-size: 11px;
		color: var(--text-secondary);
	}
	.legend li {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}
</style>
