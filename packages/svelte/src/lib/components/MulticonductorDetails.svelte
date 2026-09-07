<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt, rgbaCss } from '../format.js';
	import {
		attachmentColor,
		attachmentGlyph,
		edgeColor,
		isPhaseTerminal,
		phaseColor
	} from '../multiconductor.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { getNoticeCenter } from '../notices.svelte.js';
	import { type DistAttachmentKind } from '@tellegen/engine';

	const app = getAppState();
	const ctrl = getController();
	const notices = getNoticeCenter();
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

	const NEUTRAL_RGBA = [120, 114, 102, 255] as const;
	const ATTACHMENT_LEGEND: DistAttachmentKind[] = ['source', 'generator', 'ibr', 'load', 'shunt'];

	function terminalColor(t: string): string {
		return rgbaCss(isPhaseTerminal(t) ? phaseColor(t) : [...NEUTRAL_RGBA]);
	}
</script>

{#if app.activeMulti}
	{@const mc = app.activeMulti}
	{@const s = mc.summary}
	<h2>{mc.label} <span class="region mono">via {mc.fileName}</span></h2>
	{#if s}
		<p class="tag mono">Multiconductor</p>
		<details class="case-details">
			<summary>{s.n_bus} buses, {s.n_edge} {s.n_edge === 1 ? 'branch' : 'branches'}</summary>
			<dl class="mono">
				<div>
					<dt>buses</dt>
					<dd>{s.n_bus}</dd>
				</div>
				<div>
					<dt>Lines / switches / transformers</dt>
					<dd>{s.n_line} / {s.n_switch} / {s.n_transformer}</dd>
				</div>
				<div>
					<dt>load</dt>
					<dd>{fmt.format(s.load_kw)} kW</dd>
				</div>
				<div>
					<dt>Generation capacity</dt>
					<dd>{fmt.format(s.gen_kw)} kW</dd>
				</div>
				<div>
					<dt>sources / loads / gens</dt>
					<dd>{s.n_source} / {s.n_load} / {s.n_generator}</dd>
				</div>
				{#if s.n_ibr > 0 || s.n_shunt > 0 || (s.n_capacitor ?? 0) > 0}
					<div>
						<dt>IBRs / shunts / caps</dt>
						<dd>{s.n_ibr} / {s.n_shunt} / {s.n_capacitor ?? 0}</dd>
					</div>
				{/if}
			</dl>
		</details>

		<p class="footnote mono">
			{mc.coordsKind === 'geographic'
				? 'Geographic coordinates'
				: mc.coordsKind === 'planar'
					? 'Drawing coordinates'
					: 'Generated diagram'}
		</p>
		<div class="calculation-actions">
			{#if mc.solving}
				<span role="status">Calculating...</span><button onclick={() => mc.solveAbort?.abort()}
					>Cancel</button
				>
			{:else}
				<button
					disabled={!mc.mcPfSupported}
					onclick={() => {
						void ctrl
							.solveMultiCase(mc)
							.then(openResults)
							.catch(() => {});
					}}>Solve AC power flow</button
				>
				{#if !mc.mcPfSupported}
					<button
						class="quiet"
						onclick={() =>
							notices.push({
								kind: 'warning',
								title: 'AC power flow unavailable',
								details: mc.mcPfReason ?? 'This case cannot run AC power flow in this browser'
							})}>Why unavailable</button
					>
				{/if}
			{/if}
		</div>
		{#if mc.result}<p class="footnote">Converged, {mc.result.iterations} iterations</p>
			<button class="reset mono" onclick={openResults}>View results in Studies</button>{/if}
	{/if}

	{#if mc.selectedBus}
		{@const b = mc.selectedBus}
		<hr />
		<h3 class="mono">bus {b.id}</h3>
		<div class="terminals">
			{#each b.terminals as t (t)}
				<span
					class="term"
					class:grounded={b.grounded.includes(t)}
					style={`--tc:${terminalColor(t)}`}
					title={b.grounded.includes(t) ? `terminal ${t} (grounded)` : `terminal ${t}`}
				>
					{t}{#if b.grounded.includes(t)}<span class="gnd" aria-hidden="true">&#9178;</span>{/if}
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
			<p class="footnote mono">no attachments on this bus</p>
		{/if}
	{:else if mc.selectedEdge}
		{@const e = mc.selectedEdge}
		<hr />
		<h3 class="mono">
			<i class="swatch" style={`--sc:${rgbaCss([...edgeColor(e.kind, e.closed)])}`}></i>
			{e.kind}
			{e.from}&#8201;&ndash;&#8201;{e.to}
		</h3>
		<dl class="mono">
			<div>
				<dt>phases</dt>
				<dd>{e.n_phases}&#966; , {e.conductors.length} conductors</dd>
			</div>
			{#if e.kind === 'switch'}
				<div>
					<dt>state</dt>
					<dd>{e.closed ? 'closed' : 'open'}</dd>
				</div>
			{/if}
		</dl>
		<div class="pairs">
			{#each e.conductors as [ft, tt], i (i)}
				<span class="pair">
					<span class="term" style={`--tc:${terminalColor(ft)}`}>{ft}</span>
					<span class="pair-dash mono">&ndash;</span>
					<span class="term" style={`--tc:${terminalColor(tt)}`}>{tt}</span>
				</span>
			{/each}
		</div>
	{:else if mc.placed}
		<p class="footnote mono">select a bus or a line to expand its detail</p>
	{/if}

	<div class="mc-legend mono">
		<span class="legend-row">
			<i class="swatch" style={`--sc:${rgbaCss([...phaseColor('1')])}`}></i>a
			<i class="swatch" style={`--sc:${rgbaCss([...phaseColor('2')])}`}></i>b
			<i class="swatch" style={`--sc:${rgbaCss([...phaseColor('3')])}`}></i>c
			<i class="swatch" style={`--sc:${rgbaCss([...NEUTRAL_RGBA])}`}></i>n
		</span>
		<span class="legend-row">
			{#each ATTACHMENT_LEGEND as kind (kind)}
				<i class="swatch" style={`--sc:${rgbaCss([...attachmentColor(kind)])}`}
				></i>{attachmentGlyph(kind)}
			{/each}
		</span>
	</div>

	<button class="reset mono" onclick={() => app.requestFrame(mc.id)}>Fit case</button>
	<button class="reset mono" onclick={() => ctrl.removeMultiCase(mc)}>remove</button>
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
	.calculation-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: center;
		margin: 14px 0;
	}
	.calculation-actions button {
		font: inherit;
		padding: 7px 10px;
		border: 1px solid var(--line);
		border-radius: 3px;
		background: var(--ink);
		color: var(--paper);
		cursor: pointer;
	}
	.calculation-actions button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.calculation-actions button.quiet {
		background: transparent;
		color: var(--ink);
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

	.tag {
		margin: 0 0 10px;
		font-size: 10px;
		color: var(--text-secondary);
		letter-spacing: 0;
		text-transform: uppercase;
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
		font-size: 11px;
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
		font-size: 10px;
		letter-spacing: 0;
	}

	/* The global stylesheet's .legend is the 6px color-ramp strip; its height
	 * leaks into a scoped .legend, so this container carries its own name. */
	.mc-legend {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 12px;
		font-size: 10px;
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
		font-size: 10px;
		color: var(--text-tertiary);
		letter-spacing: 0;
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}
</style>
