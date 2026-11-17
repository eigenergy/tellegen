<script lang="ts">
	/** The live workbench (experimental), docked at the bottom while the live
	 * view runs: plots of any channels (one chart per quantity, several
	 * series each, a shared time axis and hover) and, beside them, the live
	 * controls and the disturbances applied at the moment they are picked:
	 * bus faults, line trips, closes and faults, load steps, unit trips and
	 * the units' voltage and power references. A bus clicked on the map is
	 * the one the bus actions use, and its voltage and frequency are plotted. */
	import type { RmsEvent } from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import { SPEEDS, STEPS, WINDOWS } from '../../dynamics/live.svelte.js';
	import { channelLabel, QUANTITIES, quantityOf, type Owner } from '../../dynamics/quantities.js';
	import ZoomChart, { type ZoomSeries } from './ZoomChart.svelte';

	let { dockHeight = $bindable(0) }: { dockHeight?: number } = $props();

	const app = getAppState();
	const dyn = getController().dynamics;
	const live = dyn.live;
	const ins = $derived(dyn.active?.inspection ?? null);
	const run = $derived(live.run);
	const base = $derived(ins?.base_mva ?? 100);
	const f0 = $derived(ins?.base_frequency ?? 60);
	let collapsed = $state(false);

	// ----- dock height: drag the top edge; remembered per browser -----
	const HEIGHT_KEY = 'tellegen.live.dockHeight';
	const MIN_DOCK = 180;
	const maxDock = () => (typeof window === 'undefined' ? 2000 : Math.max(MIN_DOCK, window.innerHeight - 90));
	const clampDock = (h: number) => Math.round(Math.min(maxDock(), Math.max(MIN_DOCK, h)));
	let dockPx = $state<number | null>(
		(() => {
			try {
				const v = Number(localStorage.getItem(HEIGHT_KEY));
				return Number.isFinite(v) && v >= MIN_DOCK ? v : null;
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

	// ----- speed -----
	const perf = $derived(live.perf);
	const fmtX = (x: number) =>
		!Number.isFinite(x) ? '∞×' : x >= 100 ? `${x.toFixed(0)}×` : `${x.toFixed(1)}×`;
	const spark = $derived.by(() => {
		const h = live.perfHistory.filter(Number.isFinite);
		if (h.length < 2) return '';
		const hi = Math.max(...h, 1);
		return h
			.map((v, i) => `${((i / (h.length - 1)) * 220).toFixed(1)},${(35 - (v / hi) * 33).toFixed(1)}`)
			.join(' ');
	});
	const perfTitle = $derived(
		perf
			? `Engine: ${fmtX(perf.engineX)} real time, simulated seconds per second the wasm engine computes (the last second). With the transfer to the page: ${fmtX(perf.roundX)}. ${Number.isFinite(live.speed) ? `At ${live.speed}× the round trips use ${Math.round(perf.load * 100)} % of the wall-clock budget; the rest is headroom.` : 'Unpaced: it runs as fast as it can.'}`
			: ''
	);

	const unitKey = (bus: number, id: string) => (id.trim() === '1' ? `${bus}` : `${bus}#${id.trim()}`);

	// ----- default plots -----
	$effect(() => {
		if (!ins || live.plotted.length) return;
		const gens = ins.generators.slice(0, 3);
		live.plotted = [
			...new Set([
				'coi:f',
				...gens.map((g) => `vm:${g.bus}`),
				...gens.map((g) => `omega:${unitKey(g.bus, g.machine_id)}`),
				...gens.map((g) => `pe:${unitKey(g.bus, g.machine_id)}`)
			])
		];
	});

	// ----- charts: one per quantity -----
	let hoverX = $state<number | null>(null);
	let xDomain = $state<[number, number] | null>(null);
	const charts = $derived.by(() => {
		if (!run) return [];
		const groups = new Map<string, string[]>();
		for (const k of live.plotted) {
			const q = k.split(':')[0];
			if (!quantityOf(k)) continue;
			groups.set(q, [...(groups.get(q) ?? []), k]);
		}
		const coiD = run.series('coi:delta');
		return [...groups.entries()].map(([q, keys]) => {
			const x = quantityOf(keys[0])!;
			const series: ZoomSeries[] = [];
			keys.forEach((k, i) => {
				const s = run.series(k);
				if (!s) return;
				let y: ArrayLike<number>;
				if (q === 'delta' && coiD) {
					const d = new Float64Array(s.length);
					for (let j = 0; j < s.length; j++) d[j] = ((s[j] - coiD[j]) * 180) / Math.PI;
					y = d;
				} else y = Array.from(s, (v) => x.scale(v, base));
				series.push({ key: k, label: channelLabel(k), x: run.time, y, slot: i + 1 });
			});
			return {
				q,
				title: q === 'delta' ? 'rotor angle relative to the COI' : x.title,
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

	// ----- channel picker -----
	let search = $state('');
	interface Item {
		owner: Owner;
		id: string;
		name: string;
		hint: string;
	}
	const items = $derived.by((): Item[] => {
		if (!ins) return [];
		const units: Item[] = ins.generators.map((g) => ({
			owner: 'unit',
			id: unitKey(g.bus, g.machine_id),
			name: `G${g.bus}${g.machine_id.trim() === '1' ? '' : ` '${g.machine_id.trim()}'`}`,
			hint: g.models.filter((m) => m && m !== 'none').join(' ')
		}));
		const buses: Item[] = ins.buses.map((b) => ({ owner: 'bus', id: String(b), name: `bus ${b}`, hint: '' }));
		const branches: Item[] = ins.branches.map(([a, b, c]) => ({
			owner: 'branch',
			id: c === '1' ? `${a}-${b}` : `${a}-${b}#${c}`,
			name: `${a}–${b}${c === '1' ? '' : ` (${c})`}`,
			hint: 'line'
		}));
		return [...units, ...buses, ...branches];
	});
	const found = $derived.by(() => {
		const q = search.trim().toLowerCase();
		if (!q) return [];
		return items.filter((it) => `${it.name} ${it.hint}`.toLowerCase().includes(q)).slice(0, 24);
	});
	const quantitiesOf = (owner: Owner) => QUANTITIES.filter((x) => x.owner === owner);
	const has = (key: string) => !!run?.series(key) || !run;
	function allOf(q: string, n = 10) {
		if (!ins) return;
		const keys = ins.generators.slice(0, n).map((g) => `${q}:${unitKey(g.bus, g.machine_id)}`);
		live.plotted = [...new Set([...live.plotted.filter((k) => !k.startsWith(`${q}:`)), ...keys])];
	}

	// ----- disturbances -----
	let faultBus = $state<number | null>(null);
	let faultMs = $state(100);
	let faultX = $state(0.0001);
	let branch = $state(0);
	let lineAt = $state(50);
	let lineMs = $state(100);
	let loadBus = $state<number | null>(null);
	let loadPct = $state(10);
	let gen = $state(0);
	let refs = $state<Record<string, { v: number; p: number }>>({});
	$effect(() => {
		if (!ins) return;
		if (faultBus === null) faultBus = ins.generators[0]?.bus ?? ins.buses[0] ?? null;
		if (loadBus === null) loadBus = ins.load_buses[0] ?? null;
	});
	// A bus picked on the map is the bus the actions use; plot it.
	$effect(() => {
		const b = app.selectedBus;
		if (b === null || b === undefined || !ins?.buses.includes(b)) return;
		faultBus = b;
		if (ins.load_buses.includes(b)) loadBus = b;
		const add = [`vm:${b}`, `f:${b}`].filter((k) => !live.plotted.includes(k));
		if (add.length) live.plotted = [...live.plotted, ...add];
	});
	const on = $derived(live.active && live.running);
	const busFault = () =>
		faultBus !== null &&
		live.applyNow(
			{ kind: 'bus_fault', time: 0, bus: faultBus, r: 0, x: faultX },
			[{ kind: 'clear_fault', time: 0, bus: faultBus }],
			faultMs / 1000
		);
	const br = () => ins?.branches[branch];
	const lineEv = (kind: 'branch_trip' | 'branch_close'): RmsEvent => {
		const [from, to, circuit] = br()!;
		return { kind, time: 0, from, to, circuit };
	};
	function lineFault() {
		const [from, to, circuit] = br()!;
		live.applyNow(
			{ kind: 'line_fault', time: 0, from, to, circuit, location: lineAt / 100, r: 0, x: 1e-4, permanent: true },
			[
				{ kind: 'breaker_open', time: 0, from, to, circuit, at: from },
				{ kind: 'breaker_open', time: 0, from, to, circuit, at: to }
			],
			lineMs / 1000
		);
	}
	const unit = $derived(ins?.generators[gen] ?? null);
	const ref = $derived(
		unit ? (refs[unitKey(unit.bus, unit.machine_id)] ?? { v: unit.v_ref, p: unit.p_ref }) : null
	);
	function setRef(signal: 'v_ref' | 'p_ref', value: number) {
		if (!unit) return;
		const k = unitKey(unit.bus, unit.machine_id);
		const cur = refs[k] ?? { v: unit.v_ref, p: unit.p_ref };
		refs = { ...refs, [k]: signal === 'v_ref' ? { ...cur, v: value } : { ...cur, p: value } };
		live.applyNow({
			kind: 'set_reference',
			time: 0,
			bus: unit.bus,
			machine_id: unit.machine_id.trim(),
			signal,
			value
		});
	}
	async function start() {
		const a = dyn.active;
		const l = app.activeLocal;
		if (!a?.inspection || !l?.studyInputJson) return;
		await live.start(l.id, {
			moduleJson: l.studyInputJson,
			dynText: dyn.dynText(a),
			inspection: a.inspection,
			loadModel: dyn.options.load_model ?? 'power_flow'
		});
	}
</script>

<section
	class={['wb', 'dyn-form', { collapsed }]}
	aria-label="Live workbench"
	data-testid="dyn-workbench"
	bind:clientHeight={dockHeight}
	bind:this={sectionEl}
	style:height={!collapsed && dockPx !== null ? `${dockPx}px` : null}
>
	{#if !collapsed}
		<div
			class="grip"
			role="slider"
			aria-orientation="vertical"
			aria-label="Height of the live workbench (arrow keys)"
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
		<strong>Live</strong>
		{#if live.running}
			<button onclick={() => live.pause()} data-testid="wb-pause">❚❚ Pause</button>
		{:else}
			<button class="primary" disabled={live.starting} onclick={start} data-testid="wb-start"
				>▶ {live.active ? 'Resume' : 'Start'}</button
			>
		{/if}
		<button class="ghost" disabled={!live.active} onclick={() => live.stop()}>Stop</button>
		<label
			>speed <select bind:value={live.speed}
				>{#each SPEEDS as s (s)}<option value={s}
						>{Number.isFinite(s) ? `${s}×` : 'max (unpaced)'}</option
					>{/each}</select
			></label
		>
		<label
			>window <select bind:value={live.windowS}
				>{#each WINDOWS as w (w)}<option value={w}>{w} s</option>{/each}</select
			></label
		>
		<label
			>step <select
				value={live.dt}
				onchange={(e) => void live.setStep(Number(e.currentTarget.value))}
				>{#each STEPS as s (s)}<option value={s}>{s * 1000} ms</option>{/each}</select
			></label
		>
		<output class="mono">t = {live.t.toFixed(2)} s</output>
		{#if perf}
			<div class="perfwrap" data-testid="wb-perf">
				<button type="button" class="perf" aria-describedby="wb-perf-more" aria-label={perfTitle}
					><span class="rt"><b class="mono">{fmtX(perf.engineX)}</b> real time</span>
					<span class="dim" aria-hidden="true">ⓘ</span></button
				>
				<div class="more" id="wb-perf-more" role="tooltip">
					<p>
						The engine computes <b>{fmtX(perf.engineX)}</b> faster than real time (simulated seconds
						per second of wasm compute, over the last second).
					</p>
					<svg class="spark" viewBox="0 0 220 36" aria-hidden="true"
						><polyline points={spark} fill="none" stroke="var(--accent)" stroke-width="1.5" /></svg
					>
					<p class="dim small">the last minute, one value a second</p>
					<dl>
						<dt>With the transfer to the page</dt>
						<dd class="mono">{fmtX(perf.roundX)}</dd>
						{#if Number.isFinite(live.speed)}
							<dt>Budget used at {live.speed}×</dt>
							<dd class="budget">
								<span class="bar"
									><i
										style:width={`${Math.min(100, perf.load * 100)}%`}
										class:over={perf.load > 0.9}
									></i></span
								>{Math.round(perf.load * 100)} %, headroom {Math.max(0, 100 - Math.round(perf.load * 100))} %
							</dd>
							<dt>Lag behind the clock</dt>
							<dd class:warn={perf.lagMs > 100}>{perf.lagMs.toFixed(0)} ms</dd>
						{:else}
							<dt>Pace (unpaced)</dt>
							<dd class="mono">{live.achieved?.toFixed(1)}×</dd>
						{/if}
						<dt>Per slice</dt>
						<dd class="mono">
							{perf.sliceMs.toFixed(1)} ms for {(perf.sliceSim * 1000).toFixed(0)} ms simulated
						</dd>
						<dt>Integration step</dt>
						<dd class="mono">{(live.dt * 1000).toFixed(1)} ms</dd>
					</dl>
				</div>
			</div>
		{:else if live.running}
			<span class="dim">measuring…</span>
		{/if}
		<span class="pill-experimental">Experimental</span>
		<span class="grow"></span>
		<button class="ghost" onclick={() => (collapsed = !collapsed)}
			>{collapsed ? 'Show plots' : 'Hide plots'}</button
		>
		<button class="ghost" onclick={() => live.close()} aria-label="Close the live workbench">✕</button>
	</header>
	{#if live.error}<p class="err" role="alert">{live.error}</p>{/if}
	{#if !collapsed}
		<div class="body">
			<div class="plots">
				<div class="picker">
					<input
						type="search"
						placeholder="Add a channel: bus, unit (G…) or line"
						bind:value={search}
						aria-label="Search channels"
					/>
					<button class="chip" onclick={() => allOf('omega')}>units ω</button>
					<button class="chip" onclick={() => allOf('pe')}>units P</button>
					<button class="chip" onclick={() => allOf('efd')}>units Efd</button>
					<button class="chip" onclick={() => allOf('delta')}>units δ</button>
					<button class="chip" onclick={() => (live.plotted = ['coi:f'])}>clear</button>
				</div>
				{#if found.length}
					<ul class="found">
						{#each found as it (`${it.owner}|${it.id}`)}
							<li>
								<span class="name">{it.name}</span>
								<span class="dim">{it.hint}</span>
								{#each quantitiesOf(it.owner) as x (x.q)}
									{@const key = `${x.q}:${it.id}`}
									<button
										class="chip"
										class:on={live.plotted.includes(key)}
										disabled={!has(key)}
										title={x.title}
										onclick={() => live.toggle(key)}>{x.short}</button
									>
								{/each}
							</li>
						{/each}
					</ul>
				{/if}
				{#if live.plotted.length}
					<ul class="chosen" aria-label="Plotted channels">
						{#each live.plotted as k (k)}
							<li>
								<button class="chip on" onclick={() => live.toggle(k)} title="remove"
									>{channelLabel(k)} ✕</button
								>
							</li>
						{/each}
					</ul>
				{/if}
				{#if !run}
					<p class="dim">Start the live view: the plots fill as it runs.</p>
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
			{#if ins}
				<aside aria-label="Disturbances">
					<h5>Bus fault <span class="dim">(click a bus on the map)</span></h5>
					<div class="row">
						<select bind:value={faultBus} aria-label="fault bus"
							>{#each ins.buses as b (b)}<option value={b}>bus {b}</option>{/each}</select
						>
						<label><input type="number" min="10" step="10" bind:value={faultMs} /> ms</label>
						<label>X <input type="number" min="0" step="0.0001" bind:value={faultX} /></label>
						<button class="act" disabled={!on} onclick={busFault}>⚡ Fault</button>
					</div>
					<h5>Line</h5>
					<div class="row">
						<select bind:value={branch} aria-label="line"
							>{#each ins.branches as [a, b, c], i (i)}<option value={i}
									>{a}–{b}{c === '1' ? '' : ` (${c})`}</option
								>{/each}</select
						>
						<button class="act" disabled={!on} onclick={() => live.applyNow(lineEv('branch_trip'))}
							>Trip</button
						>
						<button class="act" disabled={!on} onclick={() => live.applyNow(lineEv('branch_close'))}
							>Close</button
						>
					</div>
					<div class="row">
						<label>fault at <input type="number" min="1" max="99" step="5" bind:value={lineAt} /> %</label>
						<label>open after <input type="number" min="10" step="10" bind:value={lineMs} /> ms</label>
						<button class="act" disabled={!on} onclick={lineFault}>⚡ Fault on line</button>
					</div>
					<h5>Load step</h5>
					<div class="row">
						<select bind:value={loadBus} aria-label="load bus"
							>{#each ins.load_buses as b (b)}<option value={b}>bus {b}</option>{/each}</select
						>
						<label><input type="number" min="-100" max="100" step="5" bind:value={loadPct} /> %</label>
						<button
							class="act"
							disabled={!on || loadBus === null}
							onclick={() =>
								loadBus !== null &&
								live.applyNow({ kind: 'load_change', time: 0, bus: loadBus, scale: 1 + loadPct / 100 })}
							>Step</button
						>
					</div>
					<h5>Unit</h5>
					<div class="row">
						<select bind:value={gen} aria-label="unit"
							>{#each ins.generators as g, i (i)}<option value={i}
									>G{g.bus}{g.machine_id.trim() === '1' ? '' : ` '${g.machine_id.trim()}'`}</option
								>{/each}</select
						>
						<button
							class="act"
							disabled={!on || !unit}
							onclick={() =>
								unit &&
								live.applyNow({
									kind: 'generator_trip',
									time: 0,
									bus: unit.bus,
									machine_id: unit.machine_id.trim()
								})}>Trip</button
						>
					</div>
					{#if unit && ref}
						<label class="slider"
							><span>V_ref</span>
							<input
								type="range"
								min={unit.v_ref * 0.9}
								max={unit.v_ref * 1.1}
								step={unit.v_ref * 0.002}
								value={ref.v}
								disabled={!on}
								onchange={(e) => setRef('v_ref', Number(e.currentTarget.value))}
							/><span class="mono">{ref.v.toFixed(3)}</span></label
						>
						<label class="slider"
							><span>P_ref</span>
							<input
								type="range"
								min={unit.p_ref - 0.3}
								max={unit.p_ref + 0.3}
								step="0.005"
								value={ref.p}
								disabled={!on}
								onchange={(e) => setRef('p_ref', Number(e.currentTarget.value))}
							/><span class="mono">{ref.p.toFixed(3)}</span></label
						>
					{/if}
					<h5>Event log</h5>
					<ol class="log">
						{#each [...live.log].reverse() as [t, text], i (i)}
							<li><span class="mono">{t.toFixed(2)} s</span> {text}</li>
						{:else}
							<li class="dim">No events yet.</li>
						{/each}
					</ol>
				</aside>
			{/if}
		</div>
	{/if}
</section>

<style>
	.wb {
		position: fixed;
		left: var(--tellegen-dock-left, 12px);
		right: var(--tellegen-dock-right, 12px);
		bottom: 12px;
		height: 52vh;
		z-index: 30;
		display: flex;
		flex-direction: column;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: 8px;
		box-shadow: 0 8px 28px rgb(0 0 0 / 0.2);
		color: var(--text-primary);
		font-size: var(--fs-control);
	}
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
	.perfwrap {
		position: relative;
	}
	.perf {
		display: flex;
		font: inherit;
		color: inherit;
		align-items: center;
		gap: 6px;
		padding: 2px 8px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--surface-chip);
		cursor: help;
	}
	.rt b {
		font-size: 1.2em;
		color: var(--text-accent);
	}
	.more {
		display: none;
		position: absolute;
		bottom: calc(100% + 6px);
		left: 0;
		z-index: 50;
		width: 300px;
		padding: 8px 10px;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: 8px;
		box-shadow: 0 8px 28px rgb(0 0 0 / 0.25);
		cursor: default;
	}
	.perfwrap:hover .more,
	.perfwrap:focus-within .more {
		display: grid;
		gap: 4px;
	}
	.more p {
		margin: 0;
	}
	.more dl {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 2px 8px;
		margin: 0;
	}
	.more dt {
		color: var(--text-tertiary);
	}
	.more dd {
		margin: 0;
	}
	.small {
		font-size: var(--fs-meta);
	}
	.spark {
		width: 220px;
		height: 36px;
	}
	.budget {
		display: inline-flex;
		align-items: center;
		gap: 4px;
	}
	.bar {
		display: inline-block;
		width: 70px;
		height: 6px;
		border-radius: 3px;
		background: var(--border-default);
		overflow: hidden;
	}
	.bar i {
		display: block;
		height: 100%;
		background: var(--accent);
	}
	.bar i.over {
		background: var(--text-danger);
	}
	.warn {
		color: var(--text-danger);
	}
	.body {
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr) clamp(280px, 20vw, 380px);
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
	.picker {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
		align-items: center;
	}
	.picker input {
		flex: 1;
		min-width: 200px;
	}
	header label {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--text-secondary);
	}
	header output {
		padding: 0 4px;
		color: var(--text-secondary);
	}
	.found,
	.chosen,
	.log {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.found {
		max-height: 140px;
		overflow: auto;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
	}
	.found li:hover {
		background: var(--accent-soft);
	}
	.found li {
		display: flex;
		gap: 4px;
		align-items: center;
		padding: 1px 6px;
		flex-wrap: wrap;
	}
	.found .name {
		min-width: 90px;
		font-family: var(--font-mono);
	}
	.chosen {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.chip {
		padding: 1px 6px;
		border: 1px solid var(--border-default);
		border-radius: 9px;
		background: var(--surface-chip);
		color: var(--ink);
		font: inherit;
		font-size: var(--fs-meta);
		cursor: pointer;
	}
	.chip.on {
		border-color: var(--accent);
		color: var(--text-accent);
	}
	.chip:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.charts {
		flex: 1 1 0;
		min-height: 0;
		overflow: auto;
		display: grid;
		align-content: start;
		gap: 6px;
	}
	aside {
		border-left: 1px solid var(--border-default);
		padding: 6px 10px;
		overflow: auto;
		display: grid;
		align-content: start;
		gap: 4px;
	}
	h5 {
		margin: 10px 0 2px;
		padding-top: 8px;
		border-top: 1px solid var(--border-default);
		color: var(--text-tertiary);
		font-size: var(--fs-meta);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	h5:first-child {
		margin-top: 0;
		padding-top: 0;
		border-top: none;
	}
	h5 .dim {
		font-weight: normal;
		letter-spacing: 0;
		text-transform: none;
	}
	.row {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
		align-items: center;
	}
	.row {
		color: var(--text-secondary);
	}
	.row label {
		display: inline-flex;
		align-items: center;
		gap: 4px;
	}
	.row select {
		max-width: 130px;
	}
	.row input[type='number'] {
		flex: none;
		width: calc(9ch + 14px);
		font-family: var(--font-mono);
		text-align: right;
	}
	.slider {
		color: var(--text-secondary);
		display: grid;
		grid-template-columns: 44px 1fr 52px;
		gap: 4px;
		align-items: center;
	}
	.log {
		max-height: 120px;
		overflow: auto;
		font-size: var(--fs-meta);
	}
	.log li {
		padding: 1px 0;
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
