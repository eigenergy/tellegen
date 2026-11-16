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
	import StatBoxes from '../StatBoxes.svelte';

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
			? [
					{ label: 'Machines', value: ins.generators.length.toLocaleString() },
					{ label: 'Renewables', value: (ins.plants?.length ?? 0).toLocaleString() },
					{ label: 'Held', value: (ins.held_units?.length ?? 0).toLocaleString() },
					{ label: '.dyr records', value: ins.dyr_records.toLocaleString() }
				]
			: []
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
		<!-- The switch appears once the case has a dynamic model to show. -->
		{#if att?.status === 'ready'}<div class="dyn-mode">
				<span>Map shows</span><ModeSwitch />
			</div>{/if}
		{#if !att}
			<section class="empty">
				<dl class="rows">
					<div>
						<dt>Dynamic data</dt>
						<dd>{hosted || local ? 'None' : 'No case'}</dd>
					</div>
				</dl>
				<button class="secondary" disabled={demoBusy} onclick={demo} data-testid="dyn-open-demo"
					>{demoBusy ? 'Opening…' : 'Open WECC 240 demo'}</button
				>
				{#if demoError}<p class="err" role="alert" title={demoError}>Demo failed to open</p>{/if}
				{#if experimental && local?.studyInputJson}
					<div class="dynamize">
						<h4>Default dynamics <span class="pill-experimental">Synthetic</span></h4>
						<label
							>Unit type <select bind:value={preset}>
								<option value="steam_wscc_192">Steam (WSCC 192 MVA)</option>
								<option value="hydro_wscc_247">Hydro (WSCC 247 MVA)</option>
								<option value="gas_turbine">Gas turbine</option>
								<option value="steam_kundur_555">Steam (Kundur 555 MVA)</option>
							</select></label
						>
						<button disabled={dynamizeBusy} onclick={dynamize}
							>{dynamizeBusy ? 'Adding…' : 'Add'}</button
						>
						{#if dynamizeError}<p class="err" title={dynamizeError}>Failed</p>{/if}
					</div>
				{/if}
			</section>
		{:else}
			<section class="data" aria-label="Dynamic data">
				<p class="file">
					<span class="mono">{att.fileName}</span>
					{#if att.synthetic}<span class="pill-experimental">Synthetic</span>{/if}
				</p>
				{#if att.status === 'opening'}
					<p class="dim" role="status">Initializing…</p>
				{:else if att.status === 'failed'}
					<p class="err" role="alert">{att.error}</p>
				{:else if ins}
					<div data-testid="dyn-counts"><StatBoxes stats={counts} label="Dynamic model" /></div>
					{#if att.preparing}
						<p class="dim" role="status" data-testid="dyn-preparing">Initializing…</p>
					{:else if att.prepareError}
						<p class="err" role="alert" title={att.prepareError}>Initialization failed</p>
					{/if}
					<dl class="init" aria-busy={att.preparing}>
						{#if att.network === 'validation'}
							<dt>Network</dt>
							<dd data-testid="dyn-network">Validation loads</dd>
						{/if}
						<dt>Initial residual</dt>
						<dd>{(ins.initial_residual ?? 0).toExponential(1)}</dd>
						<dt>Power flow</dt>
						<dd>AC, Q limits {ins.options?.reactive_limits === false ? 'off' : 'on'}</dd>
						<dt>Angle reference</dt>
						<dd>{ins.reference_unit ? `Unit ${ins.reference_unit}` : 'System'}</dd>
						<dt>Flat start</dt>
						<dd data-testid="dyn-flat">
							{#if att.flat}
								Drift {att.flat.max_speed_drift.toExponential(0)} pu
								{flatOk ? '✓' : '✗'}
								{#if !flatOk}<span class="chip-warn">Not at rest</span>{/if}
							{:else if att.flatError}
								<span class="err" title={att.flatError}>Failed</span>
							{:else}
								Checking…
							{/if}
						</dd>
					</dl>
					{#if dyn.staticEdits > 0}
						<p class="warn">Edits ignored: {dyn.staticEdits}</p>
					{/if}
					<details>
						<summary>Notes ({ins.notes.length})</summary>
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
						{#if dyn.modes}
							<EigenPlot result={dyn.modes} bind:selected={dyn.selectedMode} />
						{:else}
							{#if dyn.dynamicStates > 300}
								<p class="warn" data-testid="dyn-modes-size">
									Slow: {dyn.dynamicStates.toLocaleString()} states
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
									title={dyn.running ? 'Run in progress' : undefined}
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
								<summary>Run notes</summary>
								<DynamicsNotes
									notes={current.run.notes.slice(ins?.notes.length ?? 0)}
									events={current.run.events}
								/>
							</details>
							<dl class="init footer">
								<dt>Steps</dt>
								<dd>{current.run.steps.toLocaleString()}</dd>
								<dt>Iterations</dt>
								<dd>{current.run.newtonIterations.toLocaleString()}</dd>
								<dt>Time</dt>
								<dd>
									{(current.wallMs / 1000).toFixed(2)} s
								</dd>
								<dt>Step</dt>
								<dd>{current.dt * 1000} ms</dd>
								<dt>Loads</dt>
								<dd>{(current.options.load_model ?? 'power_flow').replaceAll('_', ' ')}</dd>
							</dl>
							<button
								class="link"
								onclick={() => navigator.clipboard?.writeText(dyn.runJson(current))}
								>Copy run as JSON</button
							>
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
		color: var(--text-secondary);
	}
	.rows {
		margin: 0;
	}
	.rows div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 4px 0;
		border-bottom: 1px solid var(--line);
	}
	.rows dt {
		color: var(--text-secondary);
	}
	.rows dd {
		margin: 0;
	}
	.chip-warn {
		margin-left: 6px;
		padding: 1px 6px;
		border-radius: var(--radius-full);
		box-shadow: inset 3px 0 0 var(--amber);
		background: var(--surface-chip);
	}
	.link {
		justify-self: start;
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
