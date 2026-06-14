<script lang="ts" module>
	// Counter for local case ids; module level so ids stay unique across remounts.
	let localSeq = 0;
</script>

<script lang="ts">
	import { getCases, getNetwork, getSensitivity, getSolution, openSolveStream } from '$lib/api';
	import { busRadius, lmpDomain, lmpGradient, sensGradient } from '$lib/colors';
	import { app, CaseState, type LocalCase } from '$lib/state.svelte';
	import { formatOf, ingestCase } from '$lib/wasm';
	import Sparkline from '$lib/Sparkline.svelte';
	import TellegenMap from '$lib/TellegenMap.svelte';

	let abort: AbortController | null = null;
	let closeStream: (() => void) | null = null;
	let fileInput: HTMLInputElement | undefined = $state();

	async function load() {
		try {
			const summaries = await getCases();
			app.cases = summaries.map((s) => new CaseState(s));
			app.activeCaseId = summaries[0]?.id ?? null;
			await Promise.all(
				app.cases.map(async (c) => {
					const [network, solution] = await Promise.all([getNetwork(c.id), getSolution(c.id)]);
					c.network = network;
					c.baseSolution = solution;
					c.solution = solution;
				})
			);
			app.requestFrame('all');
		} catch (e) {
			app.error = `backend unreachable: ${e instanceof Error ? e.message : e}`;
		}
	}

	load();

	function activateCase(id: string) {
		app.activeLocalId = null;
		if (app.activeCaseId !== id) {
			clearSelection();
			app.activeCaseId = id;
		}
		app.requestFrame(id);
	}

	function activateLocal(c: LocalCase) {
		clearSelection();
		app.activeLocalId = c.id;
		if (c.view) app.requestFrame(c.id);
	}

	async function selectBus(caseId: string, busId: number) {
		app.activeLocalId = null;
		if (app.activeCaseId !== caseId) app.activeCaseId = caseId;
		const c = app.byId(caseId);
		if (!c) return;
		abort?.abort();
		const ac = new AbortController();
		abort = ac;
		app.error = null;
		app.selectedBus = busId;
		app.previewDeltaMw = null;
		app.sensitivityLoading = true;
		try {
			const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
			if (!ac.signal.aborted) c.sensitivity = col;
		} catch (e) {
			if (!ac.signal.aborted && !(e instanceof DOMException)) {
				app.error = String(e);
			}
		} finally {
			if (abort === ac) app.sensitivityLoading = false;
		}
	}

	function clearSelection() {
		abort?.abort();
		app.selectedBus = null;
		app.previewDeltaMw = null;
		if (app.active) app.active.sensitivity = null;
		app.sensitivityLoading = false;
	}

	function runSolve(c: CaseState, sensBus: number | null) {
		closeStream?.();
		app.error = null;
		c.solving = true;
		c.iterations = [];
		c.solveMs = null;
		closeStream = openSolveStream(c.id, c.deltas, sensBus, {
			oniteration: (it) => {
				c.iterations = [...c.iterations, it];
			},
			onsolution: (sol) => {
				c.solution = sol;
				c.solveMs = sol.solve_ms;
			},
			onsensitivity: (col) => {
				c.sensitivity = col;
			},
			onfail: (msg) => {
				c.solving = false;
				app.error = msg;
			},
			ondone: () => {
				c.solving = false;
			}
		});
	}

	function commitDelta(value: number) {
		const c = app.active;
		const bus = app.selectedBus;
		if (!c || bus === null) return;
		c.predictedObjective = predictedDeltaObj;
		const deltas = { ...c.deltas };
		if (Math.abs(value) < 0.25) delete deltas[bus];
		else deltas[bus] = value;
		c.deltas = deltas;
		app.previewDeltaMw = null;
		runSolve(c, bus);
	}

	function resetCase(c: CaseState) {
		c.deltas = {};
		c.predictedObjective = null;
		app.previewDeltaMw = null;
		if (c.baseSolution) c.solution = c.baseSolution;
		runSolve(c, app.selectedBus);
	}

	/** Parse dropped case files in the browser via the powerio wasm module.
	 * Files run serially; nothing uploads. */
	async function ingestFiles(files: FileList | File[]) {
		for (const file of Array.from(files)) {
			const format = formatOf(file.name);
			if (!format) {
				app.error = `${file.name}: not a case file (.m, .raw, .aux)`;
				continue;
			}
			app.parsingFile = true;
			try {
				const text = await file.text();
				const { view, ...summary } = await ingestCase(text, format);
				const id = `local-${++localSeq}`;
				const label =
					summary.name && summary.name !== 'case'
						? summary.name
						: file.name.replace(/\.[^.]+$/, '');
				app.addLocal({ id, label, fileName: file.name, summary, view });
				app.error = null; // a successful parse clears a prior file's error
				if (view) app.requestFrame(id);
			} catch (e) {
				app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				app.parsingFile = false;
			}
		}
	}

	// Depth counter so dragenter/dragleave on nested elements doesn't flicker
	// the overlay.
	let dragDepth = 0;

	function dragHasFiles(e: DragEvent): boolean {
		return e.dataTransfer?.types.includes('Files') ?? false;
	}

	function onDragEnter(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth++;
		app.dragOver = true;
	}

	function onDragLeave(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		dragDepth = Math.max(0, dragDepth - 1);
		if (dragDepth === 0) app.dragOver = false;
	}

	function onDragOver(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
	}

	function onDrop(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth = 0;
		app.dragOver = false;
		if (e.dataTransfer) ingestFiles(e.dataTransfer.files);
	}

	function splitName(name: string): [string, string] {
		const m = name.match(/^(.*?)\s*\((.*)\)$/);
		return m ? [m[1], m[2]] : [name, ''];
	}

	const stats = $derived.by(() => {
		const c = app.active;
		if (!c?.network || !c.solution || !c.baseSolution) return null;
		const lmps = c.solution.lmp.map((e) => e.usd_per_mwh);
		const domain = lmpDomain(lmps);
		const lmpMin = Math.min(...lmps);
		const lmpMax = Math.max(...lmps);
		return {
			buses: c.network.buses.length,
			branches: c.network.branches.length,
			objective: c.solution.objective,
			deltaObjective: c.solution.objective - c.baseSolution.objective,
			uniformLmp: lmpMax - lmpMin < 1 ? lmps[0] : null,
			// Mark the legend ends when outliers clamp beyond the robust domain.
			lmpLo: { value: domain.lo, clamped: lmpMin < domain.lo - 0.05 },
			lmpHi: { value: domain.hi, clamped: lmpMax > domain.hi + 0.05 },
			binding: c.solution.flows.filter((f) => f.loading >= 0.999).length
		};
	});

	const selectedBusData = $derived.by(() => {
		const c = app.active;
		if (!c?.network || app.selectedBus === null) return null;
		return c.network.buses.find((b) => b.id === app.selectedBus) ?? null;
	});

	const committedDelta = $derived(
		app.active && app.selectedBus !== null ? (app.active.deltas[app.selectedBus] ?? 0) : 0
	);
	const sliderValue = $derived(app.previewDeltaMw ?? committedDelta);
	const sliderMin = $derived(selectedBusData ? -Math.ceil(selectedBusData.demand_mw) : 0);
	const sliderMax = $derived(
		selectedBusData ? Math.max(Math.ceil(selectedBusData.demand_mw), 50) : 0
	);

	const selectedLmp = $derived.by(() => {
		const c = app.active;
		if (!c?.solution || app.selectedBus === null) return null;
		return c.solution.lmp.find((e) => e.bus === app.selectedBus)?.usd_per_mwh ?? null;
	});

	const selfSens = $derived.by(() => {
		const c = app.active;
		if (!c?.sensitivity || app.selectedBus === null) return 0;
		return c.sensitivity.values.find((v) => v.bus === app.selectedBus)?.value ?? 0;
	});

	// Second order objective preview vs base: the exact part up to the
	// committed point, plus lmp*step + S_bb*step^2/2 along the gradient.
	const predictedDeltaObj = $derived.by(() => {
		const c = app.active;
		if (!c?.solution || !c.baseSolution || selectedLmp === null) return null;
		const step = sliderValue - committedDelta;
		const committedPart = c.solution.objective - c.baseSolution.objective;
		return committedPart + selectedLmp * step + 0.5 * selfSens * step * step;
	});

	const gradientScore = $derived.by(() => {
		const c = app.active;
		if (!c?.solution || !c.baseSolution || c.predictedObjective === null || c.solving)
			return null;
		const exact = c.solution.objective - c.baseSolution.objective;
		return { pred: c.predictedObjective, exact };
	});

	const topMovers = $derived.by(() => {
		const c = app.active;
		if (!c?.sensitivity) return [];
		return [...c.sensitivity.values]
			.filter((v) => v.bus !== app.selectedBus)
			.sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
			.slice(0, 5);
	});

	const sensMaxAbs = $derived.by(() => {
		const c = app.active;
		return c?.sensitivity ? Math.max(...c.sensitivity.values.map((v) => Math.abs(v.value))) : 0;
	});

	const previewing = $derived(
		app.previewDeltaMw !== null && Math.abs(sliderValue - committedDelta) >= 0.25
	);

	const fmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
	const signed = (v: number) => `${v < 0 ? '−' : '+'}${fmt.format(Math.abs(v))}`;
	const SIZE_SAMPLES = [10, 100, 500];
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') clearSelection();
	}}
	ondragenter={onDragEnter}
	ondragleave={onDragLeave}
	ondragover={onDragOver}
	ondrop={onDrop}
/>

<main>
	<TellegenMap onbusclick={selectBus} />

	<header>
		<div class="brand">
			<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
				<path d="M4 18 L12 6 L20 18" stroke="#b25e00" stroke-width="1.6" fill="none" />
				<circle cx="4" cy="18" r="2.4" fill="#b25e00" />
				<circle cx="12" cy="6" r="2.4" fill="#20242b" />
				<circle cx="20" cy="18" r="2.4" fill="#b25e00" />
			</svg>
			<h1>tellegen</h1>
		</div>
		<nav class="cases" aria-label="networks">
			{#each app.cases as c (c.id)}
				{@const [cname, cregion] = splitName(c.name)}
				<button class:active={app.activeCaseId === c.id} onclick={() => activateCase(c.id)}>
					<span class="cname">{cname}{#if c.perturbed}<i class="mark" title="demand perturbed"
							></i>{/if}</span>
					<span class="cregion mono">{cregion}</span>
				</button>
			{/each}
			{#each app.localCases as c (c.id)}
				<button
					class="local"
					class:active={app.activeLocalId === c.id}
					onclick={() => activateLocal(c)}
				>
					<span class="cname"
						>{c.label}<span
							class="x mono"
							role="button"
							tabindex="0"
							aria-label="remove {c.label}"
							onclick={(e) => {
								e.stopPropagation();
								app.removeLocal(c.id);
							}}
							onkeydown={(e) => {
								if (e.key === 'Enter' || e.key === ' ') {
									e.stopPropagation();
									app.removeLocal(c.id);
								}
							}}>&#10005;</span
						></span
					>
					<span class="cregion mono">local</span>
				</button>
			{/each}
			<button
				class="ghost"
				title="parsed in your browser; the file never uploads"
				onclick={() => fileInput?.click()}
			>
				<span class="cname"><span class="arrow">&#8675;</span>drop a case file</span>
				<span class="cregion mono">.m &middot; .raw &middot; .aux &mdash; or click</span>
			</button>
		</nav>
		<span class="kicker mono">differentiable power systems</span>
	</header>

	<aside class="panel">
		{#if app.error}
			<p class="error mono">{app.error}</p>
		{/if}
		{#if app.parsingFile}
			<p class="dim mono blink">parsing&hellip;</p>
		{/if}
		{#if app.activeLocal}
			{@const lc = app.activeLocal}
			<h2>{lc.label} <span class="region mono">via {lc.fileName}</span></h2>
			<dl class="mono">
				<div><dt>buses</dt><dd>{lc.summary.n_bus}</dd></div>
				<div><dt>branches</dt><dd>{lc.summary.n_branch}</dd></div>
				<div><dt>generators</dt><dd>{lc.summary.n_gen}</dd></div>
				<div><dt>load</dt><dd>{fmt.format(lc.summary.load_mw)} MW</dd></div>
				<div><dt>gen capacity</dt><dd>{fmt.format(lc.summary.gen_mw)} MW</dd></div>
				<div><dt>base MVA</dt><dd>{fmt.format(lc.summary.base_mva)}</dd></div>
			</dl>
			{#if lc.summary.warnings.length > 0}
				<ul class="warnings mono">
					{#each lc.summary.warnings.slice(0, 4) as w, i (i)}
						<li>{w}</li>
					{/each}
					{#if lc.summary.warnings.length > 4}
						<li>+{lc.summary.warnings.length - 4} more</li>
					{/if}
				</ul>
			{/if}
			{#if !lc.view}
				<p class="footnote mono">
					no substation coordinates in this file &mdash; parsed, not placed
				</p>
			{/if}
			<p class="footnote mono">parsed in your browser by powerio (wasm); never uploaded</p>
			<button class="reset mono" onclick={() => app.removeLocal(lc.id)}>remove</button>
		{:else if !stats}
			{#if !app.error}
				<p class="dim mono blink">loading cases&hellip;</p>
			{/if}
		{:else}
			{@const [cname, cregion] = splitName(app.active?.name ?? '')}
			<h2>{cname} <span class="region mono">{cregion}</span></h2>
			<dl class="mono">
				<div><dt>buses</dt><dd>{stats.buses}</dd></div>
				<div><dt>branches</dt><dd>{stats.branches}</dd></div>
				<div><dt>binding lines</dt><dd>{stats.binding}</dd></div>
				<div><dt>objective</dt><dd>{fmt.format(stats.objective)} $/h</dd></div>
				{#if app.active?.perturbed}
					<div class="delta"><dt>vs base</dt><dd>{signed(stats.deltaObjective)} $/h</dd></div>
				{/if}
			</dl>
			{#if app.active?.network?.synthetic_coords}
				<p class="footnote mono">coordinates: synthetic</p>
			{/if}

			<hr />

			{#if app.selectedBus !== null && app.active?.sensitivity}
				{@const c = app.active}
				<div class="mode">
					<span class="chip">{previewing ? 'LMP preview' : '∂LMP/∂d'}</span>
					<span class="mono dim">bus {app.selectedBus}</span>
					<button class="mono" onclick={clearSelection}>esc&nbsp;clear</button>
				</div>
				{#if previewing}
					<p class="dim small">
						Prices shifted along the gradient, no solve yet. Release the slider to stream the
						exact solution.
					</p>
				{:else}
					<p class="dim small">
						Price response across the network per MW of demand added at bus
						{app.selectedBus}. One exact KKT column, no re-solve.
					</p>
					<div class="legend" style:background={sensGradient}></div>
					<div class="legend-labels mono">
						<span>&minus;{sensMaxAbs.toExponential(1)}</span>
						<span>0</span>
						<span>+{sensMaxAbs.toExponential(1)}</span>
					</div>
				{/if}

				<div class="slider-block">
					<div class="slider-head mono">
						<span>&Delta; demand</span>
						<span class="val">{signed(sliderValue)} MW</span>
					</div>
					<input
						type="range"
						min={sliderMin}
						max={sliderMax}
						step="0.5"
						value={sliderValue}
						aria-label="demand delta at selected bus"
						oninput={(e) => {
							app.previewDeltaMw = Number(e.currentTarget.value);
						}}
						onchange={(e) => commitDelta(Number(e.currentTarget.value))}
					/>
					{#if predictedDeltaObj !== null && previewing}
						<p class="pred mono dim">predicted &Delta;cost {signed(predictedDeltaObj)} $/h</p>
					{/if}
					{#if gradientScore && c.perturbed}
						<p class="score mono">
							gradient {signed(gradientScore.pred)} &middot; exact {signed(gradientScore.exact)} $/h
						</p>
					{/if}
					{#if c.perturbed}
						<button class="reset mono" onclick={() => resetCase(c)}>reset demand</button>
					{/if}
				</div>

				{#if !previewing}
					<table class="mono">
						<tbody>
							{#each topMovers as mover (mover.bus)}
								<tr>
									<td>bus {mover.bus}</td>
									<td class:pos={mover.value > 0} class:neg={mover.value < 0}>
										{mover.value >= 0 ? '+' : ''}{mover.value.toExponential(2)}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			{:else}
				<div class="mode">
					<span class="chip">LMP</span>
					<span class="mono dim">$/MWh</span>
					{#if app.sensitivityLoading}
						<span class="mono dim blink">&part; loading&hellip;</span>
					{/if}
				</div>
				<p class="dim small">
					Locational marginal prices from the DC optimal power flow. Click any bus to see its
					demand sensitivity column and perturb its load.
				</p>
				<div class="legend" style:background={lmpGradient}></div>
				<div class="legend-labels mono">
					{#if stats.uniformLmp !== null}
						<span>uniform {fmt.format(stats.uniformLmp)} $/MWh, no congestion</span>
					{:else}
						<span>{stats.lmpLo.clamped ? '≤' : ''}{fmt.format(stats.lmpLo.value)}</span>
						<span>{stats.lmpHi.clamped ? '≥' : ''}{fmt.format(stats.lmpHi.value)}</span>
					{/if}
				</div>
				<p class="dim small">
					Or bring your own grid: drop a case file (.m, .raw, .aux) anywhere on the map. powerio
					parses it in your browser; nothing uploads.
				</p>
			{/if}

			<hr />

			<div class="sizes">
				{#each SIZE_SAMPLES as mw (mw)}
					<span class="size mono">
						<i style:width="{2 * busRadius(mw)}px" style:height="{2 * busRadius(mw)}px"></i>
						{mw}
					</span>
				{/each}
				<span class="mono dim caption">MW, max(load,&#8201;gen)</span>
			</div>
		{/if}
	</aside>

	{#if app.active && (app.active.solving || app.active.iterations.length > 1)}
		<div class="solvecard">
			<div class="solvecard-head mono">
				<span>exact solve</span>
				{#if app.active.solving}
					<span class="dim blink">streaming&hellip;</span>
				{:else}
					<span class="dim">ipopt</span>
				{/if}
			</div>
			<Sparkline iterations={app.active.iterations} />
			<div class="solve-meta mono dim">
				<span>{app.active.iterations.length} iterations</span>
				{#if app.active.solveMs !== null}<span>{app.active.solveMs} ms</span>{/if}
			</div>
		</div>
	{/if}

	{#if app.dragOver}
		<div class="dropzone" aria-hidden="true">
			<div class="dropframe">
				<p class="mono">drop to parse &mdash; .m &middot; .raw &middot; .aux</p>
				<p class="mono hint">parsed in your browser; the file never uploads</p>
			</div>
		</div>
	{/if}

	<footer class="mono">
		<a href="https://electricgrids.engr.tamu.edu/" target="_blank" rel="noreferrer"
			>ACTIVSg synthetic grids</a
		>
		<i class="sep"></i>
		<a href="https://github.com/grid-opt-alg-lab/PowerDiff.jl" target="_blank" rel="noreferrer"
			>powerdiff sensitivities</a
		>
		<i class="sep"></i>
		<a href="https://github.com/eigenergy/powerio" target="_blank" rel="noreferrer"
			>powerio parser</a
		>
		<i class="sep"></i>
		<a href="https://github.com/eigenergy/tellegen" target="_blank" rel="noreferrer"
			>tellegen framework</a
		>
		<i class="sep"></i>
		<span class="drophint"><span class="arrow">&#8675;</span> drop a case file anywhere</span>
	</footer>

	<input
		type="file"
		accept=".m,.raw,.aux"
		multiple
		hidden
		bind:this={fileInput}
		onchange={(e) => {
			const input = e.currentTarget;
			if (input.files) ingestFiles(Array.from(input.files));
			input.value = '';
		}}
	/>
</main>

<style>
	main {
		position: fixed;
		inset: 0;
		overflow: hidden;
	}

	header {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 20px;
		background: linear-gradient(rgba(236, 233, 226, 0.95), rgba(236, 233, 226, 0));
		animation: drop 0.5s ease-out both;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	h1 {
		margin: 0;
		font-size: 22px;
		font-weight: 600;
		letter-spacing: -0.02em;
	}

	.cases {
		display: flex;
		gap: 6px;
	}

	.cases button {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		padding: 5px 12px 4px;
		background: rgba(252, 251, 247, 0.65);
		border: 1px solid var(--line);
		border-radius: 3px;
		cursor: pointer;
		font-family: var(--font-display);
		color: var(--ink);
		transition: border-color 0.15s ease;
	}

	.cases button:hover {
		border-color: var(--accent);
	}

	.cases button.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	.cname {
		font-size: 12.5px;
		font-weight: 600;
		line-height: 1.2;
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.cname .mark {
		width: 5px;
		height: 5px;
		background: var(--accent-bright);
		transform: rotate(45deg);
	}

	.cregion {
		font-size: 9.5px;
		color: var(--ink-dim);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	/* Local case chips: dashed border + graphite text, topology only. */
	.cases button.local {
		border-style: dashed;
		color: var(--ink-dim);
	}

	.cases button.local.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	/* Ghost chip: standing invitation to drop or pick a case file. */
	.cases button.ghost {
		background: transparent;
		border: 1px dashed var(--ink-faint);
		color: var(--ink-dim);
	}

	.cases button.ghost:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}

	.cases button.ghost:hover,
	.cases button.ghost:hover .cregion {
		color: var(--accent);
	}

	.arrow {
		display: inline-block;
		animation: bob 1.8s ease-in-out infinite alternate;
	}

	.x {
		font-size: 9px;
		color: var(--ink-faint);
		padding: 0 1px;
		cursor: pointer;
	}

	.x:hover {
		color: var(--red);
	}

	.kicker {
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.18em;
		color: var(--ink-dim);
	}

	.panel {
		position: absolute;
		top: 64px;
		left: 20px;
		z-index: 10;
		width: 312px;
		max-height: calc(100% - 110px);
		overflow-y: auto;
		padding: 18px 20px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.5s 0.12s ease-out both;
	}

	h2 {
		margin: 0 0 12px;
		font-size: 16px;
		font-weight: 600;
	}

	.region {
		font-size: 10px;
		font-weight: 400;
		color: var(--ink-dim);
		text-transform: uppercase;
		letter-spacing: 0.08em;
		margin-left: 4px;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
	}

	dl .delta dd {
		color: var(--accent);
	}

	dt {
		color: var(--ink-dim);
	}

	dd {
		margin: 0;
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 10px;
		color: var(--ink-faint);
		letter-spacing: 0.04em;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--accent);
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 14px 0;
	}

	.mode {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 12px;
	}

	.chip {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 2px 8px;
		border: 1px solid var(--accent);
		color: var(--accent);
		background: var(--accent-soft);
		border-radius: 2px;
		white-space: nowrap;
	}

	.mode button {
		margin-left: auto;
		font-size: 10.5px;
		padding: 2px 7px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.mode button:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.small {
		font-size: 12px;
		line-height: 1.55;
	}

	.dim {
		color: var(--ink-dim);
	}

	.error {
		color: var(--red);
		font-size: 12px;
	}

	.legend {
		height: 6px;
		border-radius: 3px;
		margin-top: 6px;
	}

	.legend-labels {
		display: flex;
		justify-content: space-between;
		font-size: 10.5px;
		color: var(--ink-faint);
		margin-top: 4px;
	}

	.slider-block {
		margin-top: 14px;
	}

	.slider-head {
		display: flex;
		justify-content: space-between;
		font-size: 11.5px;
		color: var(--ink-dim);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: 4px;
		background: var(--line);
		border-radius: 2px;
		outline-offset: 4px;
		margin: 6px 0;
	}

	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	input[type='range']::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	.pred {
		margin: 2px 0 0;
		font-size: 11px;
	}

	.score {
		margin: 8px 0 0;
		font-size: 11px;
		color: var(--ink);
	}

	.solvecard {
		position: absolute;
		top: 64px;
		right: 20px;
		z-index: 10;
		width: 240px;
		padding: 12px 14px 10px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.3s ease-out both;
	}

	.solvecard-head {
		display: flex;
		justify-content: space-between;
		font-size: 11px;
		margin-bottom: 6px;
	}

	.solve-meta {
		display: flex;
		gap: 12px;
		font-size: 10px;
		margin-top: 4px;
	}

	.reset {
		margin-top: 10px;
		font-size: 10.5px;
		padding: 3px 10px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.reset:hover {
		border-color: var(--red);
		color: var(--red);
	}

	table {
		width: 100%;
		margin-top: 12px;
		border-collapse: collapse;
		font-size: 12px;
	}

	td {
		padding: 3px 0;
		border-top: 1px solid var(--line);
	}

	td:last-child {
		text-align: right;
	}

	.pos {
		color: var(--pos);
	}

	.neg {
		color: var(--neg);
	}

	.sizes {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 10px;
		color: var(--ink-dim);
	}

	.size {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.size i {
		display: inline-block;
		border-radius: 50%;
		background: rgba(212, 116, 34, 0.55);
		border: 1px solid rgba(46, 42, 34, 0.45);
	}

	.caption {
		margin-left: auto;
		font-size: 9.5px;
	}

	footer {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		padding: 8px 20px;
		font-size: 10.5px;
		color: var(--ink-dim);
		background: linear-gradient(rgba(236, 233, 226, 0), rgba(236, 233, 226, 0.9));
		animation: rise 0.5s 0.24s ease-out both;
		pointer-events: none;
	}

	footer a {
		pointer-events: auto;
		color: var(--ink-dim);
	}

	footer a:hover {
		color: var(--accent);
	}

	.sep {
		width: 4px;
		height: 4px;
		margin: 0 10px;
		background: var(--accent-bright);
		opacity: 0.55;
		transform: rotate(45deg);
	}

	.drophint {
		color: var(--ink-faint);
	}

	.drophint .arrow {
		color: var(--accent);
	}

	.dropzone {
		position: fixed;
		inset: 0;
		z-index: 20;
		pointer-events: none;
		background: rgba(236, 233, 226, 0.75);
	}

	.dropframe {
		position: absolute;
		inset: 14px;
		border: 1.5px dashed var(--accent);
		border-radius: 3px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}

	.dropframe p {
		margin: 0;
		font-size: 13px;
	}

	.dropframe .hint {
		font-size: 11px;
		color: var(--ink-dim);
	}

	.blink {
		animation: blink 1.2s steps(2) infinite;
	}

	@keyframes drop {
		from {
			opacity: 0;
			transform: translateY(-8px);
		}
	}

	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}

	@keyframes blink {
		50% {
			opacity: 0.35;
		}
	}

	@keyframes bob {
		to {
			transform: translateY(2px);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		header,
		.panel,
		footer,
		.arrow {
			animation: none;
		}
	}
</style>
