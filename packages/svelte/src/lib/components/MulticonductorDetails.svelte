<script lang="ts">
	import McNetworkScope from './McNetworkScope.svelte';
	import McModelQualification from './McModelQualification.svelte';
	import McSolveControls from './McSolveControls.svelte';
	import { getAppState, getController } from '../context.svelte.js';
	import { quantity, rgbaCss } from '../format.js';
	import {
		attachmentColor,
		attachmentGlyph,
		edgeColor,
		EXCLUDED_ISLAND_BUS,
		neutralTerminal,
		phaseCss,
		phaseLabel,
		wiresLabel
	} from '../multiconductor.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { uiTheme } from './ui-theme.js';
	import type { DistAttachmentKind } from '@tellegen/engine';

	const app = getAppState();
	const ctrl = getController();
	const panels = getPanelLayout();
	function openResults() {
		const panel = panels.panels.find((p) => p.id === 'studies');
		if (panel) {
			const network = panels.panels.find((p) => p.id === 'network');
			if (network) panels.close(network);
			panel.setOpen(true);
			panels.activate(panel);
			if (panels.compact) panels.drawer = panel.id;
		}
	}

	/** A cleared or unparsable number field is not a request for zero load;
	 * only a finite value counts as an edit. */
	function parseLoadInput(raw: string): number | null {
		if (raw.trim() === '') return null;
		const value = Number(raw);
		return Number.isFinite(value) ? value : null;
	}
	const ATTACHMENT_LEGEND: DistAttachmentKind[] = ['source', 'generator', 'ibr', 'load', 'shunt'];

	const theme = uiTheme();
	const terminalColor = (t: string) => phaseCss(t, theme.current);
</script>

{#if app.activeMulti}
	{@const mc = app.activeMulti}
	{@const s = mc.summary}
	<h2>{mc.label}</h2>
	{#if mc.distribution}
		<p class="footnote">
			{mc.distribution.description ?? ''}
			{#if mc.distribution.source_url}<a
					href={mc.distribution.source_url}
					target="_blank"
					rel="noreferrer">Source data</a
				>{/if}
		</p>
	{/if}
	{#if s}
		<details class="case-details">
			<summary
				>{s.n_bus.toLocaleString('en-US')} buses, {s.n_edge.toLocaleString('en-US')}
				{s.n_edge === 1 ? 'branch' : 'branches'}</summary
			>
			<dl>
				<div>
					<dt>Buses</dt>
					<dd class="mono">{s.n_bus}</dd>
				</div>
				<div>
					<dt>Lines / switches / transformers</dt>
					<dd class="mono">{s.n_line} / {s.n_switch} / {s.n_transformer}</dd>
				</div>
				<div>
					<dt>Load</dt>
					<dd class="mono">{quantity(s.load_kw, 'kW')}</dd>
				</div>
				<div>
					<dt>Generation capacity</dt>
					<dd class="mono">{quantity(s.gen_kw, 'kW')}</dd>
				</div>
				<div>
					<dt>Sources / loads / generators</dt>
					<dd class="mono">{s.n_source} / {s.n_load} / {s.n_generator}</dd>
				</div>
				{#if s.n_ibr > 0 || s.n_shunt > 0 || (s.n_capacitor ?? 0) > 0}
					<div>
						<dt>IBRs / shunts / capacitors</dt>
						<dd class="mono">{s.n_ibr} / {s.n_shunt} / {s.n_capacitor ?? 0}</dd>
					</div>
				{/if}
				<div>
					<dt>File</dt>
					<dd class="mono">{mc.fileName}</dd>
				</div>
			</dl>
		</details>

		<dl>
			<div>
				<dt>Coordinates</dt>
				<dd>
					{mc.coordsKind === 'geographic'
						? 'Geographic'
						: mc.coordsKind === 'planar'
							? 'Drawing'
							: 'Generated diagram'}
				</dd>
			</div>
		</dl>
		<McNetworkScope
			scope={mc.mcPreparedScope}
			islands={mc.mcExcludedIslands}
			preview={!mc.result}
		/>
		<McSolveControls
			{mc}
			onsettings={() => (app.settingsOpen = true)}
			onsolve={() => {
				void ctrl
					.solveMultiCase(mc, mc.mcOptions, undefined, mc.mcRequiresEnergizedSolve)
					.then(openResults)
					.catch(() => {});
			}}
		/>
		{#if mc.result}<McModelQualification model={mc.result.load_model} compact />
			<p class="footnote">Converged, {mc.result.iterations} iterations</p>
			<button class="reset" onclick={openResults}>View results in Studies</button>{/if}
	{/if}

	{#if mc.selectedBus}
		{@const b = mc.selectedBus}
		{#if mc.mcExcludedIslands.some((island) => island.buses.includes(b.id))}<p role="status">
				{EXCLUDED_ISLAND_BUS}
			</p>{/if}
		<hr />
		<h3>Bus <span class="mono">{b.id}</span></h3>
		{@const neutral = neutralTerminal(b.terminals, b.neutral_terminal)}
		<div class="terminals">
			{#each b.terminals as t (t)}
				{@const label = phaseLabel(t, neutral)}
				<span
					class="term"
					class:grounded={b.grounded.includes(t)}
					style={`--tc:${terminalColor(t)}`}
					title={`Terminal ${label}${label === t ? '' : ` (${t} in the file)`}${b.grounded.includes(t) ? ', grounded' : ''}`}
				>
					{label}{#if b.grounded.includes(t)}<span class="gnd" aria-hidden="true">&#9178;</span
						>{/if}
				</span>
			{/each}
		</div>
		{#if b.attachmentKinds.length > 0}
			<div class="badges">
				{#each b.attachmentKinds as kind (kind)}
					<span class="badge" style={`--bc:${rgbaCss([...attachmentColor(kind)])}`}>
						{attachmentGlyph(kind)}
					</span>
				{/each}
			</div>
		{:else}
			<p class="footnote">No attachments</p>
		{/if}
		{@const loadBranches = mc.mcLoadBranches.filter((load) => load.bus === b.id)}
		{#if loadBranches.length > 0}
			<div class="load-editor">
				<h4>Load power</h4>
				{#each loadBranches as load (`${load.load}:${load.branch}`)}
					<div class="load-row">
						<span class="mono"
							>{load.load}{loadBranches.length > 1 ? ` (${load.branch + 1})` : ''}</span
						>
						<label
							>P kW<input
								type="number"
								step="any"
								value={load.p_w / 1000}
								onchange={(event) => {
									const kw = parseLoadInput(event.currentTarget.value);
									if (kw === null) {
										event.currentTarget.value = String(load.p_w / 1000);
										return;
									}
									ctrl.queueMultiLoadPower(mc, load.load, load.branch, kw * 1000, load.q_var);
								}}
							/></label
						>
						<label
							>Q kvar<input
								type="number"
								step="any"
								value={load.q_var / 1000}
								onchange={(event) => {
									const kvar = parseLoadInput(event.currentTarget.value);
									if (kvar === null) {
										event.currentTarget.value = String(load.q_var / 1000);
										return;
									}
									ctrl.queueMultiLoadPower(mc, load.load, load.branch, load.p_w, kvar * 1000);
								}}
							/></label
						>
						<button
							class="quiet"
							disabled={load.p_w === load.base_p_w && load.q_var === load.base_q_var}
							onclick={() =>
								ctrl.queueMultiLoadPower(
									mc,
									load.load,
									load.branch,
									load.base_p_w,
									load.base_q_var
								)}>Reset</button
						>
					</div>
				{/each}
				<p class="footnote">Changes solve automatically.</p>
			</div>
		{/if}
	{:else if mc.selectedEdge}
		{@const e = mc.selectedEdge}
		<hr />
		<h3 class="mono">
			<i class="swatch" style={`--sc:${rgbaCss([...edgeColor(e.kind, e.closed)])}`}></i>
			{e.kind}
			{e.from}&#8201;&ndash;&#8201;{e.to}
		</h3>
		<dl>
			<div>
				<dt>Conductors</dt>
				<dd class="mono">{wiresLabel(e.n_phases, e.conductors.length > e.n_phases)}</dd>
			</div>
			{#if e.kind === 'switch'}
				<div>
					<dt>State</dt>
					<dd>{e.closed ? 'Closed' : 'Open'}</dd>
				</div>
			{/if}
		</dl>
		<div class="pairs">
			{#each e.conductors as [ft, tt], i (i)}
				<span class="pair">
					<span class="term" style={`--tc:${terminalColor(ft)}`}>{phaseLabel(ft)}</span>
					<span class="pair-dash mono">&ndash;</span>
					<span class="term" style={`--tc:${terminalColor(tt)}`}>{phaseLabel(tt)}</span>
				</span>
			{/each}
		</div>
	{/if}

	<div class="mc-legend mono">
		<span class="legend-row">
			{#each ['A', 'B', 'C', 'N'] as phase (phase)}<i
					class="swatch"
					style={`--sc:${terminalColor(phase.toLowerCase())}`}
				></i>{phase}
			{/each}
		</span>
		<span class="legend-row">
			{#each ATTACHMENT_LEGEND as kind (kind)}
				<i class="swatch" style={`--sc:${rgbaCss([...attachmentColor(kind)])}`}
				></i>{attachmentGlyph(kind)}
			{/each}
		</span>
	</div>

	{#if mc.graph}<button
			class="reset"
			aria-pressed={mc.schematic}
			title="Symbols and the engine's schematic; results once solved"
			onclick={() => (mc.schematic = !mc.schematic)}
			>{mc.schematic ? 'Show map' : 'Show diagram'}</button
		>{/if}
	<button class="reset" onclick={() => app.requestFrame(mc.id)}>Fit case</button>
	<button class="reset" onclick={() => ctrl.removeMultiCase(mc)}>Remove</button>
{/if}

<style>
	.case-details {
		margin: 10px 0;
		font-size: 12px;
	}
	.case-details summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	.case-details dl {
		margin-top: 8px;
	}
	h2 {
		margin: 0 0 8px;
		font-size: 16px;
		font-weight: 600;
	}

	h3 {
		margin: 0 0 8px;
		font-size: 12px;
		font-weight: 600;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
		gap: 12px;
	}

	dt {
		color: var(--text-secondary);
	}

	dd {
		margin: 0;
		text-align: right;
		/* Wrap the label instead of splitting a value sequence like 126 / 0 / 9. */
		white-space: nowrap;
	}

	.terminals {
		display: flex;
		flex-wrap: wrap;
		gap: 5px;
	}

	.term {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		min-width: 18px;
		height: 20px;
		padding: 0 6px;
		border-radius: 3px;
		border: 1px solid color-mix(in srgb, var(--tc) 70%, #20242b);
		background: color-mix(in srgb, var(--tc) 26%, transparent);
		color: var(--ink);
		font-family: var(--font-mono);
		font-size: 13px;
	}

	.term.grounded {
		border-style: dashed;
	}

	.gnd {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.badges {
		display: flex;
		flex-wrap: wrap;
		gap: 5px;
		margin-top: 8px;
	}

	.badge {
		padding: 1px 6px;
		border-radius: 3px;
		background: var(--bc);
		color: #fff;
		font-size: 12px;
		letter-spacing: 0;
	}

	.load-editor {
		margin-top: 14px;
		padding-top: 10px;
		border-top: 1px solid var(--line);
	}

	.load-editor h4 {
		margin: 0 0 8px;
		font-size: 13px;
	}

	.load-row {
		display: grid;
		grid-template-columns: minmax(70px, 1fr) 72px 72px auto;
		gap: 6px;
		align-items: end;
		margin-bottom: 7px;
	}

	.load-row label {
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.load-row input {
		min-width: 0;
		width: 100%;
		box-sizing: border-box;
		font: 12px var(--font-mono);
	}

	.load-row button {
		font-size: 12px;
	}

	/* The global stylesheet's .legend is the 6px color-ramp strip; its height
	 * leaks into a scoped .legend, so this container carries its own name. */
	.mc-legend {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 12px;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.pairs {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 8px;
	}

	.pair {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.pair-dash {
		color: var(--text-secondary);
	}

	h3 .swatch {
		margin-right: 4px;
		vertical-align: -1px;
	}

	.legend-row {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-wrap: wrap;
	}

	.swatch {
		display: inline-block;
		width: 10px;
		height: 10px;
		border-radius: 2px;
		background: var(--sc);
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 12px;
		color: var(--text-tertiary);
		letter-spacing: 0;
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}
</style>
