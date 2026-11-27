<script lang="ts">
	/** Live disturbances for the selected RMS case.
	 * @block Live disturbances */
	import type { RmsEvent } from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import {
		busFault as busFaultEvent,
		DEFAULT_FAULT_X,
		generatorTrip,
		loadStep,
		unitKey
	} from '../../dynamics/events.js';
	const app = getAppState();
	const dyn = getController().dynamics;
	const live = dyn.live;
	const ins = $derived(dyn.active?.inspection ?? null);
	// ----- disturbances -----
	let faultBus = $state<number | null>(null);
	let faultMs = $state(100);
	let faultX = $state(DEFAULT_FAULT_X);
	let branch = $state(0);
	let lineAt = $state(50);
	let lineMs = $state(100);
	let loadBus = $state<number | null>(null);
	let loadPct = $state(10);
	let gen = $state(0);
	let refs = $state<Record<string, { v: number; p: number }>>({});
	// A new session starts from the case's references: the sliders too.
	$effect(() => {
		void live.generation;
		refs = {};
	});
	$effect(() => {
		if (!ins) return;
		if (faultBus === null || !ins.buses.includes(faultBus))
			faultBus = ins.generators[0]?.bus ?? ins.buses[0] ?? null;
		if (loadBus === null || !ins.load_buses.includes(loadBus)) loadBus = ins.load_buses[0] ?? null;
	});
	// Keep the disturbance target in sync with map selection.
	$effect(() => {
		const b = app.selectedBus;
		if (b === null || b === undefined || !ins?.buses.includes(b)) return;
		faultBus = b;
		if (ins.load_buses.includes(b)) loadBus = b;
	});
	const on = $derived(live.active && live.running);
	const busFault = () =>
		faultBus !== null &&
		live.now(
			busFaultEvent(faultBus, ins!.base_frequency, { at: 0, clearAfter: faultMs / 1000, x: faultX })
		);
	const br = () => ins?.branches[branch];
	const lineEv = (kind: 'branch_trip' | 'branch_close'): RmsEvent => {
		const [from, to, circuit] = br()!;
		return { kind, time: 0, from, to, circuit };
	};
	function lineFault() {
		const [from, to, circuit] = br()!;
		live.applyNow(
			{
				kind: 'line_fault',
				time: 0,
				from,
				to,
				circuit,
				location: lineAt / 100,
				r: 0,
				x: DEFAULT_FAULT_X,
				permanent: true
			},
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
	let kind = $state('bus');
</script>

{#if ins}
	<section class="disturbances" aria-label="Live disturbances">
		<label>
			Disturbance
			<select bind:value={kind}>
				<option value="bus">Bus fault</option>
				<option value="line">Line switching or fault</option>
				<option value="load">Load step</option>
				<option value="unit">Generator controls</option>
			</select>
		</label>
		{#if kind === 'bus'}
			<div class="row">
				<select bind:value={faultBus} aria-label="fault bus">
					{#each ins.buses as b (b)}<option value={b}>bus {b}</option>{/each}
				</select>
				<label>
					Clear after <input type="number" min="10" step="10" bind:value={faultMs} /> ms
				</label>
				<label>X (pu) <input type="number" min="0.0001" step="0.0001" bind:value={faultX} /></label>
				<button
					class="act"
					disabled={!on ||
						faultBus === null ||
						!Number.isFinite(faultMs) ||
						faultMs < 10 ||
						!Number.isFinite(faultX) ||
						faultX <= 0}
					onclick={busFault}
				>
					Apply fault
				</button>
			</div>
		{:else if kind === 'line'}
			<div class="row">
				<select bind:value={branch} aria-label="line">
					{#each ins.branches as [a, b, c], i (i)}<option value={i}>
							{a}–{b}{c === '1' ? '' : ` (${c})`}
						</option>{/each}
				</select>
				<button
					class="act"
					disabled={!on || !br()}
					onclick={() => live.applyNow(lineEv('branch_trip'))}
				>
					Trip
				</button>
				<button
					class="act"
					disabled={!on || !br()}
					onclick={() => live.applyNow(lineEv('branch_close'))}
				>
					Close
				</button>
			</div>
			<div class="row">
				<label>
					fault at <input type="number" min="1" max="99" step="5" bind:value={lineAt} /> %
				</label>
				<label>open after <input type="number" min="10" step="10" bind:value={lineMs} /> ms</label>
				<button
					class="act"
					disabled={!on ||
						!br() ||
						!Number.isFinite(lineAt) ||
						lineAt < 1 ||
						lineAt > 99 ||
						!Number.isFinite(lineMs) ||
						lineMs < 10}
					onclick={lineFault}
				>
					Apply line fault
				</button>
			</div>
		{:else if kind === 'load'}
			<div class="row">
				<select bind:value={loadBus} aria-label="load bus">
					{#each ins.load_buses as b (b)}<option value={b}>bus {b}</option>{/each}
				</select>
				<label>
					Change <input type="number" min="-100" max="100" step="5" bind:value={loadPct} /> %
				</label>
				<button
					class="act"
					disabled={!on || loadBus === null || !Number.isFinite(loadPct) || Math.abs(loadPct) > 100}
					onclick={() => loadBus !== null && live.now(loadStep(loadBus, loadPct))}
				>
					Step
				</button>
			</div>
		{:else}
			<div class="row">
				<select bind:value={gen} aria-label="unit">
					{#each ins.generators as g, i (i)}<option value={i}>
							G{g.bus}{g.machine_id.trim() === '1' ? '' : ` '${g.machine_id.trim()}'`}
						</option>{/each}
				</select>
				<button
					class="act"
					disabled={!on || !unit}
					onclick={() => unit && live.now(generatorTrip(unit.bus, unit.machine_id))}
				>
					Trip
				</button>
			</div>
			{#if unit && ref}
				<label class="slider">
					<span>Voltage reference</span>
					<input
						type="range"
						min={unit.v_ref * 0.9}
						max={unit.v_ref * 1.1}
						step={unit.v_ref * 0.002}
						value={ref.v}
						disabled={!on}
						onchange={(e) => setRef('v_ref', Number(e.currentTarget.value))}
					/><span class="mono">{ref.v.toFixed(3)}</span>
				</label>
				<label class="slider">
					<span>Power reference</span>
					<input
						type="range"
						min={unit.p_ref - 0.3}
						max={unit.p_ref + 0.3}
						step="0.005"
						value={ref.p}
						disabled={!on}
						onchange={(e) => setRef('p_ref', Number(e.currentTarget.value))}
					/><span class="mono">{ref.p.toFixed(3)}</span>
				</label>
			{/if}
		{/if}
		{#if live.log.length}
			<h5>Event log</h5>
			<ol class="log">
				{#each [...live.log].reverse() as [t, text], i (i)}
					<li><span class="mono">{t.toFixed(2)} s</span> {text}</li>
				{:else}
					<li class="dim">No events yet.</li>
				{/each}
			</ol>
		{/if}
	</section>
{/if}

<style>
	.disturbances {
		display: grid;
		gap: 8px;
	}
	h5,
	ol {
		margin: 0;
	}
	.log {
		padding-left: 1.4em;
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
		grid-template-columns: minmax(6em, 1fr) minmax(50px, 2fr) 5ch;
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
</style>
