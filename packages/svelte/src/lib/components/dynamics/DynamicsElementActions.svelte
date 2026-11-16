<script lang="ts">
	import { getAppState, getController } from '../../context.svelte.js';
	import {
		branchTrip,
		busFault,
		circuitsBetween,
		generatorTrip,
		lineFault,
		loadStep,
		type DynEvent
	} from '../../dynamics/events.js';

	const app = getAppState();
	const ctrl = getController();
	const dyn = ctrl.dynamics;

	const att = $derived(dyn.active);
	const inspection = $derived(att?.inspection ?? null);
	const f0 = $derived(inspection?.base_frequency ?? 60);
	const bus = $derived(app.selectedBus);
	const branch = $derived(
		app.selectedBranch === null
			? null
			: (app.activeLocal?.view?.branches.find((b) => b.id === app.selectedBranch) ?? null)
	);
	const busName = $derived(
		bus === null ? null : (app.activeLocal?.view?.buses.find((b) => b.id === bus)?.name ?? null)
	);
	const hasBus = $derived(bus !== null && !!inspection?.buses.includes(bus));
	const hasLoad = $derived(bus !== null && !!inspection?.load_buses.includes(bus));
	const units = $derived(
		bus === null || !inspection ? [] : inspection.generators.filter((g) => g.bus === bus)
	);
	const held = $derived(
		bus === null || !inspection
			? []
			: [...(inspection.held_units ?? []), ...(inspection.plants ?? [])].filter(
					(k) => Number(k.split('#')[0]) === bus
				)
	);
	const circuits = $derived(
		branch && inspection ? circuitsBetween(inspection.branches, branch.from, branch.to) : []
	);
	let circuit = $state<string | null>(null);
	const chosenCircuit = $derived(
		circuit !== null && circuits.includes(circuit) ? circuit : (circuits[0] ?? null)
	);
	let nearEnd = $state<'from' | 'to'>('from');
	let loadPct = $state(10);
	let message = $state<string | null>(null);

	function add(e: DynEvent) {
		message = dyn.add(e) ?? `Added: ${dyn.describe(e)}`;
	}
</script>

<section class="dyn-actions" aria-label="Dynamics actions">
	{#if !att}
		<p class="dim">No dynamic data</p>
	{:else if att.status !== 'ready'}
		<p class="dim" title={att.status === 'opening' ? undefined : (att.error ?? undefined)}>
			{att.status === 'opening' ? 'Opening…' : 'Failed to open'}
		</p>
	{:else if bus !== null}
		<h4>Bus {bus}{busName ? ` ${busName}` : ''}</h4>
		{#if !hasBus}
			<p class="dim">Not in simulation</p>
		{:else}
			<button class="action" onclick={() => add(busFault(bus, f0))}>⚡ Apply a fault here</button>
			{#if hasLoad}
				<div class="row">
					<button class="action" onclick={() => add(loadStep(bus, loadPct))}
						>Step load {loadPct >= 0 ? '+' : ''}{loadPct} %</button
					>
					<label
						><span class="sr-only">Load step in percent</span><input
							type="number"
							min="-100"
							max="100"
							step="5"
							bind:value={loadPct}
						/> %</label
					>
				</div>
			{/if}
			{#if units.length || held.length}
				<h5>Units</h5>
				<ul class="units">
					{#each units as g (g.machine_id)}
						<li>
							<span class="mono"
								>{g.bus} #{g.machine_id.trim()}
								<span class="dim">{g.models.map((m) => m || '—').join(' · ')}</span>
								<span class="dim">{g.mbase.toFixed(0)} MVA</span></span
							>
							<button onclick={() => add(generatorTrip(g.bus, g.machine_id.trim()))}>Trip</button>
						</li>
					{/each}
					{#each held as k (k)}
						<li>
							<span class="mono"
								>{k}
								<span class="dim">{inspection?.plants?.includes(k) ? 'Renewable' : 'Fixed'}</span
								></span
							>
							<button
								onclick={() => add(generatorTrip(bus, k.includes('#') ? k.split('#')[1] : '1'))}
								>Trip</button
							>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	{:else if branch}
		<h4>Branch {branch.from}–{branch.to}</h4>
		{#if circuits.length === 0}
			<p class="dim">Not in simulation</p>
		{:else}
			{#if circuits.length > 1}
				<fieldset>
					<legend>{branch.from}–{branch.to} · circuits {circuits.join(', ')}</legend>
					{#each circuits as c (c)}
						<label
							><input
								type="radio"
								name="dyn-circuit"
								value={c}
								checked={c === chosenCircuit}
								onchange={() => (circuit = c)}
							/>
							{c}</label
						>
					{/each}
				</fieldset>
			{/if}
			<button
				class="action"
				onclick={() => add(branchTrip(branch.from, branch.to, chosenCircuit ?? '1'))}
				>Trip circuit {chosenCircuit}</button
			>
			<div class="row">
				<button
					class="action"
					onclick={() =>
						add(
							lineFault(
								branch.from,
								branch.to,
								chosenCircuit ?? '1',
								nearEnd === 'from' ? branch.from : branch.to,
								f0
							)
						)}>⚡ Fault on this line</button
				>
				<label
					>near <select bind:value={nearEnd}
						><option value="from">{branch.from}</option><option value="to">{branch.to}</option
						></select
					></label
				>
			</div>
		{/if}
	{:else}
		<p class="dim">Select a bus or line</p>
	{/if}
	{#if message}<p class="msg" role="status">{message}</p>{/if}
</section>

<style>
	.dyn-actions {
		display: grid;
		gap: 8px;
		font-size: var(--fs-label);
	}
	h4,
	h5 {
		margin: 0;
	}
	h5 {
		margin-top: 4px;
		font-size: var(--fs-control);
		color: var(--text-tertiary);
	}
	.action {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		min-height: 36px;
		padding: 6px 10px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.action:hover {
		border-color: var(--accent);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.row input {
		width: 4.5em;
	}
	.units {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 4px;
	}
	.units li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	.units .mono {
		display: flex;
		flex-direction: column;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: var(--fs-data);
	}
	.dim {
		color: var(--text-tertiary);
	}
	.msg {
		margin: 0;
		color: var(--text-secondary);
	}
	fieldset {
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		padding: 4px 8px;
	}
	@media (pointer: coarse) {
		.action,
		.units button {
			min-height: 44px;
		}
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
	}
</style>
