<script lang="ts">
	/** Live samples in Tellegen's plot dock.
	 * @block Live plots */
	import { untrack } from 'svelte';
	import { getAppState, getController } from '../../context.svelte.js';
	import ChannelPicker from '../ChannelPicker.svelte';
	import { channelLabel, quantityOf } from '../../dynamics/quantities.js';
	import ZoomChart, { type ZoomSeries } from '../ZoomChart.svelte';

	let { dockHeight = $bindable(0), flush = false }: { dockHeight?: number; flush?: boolean } = $props();

	const app = getAppState();
	const dyn = getController().dynamics;
	const live = dyn.live;
	const ins = $derived(dyn.active?.inspection ?? null);
	const run = $derived(live.run);
	const base = $derived(ins?.base_mva ?? 100);
	let collapsed = $state(false);

	// ----- dock height: drag the top edge; remembered per browser -----
	const HEIGHT_KEY = 'tellegen.live.dockHeight';
	const MIN_DOCK = 180;
	const maxDock = () => (typeof window === 'undefined' ? 2000 : Math.max(MIN_DOCK, window.innerHeight - 90));
	const clampDock = (h: number) => Math.round(Math.min(maxDock(), Math.max(MIN_DOCK, h)));
	let dockPx = $state<number | null>(
		(() => {
			try {
				// A height stored in a taller window is clamped to this one.
				const v = Number(localStorage.getItem(HEIGHT_KEY));
				return Number.isFinite(v) && v >= MIN_DOCK ? clampDock(v) : null;
			} catch {
				return null;
			}
		})()
	);
	function saveDock() {
		try {
			if (dockPx === null) localStorage.removeItem(HEIGHT_KEY);
			else localStorage.setItem(HEIGHT_KEY, String(dockPx));
		} catch {
			/* storage unavailable: the height lasts for this page only */
		}
	}
	let sectionEl = $state<HTMLElement | null>(null);
	let drag: { y: number; h: number } | null = null;
	function gripDown(e: PointerEvent) {
		if (e.button !== 0 || !sectionEl) return;
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
		drag = { y: e.clientY, h: sectionEl.getBoundingClientRect().height };
		e.preventDefault();
	}
	function gripMove(e: PointerEvent) {
		if (drag) dockPx = clampDock(drag.h + (drag.y - e.clientY));
	}
	function gripUp() {
		if (!drag) return;
		drag = null;
		saveDock();
	}
	function gripKey(e: KeyboardEvent) {
		const h = dockPx ?? sectionEl?.getBoundingClientRect().height ?? 400;
		if (e.key === 'ArrowUp') dockPx = clampDock(h + 40);
		else if (e.key === 'ArrowDown') dockPx = clampDock(h - 40);
		else if (e.key === 'Home') dockPx = clampDock(maxDock());
		else if (e.key === 'End') dockPx = MIN_DOCK;
		else return;
		e.preventDefault();
		saveDock();
	}
	// A smaller window keeps the dock inside it.
	$effect(() => {
		const onResize = () => {
			if (dockPx !== null && dockPx > maxDock()) dockPx = clampDock(dockPx);
		};
		window.addEventListener('resize', onResize);
		return () => window.removeEventListener('resize', onResize);
	});

	// ----- chart grid: fill the plot area -----
	const GAP = 6;
	const MIN_CHART_W = 420;
	let chartsW = $state(0);
	let chartsH = $state(0);

	// ----- charts: one per quantity -----
	let hoverX = $state<number | null>(null);
	let xDomain = $state<[number, number] | null>(null);
	const charts = $derived.by(() => {
		if (!run) return [];
		const groups = new Map<string, string[]>();
		for (const k of live.plotted) {
			const q = quantityOf(k)?.q;
			if (!q) continue;
			groups.set(q, [...(groups.get(q) ?? []), k]);
		}
		return [...groups.entries()].map(([q, keys]) => {
			const x = quantityOf(keys[0])!;
			const series: ZoomSeries[] = [];
			keys.forEach((k, i) => {
				const s = run.series(k);
				if (!s) return;
				const y = Array.from(s, (v) => x.scale(v, base));
				series.push({ key: k, label: channelLabel(k), x: run.time, y, slot: i + 1 });
			});
			return {
				q,
				title: x.title,
				unit: x.unit,
				digits: x.digits,
				series
			};
		});
	});

	/** Columns and chart height so the charts fill the plot area: as many
	 * columns as fit at {@link MIN_CHART_W}, rows sharing the height. */
	const grid = $derived.by(() => {
		const n = Math.max(1, charts.length);
		const cols = Math.max(1, Math.min(n, Math.floor((chartsW + GAP) / (MIN_CHART_W + GAP))));
		const rows = Math.ceil(n / cols);
		// Title row and legend line around each chart's plot.
		const over = 22 + (charts.some((c) => c.series.length > 1) ? 22 : 0);
		const h = Math.floor((chartsH - GAP * (rows - 1)) / rows) - over;
		return { cols, height: Math.max(120, Math.min(h, 1200)) };
	});

	const keys = $derived(run?.keys);
	const channels = $derived((keys ?? []).flatMap((key) => {
		const q = quantityOf(key);
		return q ? [{ key, label: channelLabel(key), detail: `${q.title} (${q.unit})` }] : [];
	}));

	$effect(() => {
		const bus = app.selectedBus;
		if (bus === null || !ins?.buses.includes(bus)) return;
		untrack(() => {
			const key = `vm:${bus}`;
			if (!live.plotted.includes(key)) live.plotted = [...live.plotted, key];
		});
	});
</script>

<section
	class={['wb', 'dyn-form', { collapsed, compact: app.compactLayout, flush }]}
	aria-label="Live simulation plots"
	data-testid="dyn-workbench"
	bind:clientHeight={dockHeight}
	bind:this={sectionEl}
	style:height={!app.compactLayout && !collapsed && dockPx !== null ? `${dockPx}px` : null}
>
	{#if !collapsed && !app.compactLayout}
		<div
			class="grip"
			role="slider"
			aria-orientation="vertical"
			aria-label="Plot height"
			aria-valuemin={MIN_DOCK}
			aria-valuemax={maxDock()}
			aria-valuenow={dockPx ?? Math.round(dockHeight)}
			tabindex="0"
			title="Drag to resize; double-click for the default height"
			onpointerdown={gripDown}
			onpointermove={gripMove}
			onpointerup={gripUp}
			onpointercancel={gripUp}
			ondblclick={() => {
				dockPx = null;
				saveDock();
			}}
			onkeydown={gripKey}
		></div>
	{/if}
	<header>
		<strong>Live plots</strong>
		{#if live.running}
			<button onclick={() => live.pause()} data-testid="wb-pause">Pause</button>
		{:else}
			<button class="primary" disabled={live.starting} onclick={() => dyn.startLive()} data-testid="wb-start"
				>{live.active ? 'Resume' : 'Start'}</button
			>
		{/if}
		<button class="ghost" disabled={!live.active} onclick={() => live.stop()}>Stop</button>
		<output class="mono">t = {live.t.toFixed(2)} s</output>
		{#if live.perf}
			<span class="dim" data-testid="wb-perf" title="Simulated seconds per second of worker compute">Engine {live.perf.engineX.toFixed(1)}×</span>
		{/if}
		<span class="grow"></span>
		<button class="ghost" onclick={() => (collapsed = !collapsed)}
			>{collapsed ? 'Show plots' : 'Hide plots'}</button
		>
		<button class="ghost" onclick={() => live.close()} aria-label="Close live simulation">×</button>
	</header>
	{#if live.error}<p class="err" role="alert">{live.error}</p>{/if}
	{#if !collapsed}
		<div class="body">
			<div class="plots">
				<ChannelPicker {channels} selected={live.plotted} ontoggle={(key) => live.toggle(key)} onclear={() => (live.plotted = [])} />
				{#if !run}
					<p class="dim">Waiting for samples…</p>
				{:else if !live.plotted.length}
					<p class="dim">Choose a channel to plot.</p>
				{:else}
					<div
						class="charts"
						bind:clientWidth={chartsW}
						bind:clientHeight={chartsH}
						style:grid-template-columns={`repeat(${grid.cols}, minmax(0, 1fr))`}
					>
						{#each charts as c (c.q)}
							<ZoomChart
								series={c.series}
								title={c.title}
								unit={c.unit}
								digits={c.digits}
								height={grid.height}
								xLabel="time [s]"
								showReset={false}
								follow={live.windowS}
								bind:xDomain
								bind:hoverX
							/>
						{/each}
					</div>
				{/if}
			</div>

		</div>
	{/if}
</section>

<style>
	.wb {
		position: fixed;
		left: var(--tellegen-dock-left, 12px);
		right: var(--tellegen-dock-right, 12px);
		bottom: 12px;
		height: min(38vh, 420px);
		z-index: var(--z-dock);
		display: flex;
		flex-direction: column;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		box-shadow: var(--elev-2);
		color: var(--text-primary);
		font-size: var(--fs-control);
	}
	.wb.compact { left: 8px; right: 8px; bottom: 40px; max-height: 38dvh; }
	.wb.compact.flush { left: 0; right: 0; bottom: 0; border-radius: 0; }
	.wb.collapsed {
		height: auto;
	}
	.grip {
		position: absolute;
		top: -5px;
		left: 0;
		right: 0;
		height: 10px;
		cursor: ns-resize;
		touch-action: none;
		z-index: 1;
	}
	.grip::after {
		content: '';
		position: absolute;
		top: 3px;
		left: 50%;
		width: 44px;
		height: 4px;
		margin-left: -22px;
		border-radius: 2px;
		background: var(--border-default);
	}
	.grip:hover::after,
	.grip:focus-visible::after {
		background: var(--accent);
	}
	.grip:focus-visible {
		outline: none;
	}
	header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		border-bottom: 1px solid var(--border-default);
		flex-wrap: wrap;
	}
	.grow {
		flex: 1;
	}
	.body {
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}
	.plots {
		min-width: 0;
		min-height: 0;
		padding: 6px 10px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.plots > :not(.charts) {
		flex: none;
	}
	header output {
		padding: 0 4px;
		color: var(--text-secondary);
	}
	.charts {
		flex: 1 1 0;
		min-height: 0;
		overflow: auto;
		display: grid;
		align-content: start;
		gap: 6px;
	}
	.mono {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
	}
	.dim {
		color: var(--text-tertiary);
	}
	.err {
		color: var(--text-danger);
		margin: 4px 10px;
	}
</style>
