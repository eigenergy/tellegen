<script lang="ts">
	import { rmsDynamize } from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import PanelFrame from '../PanelFrame.svelte';
	import DynamicsEventList from './DynamicsEventList.svelte';
	import DynamicsRunControls from './DynamicsRunControls.svelte';
	import DynamicsVerdict from './DynamicsVerdict.svelte';
	import DynamicsNotes from './DynamicsNotes.svelte';
	import EigenPlot from './EigenPlot.svelte';
	import { headline } from '../../dynamics/verdict.js';
	import { onTabKey } from '../../dynamics/tabs.js';
	import ModeSwitch from './ModeSwitch.svelte';

	const app = getAppState();
	const ctrl = getController();
	const dyn = ctrl.dynamics;

	let open = $state(false);
	let tab = $state<'run' | 'modes'>('run');
	let demoBusy = $state(false);
	let demoError = $state<string | null>(null);
	const att = $derived(dyn.active);
	const ins = $derived(att?.inspection ?? null);
	const experimental = $derived(app.preferences.experimental && !!dyn.support?.experimental);
	const local = $derived(app.activeLocal);
	const current = $derived(dyn.current);
	const hosted = $derived(!!app.active && !local);

	// The panel opens with the mode, and the mode follows the panel.
	$effect(() => {
		if (app.mode === 'dynamics') open = true;
	});
	$effect(() => {
		void dyn.probe();
	});

	async function demo() {
		demoBusy = true;
		demoError = null;
		try {
			await dyn.openDemo();
		} catch (e) {
			demoError = (e as Error).message;
		} finally {
			demoBusy = false;
		}
	}

	// Experimental: default dynamics for a case without .dyr data.
	let preset = $state('steam_wscc_192');
	let dynamizeBusy = $state(false);
	let dynamizeError = $state<string | null>(null);
	async function dynamize() {
		if (!local?.studyInputJson) return;
		dynamizeBusy = true;
		dynamizeError = null;
		try {
			const text = await rmsDynamize(local.studyInputJson, '', { preset });
			await dyn.attach(local, 'default dynamics (typical parameters).dyr', text, false, true);
		} catch (e) {
			dynamizeError = (e as Error).message;
		} finally {
			dynamizeBusy = false;
		}
	}

	const counts = $derived(
		ins
			? `${ins.generators.length} machines, ${ins.plants?.length ?? 0} renewable plants simulated, ${ins.held_units?.length ?? 0} held, ${ins.sources.length} voltage sources and ${ins.dyr_records.toLocaleString()} .dyr records.`
			: ''
	);
	const flatOk = $derived(!!att?.flat && att.flat.max_speed_drift <= 1e-6);
</script>

<PanelFrame
	id="dynamics"
	title="Dynamics"
	side="right"
	order={25}
	tab
	width={380}
	bind:open
	onopen={() => (app.mode = 'dynamics')}
>
	<div class="dyn" data-testid="dynamics-panel">
		<div class="dyn-mode"><span>Map shows</span><ModeSwitch /></div>
		{#if !att}
			<section class="empty">
				<h3>Dynamics simulation</h3>
				<p>
					Simulate the first seconds after a fault, a line or unit trip, or a load step. The map
					shows bus voltages and machine speeds as they swing. The result says whether the machines
					stayed in step and how far frequency fell.
				</p>
				<p>
					It needs PSS/E dynamic data (.dyr).{hosted ? ' This case has none.' : ''} Drop a .raw case with
					its .dyr file on the map, or open the demo.
				</p>
				<button class="secondary" disabled={demoBusy} onclick={demo} data-testid="dyn-open-demo"
					>{demoBusy ? 'Opening WECC 240…' : 'Open the WECC 240 demo'}</button
				>
				<p class="dim">
					243 buses of the western US grid, with machines, exciters, governors and 37 renewable
					plants.
				</p>
				{#if demoError}<p class="err" role="alert">{demoError}</p>{/if}
				{#if experimental && local?.studyInputJson}
					<div class="dynamize">
						<h4>Add default dynamics <span class="pill-experimental">Experimental</span></h4>
						<p class="dim">
							Gives every generator of {local.label} typical models. Synthetic dynamic data: typical parameters,
							not the owner's.
						</p>
						<label
							>Unit type <select bind:value={preset}>
								<option value="steam_wscc_192">Steam (WSCC 192 MVA)</option>
								<option value="hydro_wscc_247">Hydro (WSCC 247 MVA)</option>
								<option value="gas_turbine">Gas turbine</option>
								<option value="steam_kundur_555">Steam (Kundur 555 MVA)</option>
							</select></label
						>
						<button disabled={dynamizeBusy} onclick={dynamize}
							>{dynamizeBusy ? 'Adding…' : 'Add default dynamics'}</button
						>
						{#if dynamizeError}<p class="err">{dynamizeError}</p>{/if}
					</div>
				{/if}
			</section>
		{:else}
			<section class="data" aria-label="Dynamic data">
				<p class="file">
					<span class="mono">{att.fileName}</span>
					{#if att.synthetic}<span class="pill-experimental">Synthetic dynamic data</span>{/if}
				</p>
				{#if att.status === 'opening'}
					<p class="dim" role="status">Initializing from the AC power flow…</p>
				{:else if att.status === 'failed'}
					<p class="err" role="alert">{att.error}</p>
				{:else if ins}
					<p data-testid="dyn-counts">{counts}</p>
					{#if att.preparing}
						<p class="dim" role="status" data-testid="dyn-preparing">
							Initializing again with the new settings…
						</p>
					{:else if att.prepareError}
						<p class="err" role="alert">{att.prepareError}</p>
					{/if}
					<dl class="init" aria-busy={att.preparing}>
						{#if att.network === 'validation'}
							<dt>Network</dt>
							<dd data-testid="dyn-network">the validation study's loads (case-validation.raw)</dd>
						{/if}
						<dt>Initial residual</dt>
						<dd>‖f(x₀)‖∞ = {(ins.initial_residual ?? 0).toExponential(1)}</dd>
						<dt>Power flow</dt>
						<dd>
							AC, reactive limits {ins.options?.reactive_limits === false
								? 'not enforced'
								: 'enforced'}
						</dd>
						<dt>Angle reference</dt>
						<dd>{ins.reference_unit ? `unit ${ins.reference_unit}` : 'synchronous frame'}</dd>
						<dt>Flat start</dt>
						<dd data-testid="dyn-flat">
							{#if att.flat}
								max speed drift {att.flat.max_speed_drift.toExponential(0)} pu over {att.flat
									.seconds} s
								{flatOk ? '✓' : '✗'}
								{#if !flatOk}<br /><span class="warn"
										>the case is not at rest; results include that drift</span
									>{/if}
							{:else if att.flatError}
								<span class="err">{att.flatError}</span>
							{:else}
								checking…
							{/if}
						</dd>
					</dl>
					{#if dyn.staticEdits > 0}
						<p class="warn">
							Dynamics uses the case as loaded; {dyn.staticEdits} steady-state edit{dyn.staticEdits ===
							1
								? ' is'
								: 's are'} not included.
						</p>
					{/if}
					<details>
						<summary>Initialization notes ({ins.notes.length})</summary>
						<DynamicsNotes notes={ins.notes} />
					</details>
				{/if}
			</section>

			{#if experimental}
				<div
					class="tabs"
					role="tablist"
					aria-label="Dynamics views"
					tabindex="-1"
					onkeydown={(e) => onTabKey(e, ['run', 'modes'] as const, tab, (t) => (tab = t))}
				>
					<button
						role="tab"
						id="dyn-tab-run"
						aria-selected={tab === 'run'}
						aria-controls="dyn-tabpanel"
						tabindex={tab === 'run' ? 0 : -1}
						onclick={() => (tab = 'run')}>Run</button
					>
					<button
						role="tab"
						id="dyn-tab-modes"
						aria-selected={tab === 'modes'}
						aria-controls="dyn-tabpanel"
						tabindex={tab === 'modes' ? 0 : -1}
						onclick={() => (tab = 'modes')}
						>Modes <span class="pill-experimental">Experimental</span></button
					>
				</div>
			{/if}

			<div
				class="tabpanel"
				id="dyn-tabpanel"
				role={experimental ? 'tabpanel' : undefined}
				aria-labelledby={experimental
					? tab === 'modes'
						? 'dyn-tab-modes'
						: 'dyn-tab-run'
					: undefined}
			>
				{#if tab === 'modes' && experimental}
					<section aria-label="Small-signal modes" data-testid="dyn-modes">
						<p class="dim">
							Eigenvalues of the system linearized at the operating point, with the states that take
							part most in each mode. <span class="pill-experimental">Experimental</span>
						</p>
						{#if dyn.modes}
							<EigenPlot result={dyn.modes} bind:selected={dyn.selectedMode} />
						{:else}
							{#if dyn.dynamicStates > 300}
								<p class="warn" data-testid="dyn-modes-size">
									A dense analysis of {dyn.dynamicStates.toLocaleString()} machine states plus the network
									and plant states: it can take many minutes in the browser. Stop ends it.
								</p>
							{/if}
							{#if dyn.modesBusy}
								<p role="status">
									Computing modes… <button class="link" onclick={() => dyn.stopModes()}>Stop</button
									>
								</p>
							{:else}
								<button
									disabled={att.status !== 'ready' || dyn.running}
									title={dyn.running ? 'A run is in progress' : undefined}
									onclick={() => dyn.computeModes()}>Compute modes</button
								>
							{/if}
						{/if}
						{#if dyn.modesError}<p class="err">{dyn.modesError}</p>{/if}
					</section>
				{:else if att.status === 'ready'}
					<DynamicsEventList />
					<DynamicsRunControls />
					{#if current}
						<DynamicsVerdict record={current} experimental={!!att.synthetic} />
						{#if current.run}
							<details>
								<summary>Run notes and events</summary>
								<DynamicsNotes
									notes={current.run.notes.slice(ins?.notes.length ?? 0)}
									events={current.run.events}
								/>
							</details>
							<p class="footer">
								{current.run.steps.toLocaleString()} steps and {current.run.newtonIterations.toLocaleString()}
								Newton iterations. Engine time {(current.run.workerMs / 1000).toFixed(2)} s, total {(
									current.wallMs / 1000
								).toFixed(2)} s. Time step {current.dt * 1000} ms, output every {current.run
									.outputStep * 1000} ms. Load model: {(
									current.options.load_model ?? 'power_flow'
								).replaceAll('_', ' ')}.
								<button
									class="link"
									onclick={() => navigator.clipboard?.writeText(dyn.runJson(current))}
									>Copy run as JSON</button
								>
							</p>
						{/if}
					{/if}
					{#if dyn.runs.length > 1}
						<div class="history" aria-label="Recent runs">
							{#each dyn.runs as r (r.id)}
								{@const h = headline(r.run?.screening ?? null, r.failure, r.tEnd)}
								<button aria-pressed={r.id === dyn.currentRunId} onclick={() => dyn.select(r.id)}>
									<span aria-hidden="true">{h.icon}</span>
									{r.events.length ? dyn.describe(r.events[0]).split(' at ')[0] : 'No events'}{r
										.events.length > 1
										? ` +${r.events.length - 1}`
										: ''}
									{#if r.agent}<span class="agent">agent</span>{/if}
								</button>
							{/each}
						</div>
					{/if}
				{/if}
			</div>
		{/if}
	</div>
</PanelFrame>

<style>
	.tabpanel {
		display: contents;
	}
	.dyn-mode {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-secondary);
	}
	.dyn {
		display: grid;
		gap: 12px;
		padding: 12px 14px 16px;
		font-size: var(--fs-label);
	}
	h3,
	h4 {
		margin: 0 0 6px;
	}
	p {
		margin: 0;
		line-height: 1.45;
	}
	.empty {
		display: grid;
		gap: 8px;
	}
	.secondary {
		justify-self: start;
		min-height: 28px;
		padding: 5px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.secondary:hover:not(:disabled) {
		border-color: var(--border-strong);
	}
	@media (pointer: coarse) {
		.secondary {
			min-height: 44px;
		}
	}
	.dim {
		color: var(--text-tertiary);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
	.warn {
		border-left: 3px solid var(--warn);
		padding-left: 6px;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: var(--fs-data);
	}
	.file {
		display: flex;
		gap: 6px;
		align-items: center;
	}
	.data {
		display: grid;
		gap: 6px;
	}
	.init {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 2px 10px;
		margin: 0;
		font-size: var(--fs-control);
	}
	.init dt {
		color: var(--text-tertiary);
	}
	.init dd {
		margin: 0;
		font-variant-numeric: tabular-nums;
	}
	.tabs {
		display: flex;
		gap: 4px;
		border-bottom: 1px solid var(--line);
	}
	.tabs button {
		border: none;
		background: none;
		padding: 4px 8px;
		font: inherit;
		color: var(--text-secondary);
		cursor: pointer;
	}
	.tabs button[aria-selected='true'] {
		color: var(--ink);
		box-shadow: var(--inset-active);
	}
	.footer {
		font-size: var(--fs-meta);
		color: var(--text-tertiary);
	}
	.link {
		border: none;
		background: none;
		padding: 0;
		color: var(--text-accent);
		cursor: pointer;
		font: inherit;
		text-decoration: underline;
	}
	.history {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.history button {
		padding: 2px 8px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-control);
		background: var(--surface-chip);
		color: var(--ink);
		font: inherit;
		font-size: var(--fs-control);
		cursor: pointer;
	}
	.history button[aria-pressed='true'] {
		border-color: var(--accent);
	}
	.agent {
		color: var(--agent);
		font-weight: 600;
	}
	.dynamize {
		display: grid;
		gap: 6px;
		padding: 8px;
		border: 1px dashed var(--border-default);
		border-radius: var(--radius-sm);
	}
	details summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
</style>
