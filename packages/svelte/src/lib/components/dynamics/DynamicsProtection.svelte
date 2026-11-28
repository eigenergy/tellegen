<script lang="ts">
	/** The Protection tab (experimental): the relays of the active case.
	 * Switched on, they act in every RMS run and in the live session. Settings
	 * by rule (IEC 60255 functions), the setting audit, the standard tests
	 * (run on a worker of their own, replayed on the map), the relays at the
	 * operating point with static fault studies, and the relay record of the
	 * run, study or live session shown.
	 * @block Protection tab */
	import { describeExpectation } from '@tellegen/engine';
	import type { Finding, ProtectionTest, RelayOperation, StaticFault } from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import { relayCodes } from '../../dynamics/protection.svelte.js';

	const app = getAppState();
	const dyn = getController().dynamics;
	const prot = dyn.protection;

	const local = $derived(app.activeLocal);
	const p = $derived(prot.active);
	const ins = $derived(dyn.active?.inspection ?? null);
	const ready = $derived(dyn.active?.status === 'ready' && !!local?.studyInputJson);

	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	let characteristic = $state<'mho' | 'quadrilateral'>('mho');
	type Sub = 'relays' | 'audit' | 'tests' | 'study' | 'record';
	let sub = $state<Sub>('relays');

	async function act(label: string, f: () => Promise<unknown>, then?: Sub) {
		busy = label;
		error = null;
		try {
			await f();
			if (then) sub = then;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = null;
		}
	}

	const setByRule = () => act('Setting relays…', () => prot.setByRule({ characteristic }), 'audit');
	const toggleLayer = (on: boolean) =>
		act(on && !p ? 'Setting relays by rule…' : 'Switching…', () => prot.setLayer(on));

	function download() {
		if (!p || !local) return;
		const blob = new Blob([JSON.stringify(p.scheme, null, 2)], { type: 'application/json' });
		const a = document.createElement('a');
		a.href = URL.createObjectURL(blob);
		a.download = `${local.label.replace(/\W+/g, '_')}.protection.json`;
		a.click();
		URL.revokeObjectURL(a.href);
	}
	async function upload(e: Event) {
		const f = (e.currentTarget as HTMLInputElement).files?.[0];
		if (f) await act('Loading scheme…', async () => prot.load(await f.text()));
	}

	// ----- relays grouped by kind -----
	const KINDS: { kind: string; label: string }[] = [
		{ kind: 'line', label: 'Lines' },
		{ kind: 'transformer', label: 'Transformers' },
		{ kind: 'busbar', label: 'Busbars' },
		{ kind: 'generator', label: 'Generators' },
		{ kind: 'inverter', label: 'Inverters' }
	];
	const grouped = $derived(
		KINDS.map((k) => ({
			...k,
			relays: (p?.scheme.relays ?? []).filter((r) => r.location.kind === k.kind)
		})).filter((g) => g.relays.length)
	);
	const selected = $derived(p?.scheme.relays.find((r) => r.id === prot.selectedRelay) ?? null);
	const zonesOf = $derived(
		selected?.functions.flatMap((f) => (f.type === 'distance' ? f.zones : [])) ?? []
	);
	const pick = (id: string) => (prot.selectedRelay = prot.selectedRelay === id ? null : id);
	function toggleRelay(id: string) {
		const r = p?.scheme.relays.find((x) => x.id === id);
		if (r) void act('Auditing…', () => prot.updateRelay({ ...r, enabled: !(r.enabled ?? true) }));
	}

	// ----- audit -----
	const SEV_ORDER = { error: 0, warning: 1, info: 2, ok: 3 } as const;
	const auditCounts = $derived.by(() => {
		const c = { error: 0, warning: 0, info: 0, ok: 0 };
		for (const f of p?.audit ?? []) c[f.severity] += 1;
		return c;
	});
	let showOk = $state(false);
	const findings = $derived(
		(p?.audit ?? [])
			.filter((f: Finding) => showOk || f.severity !== 'ok')
			.slice()
			.sort((a, b) => SEV_ORDER[a.severity] - SEV_ORDER[b.severity])
	);

	// ----- tests -----
	let category = $state('all');
	const categories = $derived([...new Set((p?.tests ?? []).map((t) => t.category))]);
	const outcome = (name: string) => p?.suite?.tests.find((o) => o.name === name);
	const tests = $derived(
		(p?.tests ?? []).filter((t) => category === 'all' || t.category === category)
	);
	let expanded = $state<string | null>(null);
	const runTests = (some?: ProtectionTest[]) =>
		act(some ? 'Running tests…' : 'Running all tests…', () => prot.runSuite(some), 'tests');
	const play = (t: ProtectionTest) =>
		act(`Replaying ${t.name}…`, () => prot.replay(t), 'record');

	// ----- load flow: operating point and static fault studies -----
	$effect(() => {
		if (sub !== 'study' || !p?.enabled) return;
		void p.scheme;
		prot.evaluateOperating().catch((e) => (error = e instanceof Error ? e.message : String(e)));
	});
	const operating = $derived(prot.operating?.scheme === p?.scheme ? prot.operating : null);
	let faultKind = $state<'line' | 'bus'>('line');
	let faultBranch = $state(0);
	let faultBus = $state<number | null>(null);
	let faultAt = $state(50);
	const branchList = $derived(ins?.branches ?? []);
	const busList = $derived(ins?.buses ?? []);
	function study() {
		const b = branchList[faultBranch];
		const fault: StaticFault =
			faultKind === 'line' && b
				? {
						kind: 'line',
						from: b[0],
						to: b[1],
						circuit: b[2],
						location: Math.min(0.999, Math.max(0.001, faultAt / 100))
					}
				: { kind: 'bus', bus: faultBus ?? busList[0] };
		void act('Static protection study…', () => prot.studyFault(fault));
	}
	const shownStudy = $derived(
		prot.study && prot.study.localId === local?.id ? prot.study.study : null
	);

	// ----- record of what is shown -----
	const view = $derived(prot.view);
	const ops = $derived(
		(view?.ops ?? []).filter(
			(o: RelayOperation) =>
				o.kind !== 'reset' && (o.kind !== 'pickup' || o.relay === prot.selectedRelay)
		)
	);
	const trips = $derived((view?.ops ?? []).filter((o) => o.kind === 'trip').length);
	const seek = (t: number) => {
		if (view?.kind === 'run') dyn.player.seek(t);
	};
</script>

<section class="prot dyn-form" aria-label="Protection" data-testid="dyn-protection">
	<p class="dim">
		Relays on every line end, transformer, busbar and unit, set by rule. Switched on, they act in
		every run and in the live view. <span class="pill-experimental">Experimental</span>
	</p>
	<div class="layer">
		<label class="switch"
			><input
				type="checkbox"
				checked={!!p?.enabled}
				disabled={!!busy || !ready}
				onchange={(e) => toggleLayer(e.currentTarget.checked)}
			/>
			<b>Protection layer</b></label
		>
		<span class="dim">{p?.enabled ? 'acts in load flow and RMS' : p ? 'off: settings kept' : 'off'}</span>
	</div>
	{#if !ready}
		<p class="dim">The case needs dynamic data first: relays act in the dynamic simulation.</p>
	{:else if !p}
		<p class="lead">
			Distance (21) with POTT and power swing blocking, line, transformer, busbar and unit
			differential (87), overcurrent (50/51/67), voltage and frequency (27/59/81/81R), generator
			functions (24/32/40/46/49/78), inverter ride-through, breaker failure (50BF) and auto-reclose
			(79), set from the case's impedances and a short-circuit calculation.
		</p>
		<div class="row">
			<label class="inline"
				>Distance <select bind:value={characteristic}
					><option value="mho">mho</option><option value="quadrilateral">quadrilateral</option></select
				></label
			>
			<button class="primary" disabled={!!busy} onclick={setByRule}>Set protection by rule</button>
		</div>
		<label class="secondary file"
			>Load scheme (.json)<input type="file" accept=".json" onchange={upload} hidden /></label
		>
	{:else}
		<div class="row">
			<span class="chip">{p.scheme.relays.length} relays</span>
			<span class="grow"></span>
			<button class="secondary" onclick={setByRule} disabled={!!busy}>Reset by rule</button>
			<button class="secondary" onclick={download}>Save</button>
			<label class="secondary file"
				>Load<input type="file" accept=".json" onchange={upload} hidden /></label
			>
		</div>
		<div class="subtabs" role="tablist" aria-label="Protection views">
			{#each [['relays', 'Relays'], ['audit', `Audit${auditCounts.error + auditCounts.warning ? ` · ${auditCounts.error + auditCounts.warning}` : ''}`], ['tests', `Tests${p.suite ? ` · ${p.suite.passed}/${p.suite.tests.length}` : ''}`], ['study', 'Load flow'], ['record', 'Record']] as [id, label] (id)}
				<button
					role="tab"
					aria-selected={sub === id}
					onclick={() => (sub = id as Sub)}>{label}</button
				>
			{/each}
		</div>

		{#if sub === 'relays'}
			<p class="dim">Pick a relay (here or its dot on the map) to draw its zones along the lines.</p>
			{#each grouped as g (g.kind)}
				<details open={g.kind === 'line' || g.relays.length < 8}>
					<summary>{g.label} <span class="dim">{g.relays.length}</span></summary>
					<ul class="relays">
						{#each g.relays as r, ri (`${r.id}|${ri}`)}
							<li class:selected={prot.selectedRelay === r.id} class:off={r.enabled === false}>
								<button class="relay" onclick={() => pick(r.id)}
									><span class="mono">{r.id}</span><span class="codes">{relayCodes(r)}</span></button
								>
								<input
									type="checkbox"
									title="in service"
									aria-label="{r.id} in service"
									checked={r.enabled ?? true}
									disabled={!!busy}
									onchange={() => toggleRelay(r.id)}
								/>
							</li>
						{/each}
					</ul>
				</details>
			{/each}
			{#if selected}
				<div class="detail">
					<h4>{selected.id}</h4>
					{#if zonesOf.length}
						<table class="zones">
							<thead><tr><th>Zone</th><th>Reach</th><th>Delay</th><th>Dir</th></tr></thead>
							<tbody>
								{#each zonesOf as z, zi (zi)}
									<tr
										><td><span class="swatch {z.name}"></span>{z.name}</td><td class="num"
											>{z.reach.toFixed(2)} Ω</td
										><td class="num">{z.delay.toFixed(2)} s</td><td>{z.direction ?? 'forward'}</td></tr
									>
								{/each}
							</tbody>
						</table>
					{/if}
					<ul class="fns">
						{#each selected.functions.filter((f) => f.type !== 'distance') as f, i (i)}
							<li>
								<span class="mono">{'label' in f && f.label ? f.label : f.type.replace(/_/g, ' ')}</span>
								<span class="dim"
									>{'pickup' in f && f.pickup != null ? `pickup ${f.pickup.toFixed(f.pickup < 2 ? 3 : 1)}` : ''}
									{'delay' in f && f.delay != null ? ` · ${f.delay} s` : ''}
									{'curve' in f && f.curve && f.curve !== 'definite'
										? ` · ${f.curve.replace(/_/g, ' ')}, TMS ${f.tms?.toFixed(2)}`
										: ''}</span
								>
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		{:else if sub === 'audit'}
			<div class="row counts">
				<span class="sev error">{auditCounts.error} errors</span>
				<span class="sev warning">{auditCounts.warning} warnings</span>
				<span class="sev info">{auditCounts.info} notes</span>
				<span class="sev ok">{auditCounts.ok} ok</span>
				<label class="inline"><input type="checkbox" bind:checked={showOk} /> show ok</label>
			</div>
			<ul class="findings">
				{#each findings as f, i (i)}
					<li class={f.severity}>
						<button class="relay" onclick={() => (prot.selectedRelay = f.relay)}
							><span class="mono">{f.relay}</span></button
						>
						<span class="check">{f.check}</span>
						<p>{f.message}</p>
					</li>
				{/each}
			</ul>
			{#if p.notes.length}
				<details>
					<summary>Setting basis</summary>
					<ul class="notes">
						{#each p.notes as n, i (i)}<li>{n}</li>{/each}
					</ul>
				</details>
			{/if}
		{:else if sub === 'tests'}
			<div class="row">
				<select bind:value={category} aria-label="Test category"
					><option value="all">all {p.tests.length} tests</option>{#each categories as c (c)}<option
							value={c}>{c.replace(/_/g, ' ')}</option
						>{/each}</select
				>
				<button
					class="primary"
					disabled={!!busy || prot.suiteRunning}
					onclick={() => runTests(category === 'all' ? undefined : tests)}
					>Run {category === 'all' ? 'all' : category.replace(/_/g, ' ')}</button
				>
				{#if prot.suiteRunning}<button class="secondary" onclick={() => prot.stopSuite()}
						>Stop</button
					>{/if}
			</div>
			{#if p.suite}
				<p class="dim">
					{p.suite.passed} passed, {p.suite.failed} failed · RMS, relays with a {(
						(p.scheme.measuring_time ?? 0.02) * 1000
					).toFixed(0)} ms measuring time. ▶ replays a test on the map.
				</p>
			{/if}
			<ul class="tests">
				{#each tests as test, ti (`${test.name}|${ti}`)}
					{@const o = outcome(test.name)}
					<li class:pass={o?.passed} class:fail={o && !o.passed}>
						<div class="trow">
							<span class="mark">{o ? (o.passed ? '✓' : '✗') : '·'}</span>
							<button
								class="tname"
								aria-expanded={expanded === test.name}
								onclick={() => (expanded = expanded === test.name ? null : test.name)}
								>{test.name}</button
							>
							<button
								class="secondary small"
								title="replay on the map"
								aria-label="Replay {test.name}"
								disabled={!!busy || dyn.running}
								onclick={() => play(test)}>▶</button
							>
						</div>
						{#if expanded === test.name}
							{#if test.description}<p class="dim">{test.description}</p>{/if}
							{#if o}
								<ul class="checks">
									{#each o.checks as c, i (i)}
										<li class:bad={!c.passed}>
											{c.passed ? '✓' : '✗'}
											{c.expectation} <span class="dim">— {c.observed}</span>
										</li>
									{/each}
								</ul>
								{#if o.error}<p class="err">{o.error}</p>{/if}
							{:else}
								<ul class="checks">
									{#each test.expect as x, i (i)}<li class="dim">{describeExpectation(x)}</li>{/each}
								</ul>
							{/if}
						{/if}
					</li>
				{/each}
			</ul>
		{:else if sub === 'study'}
			<h4>At the operating point</h4>
			{#if !p.enabled}
				<p class="dim">Switch the layer on.</p>
			{:else if operating}
				<p class="dim">
					{operating.pickups.length
						? `${operating.pickups.length} function(s) picked up: ${operating.pickups.map((o) => `${o.relay} ${o.function}`).join(', ')}`
						: 'No relay picks up at the operating point (load, voltages and load impedances inside the settings).'}
				</p>
			{:else}
				<p class="dim">Evaluating…</p>
			{/if}
			<h4>Static study of a fault</h4>
			<p class="dim">
				The relays on the short circuit (loads neglected): breakers open in sequence and the short
				circuit is solved again, until the fault is isolated.
			</p>
			<div class="row">
				<select bind:value={faultKind} aria-label="Fault kind"
					><option value="line">fault on line</option><option value="bus">bus fault</option></select
				>
				{#if faultKind === 'line'}
					<select bind:value={faultBranch} aria-label="Line"
						>{#each branchList as b, i (i)}<option value={i}
								>{b[0]}–{b[1]}{b[2].trim() === '1' ? '' : ` (${b[2].trim()})`}</option
							>{/each}</select
					>
					<label class="inline"
						>at <input class="num" type="number" min="1" max="99" step="5" bind:value={faultAt} /> %</label
					>
				{:else}
					<select bind:value={faultBus} aria-label="Bus"
						>{#each busList as b (b)}<option value={b}>bus {b}</option>{/each}</select
					>
				{/if}
				<button class="primary" disabled={!!busy || !p.enabled} onclick={study}>Study</button>
			</div>
			{#if shownStudy}
				<p class="dim">
					{shownStudy.fault_current_ka.toFixed(2)} kA initial · {shownStudy.cleared_at != null
						? `isolated after ${(shownStudy.cleared_at * 1000).toFixed(0)} ms`
						: 'not isolated'}
				</p>
				<ol class="ops">
					{#each shownStudy.steps as st, i (i)}
						<li class="trip past">
							<span class="opt">{(st.time * 1000).toFixed(0)} ms</span><span>{st.opened.join(', ')}</span
							><span class="dim">→ {st.fault_current_ka.toFixed(2)} kA</span>
						</li>
					{/each}
				</ol>
				<details>
					<summary>What each relay sees at the inception</summary>
					<table class="zones">
						<thead><tr><th>Relay</th><th>I kA</th><th>V pu</th><th>Z Ω</th></tr></thead>
						<tbody>
							{#each shownStudy.seen.filter((x) => x.current_ka > 0.01) as x, xi (`${x.relay}|${xi}`)}
								<tr
									><td class="mono">{x.relay}</td><td class="num">{x.current_ka.toFixed(2)}</td><td
										class="num">{x.voltage_pu.toFixed(3)}</td
									><td class="num"
										>{x.impedance ? `${x.impedance[0].toFixed(1)} + j${x.impedance[1].toFixed(1)}` : ''}</td
									></tr
								>
							{/each}
						</tbody>
					</table>
				</details>
			{/if}
		{:else if view && view.kind !== 'planned' && view.recorded}
			<p class="dim">
				<span class="mono">{view.label}</span> · {trips} trip{trips === 1 ? '' : 's'}{view.kind === 'run'
					? ` · t = ${view.t.toFixed(3)} s`
					: ''}
			</p>
			<ol class="ops">
				{#each ops as o, i (i)}
					<li class={o.kind} class:past={o.time <= view.t + 1e-9}>
						{#if view.kind === 'run'}
							<button class="opt" onclick={() => seek(o.time)}>{o.time.toFixed(3)} s</button>
						{:else}
							<span class="opt">{o.time.toFixed(3)} s</span>
						{/if}
						<button class="relay" onclick={() => (prot.selectedRelay = o.relay)}
							><span class="mono">{o.relay}</span></button
						>
						<span>{o.function} {o.kind.replace(/_/g, ' ')}</span>
						{#if o.detail}<span class="dim">{o.detail}</span>{/if}
					</li>
				{:else}
					<li class="dim">No relay has operated.</li>
				{/each}
			</ol>
		{:else}
			<p class="dim">
				Run the dynamics with the relays on, replay a test, or study a fault: the relay record shows
				here.
			</p>
		{/if}
	{/if}
	{#if busy}<p class="dim" role="status">{busy}</p>{/if}
	{#if error}<p class="err" role="alert">{error}</p>{/if}
</section>

<style>
	.prot {
		display: grid;
		gap: 8px;
		grid-template-columns: minmax(0, 1fr);
		min-width: 0;
		font-size: var(--fs-control);
	}
	p {
		margin: 0;
	}
	.lead {
		line-height: var(--lh-snug, 1.35);
	}
	h4 {
		margin: 4px 0 0;
		font-size: var(--fs-label);
		font-weight: 600;
		color: var(--text-secondary);
	}
	.layer {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		padding: 6px 8px;
		border-radius: var(--radius-sm);
		background: var(--surface-chip);
	}
	.switch,
	.inline {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.grow {
		flex: 1;
	}
	.chip {
		padding: 1px 8px;
		border-radius: var(--radius-control);
		background: var(--surface-chip);
	}
	.primary,
	.secondary {
		min-height: 28px;
		padding: 4px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.primary {
		border-color: var(--accent-fill);
		background: var(--accent-fill);
		color: var(--on-accent);
		font-weight: 600;
	}
	.primary:disabled,
	.secondary:disabled {
		border-color: transparent;
		background: var(--surface-hover);
		color: var(--text-tertiary);
		cursor: default;
	}
	.small {
		min-height: 22px;
		padding: 0 6px;
	}
	.file {
		align-self: flex-start;
		justify-self: start;
	}
	.subtabs {
		display: flex;
		gap: 2px;
		border-bottom: 1px solid var(--line);
	}
	.subtabs button {
		position: relative;
		border: none;
		background: none;
		padding: 4px 6px;
		font: inherit;
		color: var(--text-secondary);
		cursor: pointer;
	}
	.subtabs button[aria-selected='true'] {
		color: var(--ink);
	}
	.subtabs button[aria-selected='true']::after {
		content: '';
		position: absolute;
		left: 6px;
		right: 6px;
		bottom: -1px;
		height: 2px;
		background: var(--accent);
	}
	details summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	ul,
	ol {
		list-style: none;
		margin: 4px 0 0;
		padding: 0;
	}
	.relays li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		border-radius: var(--radius-sm);
		padding: 0 4px;
	}
	.relays li.selected {
		background: var(--surface-chip);
		box-shadow: var(--inset-active);
	}
	.relays li.off {
		opacity: 0.5;
	}
	.relay {
		display: inline-flex;
		gap: 8px;
		align-items: baseline;
		border: none;
		background: none;
		padding: 2px 0;
		font: inherit;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}
	.codes {
		color: var(--text-tertiary);
	}
	.detail {
		display: grid;
		gap: 4px;
		padding-top: 6px;
		border-top: 1px solid var(--line);
	}
	table.zones {
		border-collapse: collapse;
	}
	table.zones th,
	table.zones td {
		padding: 2px 8px 2px 0;
		text-align: left;
	}
	td.num {
		text-align: right;
		font-family: var(--font-mono);
	}
	input.num {
		width: 4.5em;
	}
	.swatch {
		display: inline-block;
		width: 10px;
		height: 4px;
		margin-right: 6px;
		border-radius: 2px;
		vertical-align: middle;
		background: #999;
	}
	.swatch.Z1 {
		background: rgb(46, 160, 90);
	}
	.swatch.Z2 {
		background: rgb(230, 168, 23);
	}
	.swatch.Z3 {
		background: rgb(232, 112, 40);
	}
	.fns li {
		display: flex;
		gap: 8px;
	}
	.counts .sev {
		padding: 1px 8px;
		border-radius: var(--radius-control);
	}
	.sev.error {
		background: color-mix(in srgb, var(--text-danger) 15%, transparent);
	}
	.sev.warning {
		background: color-mix(in srgb, var(--amber, #e6a817) 20%, transparent);
	}
	.sev.info {
		background: color-mix(in srgb, var(--accent) 12%, transparent);
	}
	.sev.ok {
		background: color-mix(in srgb, #2ea05a 14%, transparent);
	}
	.findings li {
		border-left: 3px solid var(--line);
		padding: 2px 0 4px 8px;
		margin-bottom: 4px;
	}
	.findings li.error {
		border-color: var(--text-danger);
	}
	.findings li.warning {
		border-color: var(--amber, #e6a817);
	}
	.findings li.info {
		border-color: var(--accent);
	}
	.findings li.ok {
		border-color: #2ea05a;
	}
	.findings .check {
		margin-left: 6px;
		color: var(--text-secondary);
	}
	.findings p {
		margin: 2px 0 0;
	}
	.notes li {
		margin-bottom: 2px;
		color: var(--text-tertiary);
	}
	.tests li {
		padding: 2px 0;
		border-bottom: 1px solid var(--line);
	}
	.trow {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.mark {
		width: 1em;
		text-align: center;
		color: var(--text-tertiary);
	}
	.pass .mark {
		color: #2ea05a;
	}
	.fail .mark {
		color: var(--text-danger);
	}
	.tname {
		flex: 1;
		border: none;
		background: none;
		padding: 2px 0;
		font: inherit;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}
	.checks li {
		margin: 2px 0 2px 1.4em;
	}
	.checks li.bad {
		color: var(--text-danger);
	}
	.ops li {
		display: flex;
		gap: 6px;
		align-items: baseline;
		flex-wrap: wrap;
		padding: 1px 0;
		opacity: 0.55;
	}
	.ops li.past {
		opacity: 1;
	}
	.ops li.trip,
	.ops li.breaker_failure,
	.ops li.lockout {
		font-weight: 600;
	}
	.opt {
		border: none;
		background: none;
		padding: 0;
		font: inherit;
		font-family: var(--font-mono);
		color: var(--text-secondary);
		cursor: pointer;
	}
	.mono {
		font-family: var(--font-mono);
	}
	.dim {
		color: var(--text-tertiary);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
</style>
