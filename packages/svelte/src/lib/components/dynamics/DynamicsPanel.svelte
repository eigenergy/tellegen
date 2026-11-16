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
	import DynamicsBode from './DynamicsBode.svelte';
	import DynamicsLive from './DynamicsLive.svelte';
	import DynamicsModels from './DynamicsModels.svelte';
	import { headline } from '../../dynamics/verdict.js';
	import { onTabKey } from '../../dynamics/tabs.js';
	import ModeSwitch from './ModeSwitch.svelte';
	import StatBoxes from '../StatBoxes.svelte';
	import { splitName } from '../../format.js';

	const app = getAppState();
	const ctrl = getController();
	const layout = getPanelLayout();
	const dyn = ctrl.dynamics;

	let open = $state(false);
	type Tab = 'run' | 'live' | 'models' | 'modes';
	const TABS: [Tab, string][] = [
		['run', 'Simulation'],
		['live', 'Live'],
		['models', 'Models'],
		['modes', 'Modes']
	];
	let tab = $state<Tab>('run');
	let demoBusy = $state(false);
	let demoError = $state<string | null>(null);
	const att = $derived(dyn.active);
	const ins = $derived(att?.inspection ?? null);
	const experimental = $derived(app.preferences.experimental && !!dyn.support?.experimental);
	const local = $derived(app.activeLocal);
	const current = $derived(dyn.current);
	/** The engine's notes on the dynamic data and the import's notes on the case, as the Log
	 * lists them. */
	const noteCount = $derived(
		(ins?.notes.length ?? 0) +
			(local?.summary
				? local.summary.diagnostics.length +
					('warnings' in local.summary ? local.summary.warnings.length : 0)
				: 0)
	);

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
	<div class="dyn settings-list" data-testid="dynamics-panel">
		<!-- The switch appears once there is a simulation to show. -->
		{#if dyn.runs.length > 0}<div class="setting">
				<span>Map shows</span><ModeSwitch />
			</div>{/if}
		{#if !att}
			<section class="block" aria-label="Case">
				<div class="setting">
					<span>Case</span><span class="value">{caseName ?? 'None'}</span>
				</div>
				<div class="setting">
					<span>Dynamic data</span><span class="value">None</span>
				</div>
				{#if experimental && local?.studyInputJson}
					<div class="setting">
						<label for="dyn-preset">Generator type</label>
						<select id="dyn-preset" bind:value={preset}>
							<option value="steam_wscc_192">Steam</option>
							<option value="hydro_wscc_247">Hydro</option>
							<option value="gas_turbine">Gas turbine</option>
							<option value="steam_kundur_555">Steam, large</option>
						</select>
					</div>
				{/if}
				<!-- The run controls, as they read once the case has dynamic data. -->
				<div class="setting">
					<span>Event</span><span class="value">None</span>
				</div>
				<div class="actions">
					{#if caseName}<button class="secondary" onclick={openDyr}>Open .dyr file…</button>{/if}
					{#if experimental && local?.studyInputJson}<button
							class="secondary"
							disabled={dynamizeBusy}
							onclick={dynamize}>{dynamizeBusy ? 'Adding…' : 'Add dynamic data'}</button
						>{/if}
					<button class="secondary" disabled={demoBusy} onclick={demo} data-testid="dyn-open-demo"
						>{demoBusy ? 'Opening…' : 'Open WECC 240 demo'}</button
					>
				</div>
				{#if demoError}<p class="err" role="alert" title={demoError}>Demo failed to open</p>{/if}
				{#if dynamizeError}<p class="err" title={dynamizeError}>
						Dynamic data failed to build
					</p>{/if}
				<button class="run" disabled title="Needs dynamic data">Run simulation</button>
			</section>
		{:else}
			<section class="block" aria-label="Dynamic data">
				<div class="setting">
					<span>Case</span><span class="value">{caseName ?? 'None'}</span>
				</div>
				<div class="setting">
					<span>Dynamic data</span>
					<span class="value"
						>{att.fileName}{#if att.synthetic}<span class="tag">Synthetic</span>{/if}</span
					>
				</div>
				{#if att.status === 'opening'}
					<p class="dim" role="status">Opening…</p>
				{:else if att.status === 'failed'}
					<p class="err" role="alert" title={att.error ?? undefined}>Dynamic data failed to open</p>
				{:else if ins}
					<div class="counts" data-testid="dyn-counts">
						<StatBoxes stats={counts} label="Dynamic model" />
					</div>
					{#if att.preparing}
						<p class="dim" role="status" data-testid="dyn-preparing">Preparing…</p>
					{:else if att.prepareError}
						<p class="err" role="alert" title={att.prepareError}>Preparation failed</p>
					{/if}
					{#if att.flat && !flatOk}
						<p class="warn-line" role="status">Not steady before the event</p>
					{:else if att.flatError}
						<p class="err" title={att.flatError}>Rest check failed</p>
					{/if}
					{#if dyn.staticEdits > 0}
						<p class="warn-line">Edits ignored: {dyn.staticEdits}</p>
					{/if}
					<div class="setting" aria-busy={att.preparing}>
						<span>Starts settled</span>
						<span class="value" data-testid="dyn-flat"
							>{#if att.flat}{flatOk
									? 'Yes'
									: 'No'}{:else if att.flatError}—{:else}Checking…{/if}</span
						>
					</div>
					{#if noteCount}
						<div class="setting">
							<span>Notes</span>
							<span class="value"
								>{noteCount.toLocaleString('en-US')}
								<button class="link" onclick={() => layout.show('log')}>Show in Log</button></span
							>
						</div>
					{/if}
				{/if}
			</section>

			{#if experimental}
				<div
					class="tabs"
					role="tablist"
					aria-label="Dynamics views"
					tabindex="-1"
					onkeydown={(e) =>
						onTabKey(
							e,
							TABS.map(([t]) => t),
							tab,
							(t) => (tab = t)
						)}
				>
					{#each TABS as [t, label] (t)}
						<button
							role="tab"
							id={`dyn-tab-${t}`}
							aria-selected={tab === t}
							aria-controls="dyn-tabpanel"
							tabindex={tab === t ? 0 : -1}
							onclick={() => (tab = t)}
							>{label}</button
						>
					{/each}
				</div>
			{/if}

			<div
				class="tabpanel"
				id="dyn-tabpanel"
				role={experimental ? 'tabpanel' : undefined}
				aria-labelledby={experimental ? `dyn-tab-${tab}` : undefined}
			>
				{#if tab === 'live' && experimental}
					<DynamicsLive />
				{:else if tab === 'models' && experimental}
					<DynamicsModels />
				{:else if tab === 'modes' && experimental}
					<section class="block" aria-label="Small-signal modes" data-testid="dyn-modes">
						{#if dyn.modes}
							<EigenPlot result={dyn.modes} bind:selected={dyn.selectedMode} />
						{:else}
							{#if dyn.dynamicStates > 300}
								<p class="warn-line" data-testid="dyn-modes-size">May take a few minutes</p>
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
					<DynamicsBode />
				{:else if att.status === 'ready'}
					<DynamicsEventList />
					<DynamicsRunControls />
					{#if current}
						<DynamicsVerdict record={current} />
					{/if}
					{#if dyn.runs.length > 1}
						<div class="history" aria-label="Recent runs">
							{#each dyn.runs as r (r.id)}
								{@const h = headline(r.run?.screening ?? null, r.failure, r.tEnd)}
								<button aria-pressed={r.id === dyn.currentRunId} onclick={() => dyn.select(r.id)}>
									<span aria-hidden="true">{h.icon}</span>
									{r.events.length ? dyn.describe(r.events[0]) : 'No events'}{r.events.length > 1
										? ` and ${r.events.length - 1} more`
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
	.dyn {
		display: grid;
		gap: 12px;
		padding: 12px 16px 16px;
		font-size: var(--fs-body);
	}
	p {
		margin: 0;
	}
	.block {
		display: grid;
		gap: 8px;
	}
	.counts :global(dl.stats) {
		margin: 4px 0 0;
	}
	/* The case actions share the Run button's width, two to a row; one left alone takes the row. */
	.actions {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 8px;
	}
	.actions > :last-child:nth-child(odd) {
		grid-column: 1 / -1;
	}
	.actions .secondary {
		justify-self: stretch;
		min-height: 32px;
	}
	.run {
		min-height: 36px;
		border: 1px solid var(--accent-fill);
		border-radius: var(--radius-sm);
		background: var(--accent-fill);
		color: var(--on-accent);
		font: inherit;
		font-weight: 600;
	}
	/* Disabled is a flat grey fill with no border, unlike any enabled button. */
	.run:disabled {
		border-color: transparent;
		background: var(--surface-hover);
		color: var(--text-tertiary);
		cursor: default;
	}
	.secondary {
		justify-self: start;
		min-height: 28px;
		padding: 4px 10px;
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
		color: var(--text-tertiary);
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
	.link {
		margin-left: 8px;
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
		font-size: var(--fs-label);
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
