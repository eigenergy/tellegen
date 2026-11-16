<script lang="ts">
	import { rmsDynamize } from '@tellegen/engine';
	import { untrack } from 'svelte';
	import { getAppState, getController } from '../../context.svelte.js';
	import { getPanelLayout } from '../../panels.svelte.js';
	import PanelFrame from '../PanelFrame.svelte';
	import DynamicsEventList from './DynamicsEventList.svelte';
	import DynamicsRunControls from './DynamicsRunControls.svelte';
	import DynamicsVerdict from './DynamicsVerdict.svelte';
	import EigenPlot from './EigenPlot.svelte';
	import { headline } from '../../dynamics/verdict.js';
	import { groupNotes } from '../../dynamics/notes.js';
	import { onTabKey } from '../../dynamics/tabs.js';
	import ModeSwitch from './ModeSwitch.svelte';
	import StatBoxes from '../StatBoxes.svelte';
	import { splitName } from '../../format.js';

	const app = getAppState();
	const ctrl = getController();
	const layout = getPanelLayout();
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

	// The panel opens with the mode, and closing it returns the map to steady state.
	$effect(() => {
		if (app.mode === 'dynamics') open = true;
	});
	$effect(() => {
		if (!open)
			untrack(() => {
				if (app.mode === 'dynamics') app.mode = 'steady';
			});
	});
	$effect(() => {
		void dyn.probe();
	});

	// The engine's notes go to the Log, where they can be read in full; the panel counts them.
	$effect(() => {
		const notes = ins?.notes;
		const subject = att?.fileName;
		if (!notes?.length) return;
		untrack(() =>
			app.log.record(
				notes.map((message) => ({
					severity: 'note' as const,
					stage: 'Dynamics',
					source: 'RMS',
					subject,
					message
				}))
			)
		);
	});
	$effect(() => {
		const run = current?.run;
		if (!run) return;
		const known = untrack(() => ins?.notes.length ?? 0);
		const messages = [...run.notes.slice(known), ...run.events];
		if (!messages.length) return;
		untrack(() =>
			app.log.record(
				messages.map((message) => ({
					severity: 'note' as const,
					stage: 'Dynamics',
					source: 'RMS',
					subject: att?.fileName,
					message
				}))
			)
		);
	});
	const noteGroups = $derived(groupNotes(ins?.notes ?? []));

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

	const caseName = $derived(local?.label ?? (app.active ? splitName(app.active.name)[0] : null));
	/** Pick a PSS/E .dyr file for the open case. */
	function openDyr() {
		const input = document.createElement('input');
		input.type = 'file';
		input.accept = '.dyr';
		input.onchange = () => {
			if (input.files) void ctrl.ingestFiles(input.files);
		};
		input.click();
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
					{ label: 'Renewables', value: (ins.plants?.length ?? 0).toLocaleString() }
				]
			: []
	);
	const flatOk = $derived(!!att?.flat && att.flat.max_speed_drift <= 1e-6);
	/** A small number in plain decimals, to two significant figures. */
	const small = (value: number) =>
		value === 0 ? '0' : value.toLocaleString('en-US', { maximumSignificantDigits: 2 });
</script>

<PanelFrame
	id="dynamics"
	title="Dynamics"
	side="right"
	order={45}
	tab
	width={380}
	bind:open
	onopen={() => (app.mode = 'dynamics')}
>
	<div class="dyn" data-testid="dynamics-panel">
		<!-- The switch appears once there is a simulation to show. -->
		{#if dyn.runs.length > 0}<div class="dyn-mode">
				<span>Map shows</span><ModeSwitch />
			</div>{/if}
		{#if !att}
			<section class="empty">
				<dl class="rows">
					<div>
						<dt>Case</dt>
						<dd>{caseName ?? 'None'}</dd>
					</div>
					<div>
						<dt>Dynamic data</dt>
						<dd>None</dd>
					</div>
				</dl>
				<!-- The controls a run takes, disabled until the case has dynamic data. -->
				<div class="row">
					<label for="dyn-event-empty">Event</label>
					<select id="dyn-event-empty" disabled><option>Bus fault</option></select>
				</div>
				<button class="run" disabled title="Needs dynamic data">Run</button>
				<div class="actions">
					{#if caseName}<button class="secondary" onclick={openDyr}>Open .dyr file…</button>{/if}
					<button class="secondary" disabled={demoBusy} onclick={demo} data-testid="dyn-open-demo"
						>{demoBusy ? 'Opening…' : 'Open WECC 240 demo'}</button
					>
				</div>
				{#if demoError}<p class="err" role="alert" title={demoError}>Demo failed to open</p>{/if}
				{#if experimental && local?.studyInputJson}
					<div class="row">
						<label for="dyn-preset">Default dynamics</label>
						<span class="inline">
							<select id="dyn-preset" bind:value={preset}>
								<option value="steam_wscc_192">Steam</option>
								<option value="hydro_wscc_247">Hydro</option>
								<option value="gas_turbine">Gas turbine</option>
								<option value="steam_kundur_555">Steam, large</option>
							</select>
							<button class="secondary" disabled={dynamizeBusy} onclick={dynamize}
								>{dynamizeBusy ? 'Adding…' : 'Add'}</button
							>
						</span>
					</div>
					{#if dynamizeError}<p class="err" title={dynamizeError}>Default dynamics failed</p>{/if}
				{/if}
			</section>
		{:else}
			<section class="data" aria-label="Dynamic data">
				<dl class="rows">
					<div>
						<dt>Dynamic data</dt>
						<dd>
							{att.fileName}{#if att.synthetic}<span class="tag">Synthetic</span>{/if}
						</dd>
					</div>
				</dl>
				{#if att.status === 'opening'}
					<p class="dim" role="status">Initializing…</p>
				{:else if att.status === 'failed'}
					<p class="err" role="alert" title={att.error ?? undefined}>Dynamic data failed to open</p>
				{:else if ins}
					<div data-testid="dyn-counts"><StatBoxes stats={counts} label="Dynamic model" /></div>
					{#if att.preparing}
						<p class="dim" role="status" data-testid="dyn-preparing">Initializing…</p>
					{:else if att.prepareError}
						<p class="err" role="alert" title={att.prepareError}>Initialization failed</p>
					{/if}
					{#if att.flat && !flatOk}
						<p class="warn-line" role="status">Not at rest before the event</p>
					{:else if att.flatError}
						<p class="err" title={att.flatError}>Rest check failed</p>
					{/if}
					{#if dyn.staticEdits > 0}
						<p class="warn-line">Edits ignored: {dyn.staticEdits}</p>
					{/if}
					<details class="more">
						<summary>Details</summary>
						<dl class="rows" aria-busy={att.preparing}>
							{#if att.network === 'validation'}
								<div>
									<dt>Network</dt>
									<dd data-testid="dyn-network">Validation loads</dd>
								</div>
							{/if}
							<div>
								<dt>Held units</dt>
								<dd>{(ins.held_units?.length ?? 0).toLocaleString()}</dd>
							</div>
							<div>
								<dt>Records</dt>
								<dd>{ins.dyr_records.toLocaleString()}</dd>
							</div>
							<div>
								<dt>Reactive limits</dt>
								<dd>{ins.options?.reactive_limits === false ? 'Off' : 'On'}</dd>
							</div>
							<div>
								<dt>Angle reference</dt>
								<dd>{ins.reference_unit ? `Unit ${ins.reference_unit}` : 'System'}</dd>
							</div>
							<div>
								<dt>Initial mismatch</dt>
								<dd>{small(ins.initial_residual ?? 0)}</dd>
							</div>
							<div>
								<dt>At rest</dt>
								<dd data-testid="dyn-flat">
									{#if att.flat}{flatOk
											? 'Yes'
											: 'No'}{:else if att.flatError}—{:else}Checking…{/if}
								</dd>
							</div>
							{#each noteGroups as g (g.id)}
								<div>
									<dt>{g.title}</dt>
									<dd>{g.notes.length}</dd>
								</div>
							{/each}
						</dl>
						{#if ins.notes.length}<button class="link" onclick={() => layout.show('log')}
								>Notes in Log</button
							>{/if}
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
						onclick={() => (tab = 'modes')}>Modes</button
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
					<section class="modes" aria-label="Small-signal modes" data-testid="dyn-modes">
						{#if dyn.modes}
							<EigenPlot result={dyn.modes} bind:selected={dyn.selectedMode} />
						{:else}
							{#if dyn.dynamicStates > 300}
								<p class="dim" data-testid="dyn-modes-size">
									Slow: {dyn.dynamicStates.toLocaleString()} states
								</p>
							{/if}
							{#if dyn.modesBusy}
								<p class="status-row" role="status">
									Computing modes… <button class="secondary" onclick={() => dyn.stopModes()}
										>Stop</button
									>
								</p>
							{:else}
								<button
									class="secondary"
									disabled={att.status !== 'ready' || dyn.running}
									title={dyn.running ? 'Run in progress' : undefined}
									onclick={() => dyn.computeModes()}>Compute modes</button
								>
							{/if}
						{/if}
						{#if dyn.modesError}<p class="err" title={dyn.modesError}>
								{dyn.modesError === 'Stopped' ? 'Stopped' : 'Modes failed'}
							</p>{/if}
					</section>
				{:else if att.status === 'ready'}
					<DynamicsEventList />
					<DynamicsRunControls />
					{#if current}
						<DynamicsVerdict record={current} />
						{#if current.run}
							<details class="more">
								<summary>Run details</summary>
								<dl class="rows">
									<div>
										<dt>Steps</dt>
										<dd>{current.run.steps.toLocaleString()}</dd>
									</div>
									<div>
										<dt>Iterations</dt>
										<dd>{current.run.newtonIterations.toLocaleString()}</dd>
									</div>
									<div>
										<dt>Time</dt>
										<dd>{(current.wallMs / 1000).toFixed(2)} s</dd>
									</div>
									<div>
										<dt>Step</dt>
										<dd>{current.dt * 1000} ms</dd>
									</div>
									<div>
										<dt>Loads</dt>
										<dd>
											{(current.options.load_model ?? 'power_flow').replaceAll('_', ' ')}
										</dd>
									</div>
								</dl>
								<button
									class="link"
									onclick={() => navigator.clipboard?.writeText(dyn.runJson(current))}
									>Copy run as JSON</button
								>
							</details>
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
									{#if r.agent}<span class="agent">Agent</span>{/if}
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
	p {
		margin: 0;
		line-height: 1.45;
	}
	.empty,
	.data,
	.modes {
		display: grid;
		gap: 10px;
	}
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}
	.row select {
		width: 150px;
	}
	.inline {
		display: inline-flex;
		gap: 6px;
	}
	.inline select {
		width: 120px;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.run {
		justify-self: start;
		min-height: 28px;
		padding: 5px 14px;
		border: 1px solid var(--accent);
		border-radius: var(--radius-control);
		background: var(--accent);
		color: var(--on-accent);
		font: inherit;
		font-weight: 600;
	}
	.run:disabled {
		opacity: 0.45;
		cursor: default;
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
	.secondary:disabled {
		opacity: 0.45;
		cursor: default;
	}
	@media (pointer: coarse) {
		.secondary,
		.run {
			min-height: 44px;
		}
	}
	.dim {
		color: var(--text-secondary);
	}
	.err {
		color: var(--text-danger);
	}
	.warn-line {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--ink);
	}
	.warn-line::before {
		content: '';
		flex: none;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--amber);
	}
	.status-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.tag {
		margin-left: 6px;
		padding: 0 6px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		color: var(--text-secondary);
	}
	.tabs {
		display: flex;
		gap: 4px;
		border-bottom: 1px solid var(--line);
	}
	.tabs button {
		position: relative;
		border: none;
		background: none;
		padding: 6px 8px;
		font: inherit;
		color: var(--text-secondary);
		cursor: pointer;
	}
	.tabs button[aria-selected='true'] {
		color: var(--ink);
	}
	.tabs button[aria-selected='true']::after {
		content: '';
		position: absolute;
		left: 8px;
		right: 8px;
		bottom: -1px;
		height: 2px;
		background: var(--accent);
	}
	.rows {
		margin: 0;
		font-variant-numeric: tabular-nums;
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
		min-width: 0;
		overflow-wrap: anywhere;
		text-align: right;
	}
	.more {
		display: grid;
		gap: 6px;
	}
	.more summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	.more .link {
		margin-top: 6px;
	}
	.link {
		justify-self: start;
		border: none;
		background: none;
		padding: 0;
		color: var(--text-accent);
		cursor: pointer;
		font: inherit;
	}
	.link:hover {
		text-decoration: underline;
	}
	.history {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.history button {
		padding: 2px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
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
</style>
