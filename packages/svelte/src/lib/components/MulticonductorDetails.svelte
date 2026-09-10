<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt, rgbaCss } from '../format.js';
	import {
		attachmentColor,
		attachmentGlyph,
		edgeColor,
		isPhaseTerminal,
		neutralTerminal,
		phaseColor
	} from '../multiconductor.js';
	import { formatPowerIoDiagnostic, type DistAttachmentKind } from '@tellegen/engine';

	const app = getAppState();
	const ctrl = getController();

	const NEUTRAL_RGBA = [120, 114, 102, 255] as const;
	const ATTACHMENT_LEGEND: DistAttachmentKind[] = ['source', 'generator', 'ibr', 'load', 'shunt'];

	function terminalColor(t: string): string {
		return rgbaCss(isPhaseTerminal(t) ? phaseColor(t) : [...NEUTRAL_RGBA]);
	}
	function magnitude(z: { re: number; im: number }): number {
		return Math.hypot(z.re, z.im);
	}
	function angle(z: { re: number; im: number }): number {
		return (Math.atan2(z.im, z.re) * 180) / Math.PI;
	}
	function angleText(z: { re: number; im: number }): string {
		return magnitude(z) === 0 ? '—' : `${fixed(angle(z), 2)}°`;
	}
	function relativeVoltage(
		voltage: { re: number; im: number },
		neutral: { re: number; im: number } | null
	): { re: number; im: number } {
		return neutral ? { re: voltage.re - neutral.re, im: voltage.im - neutral.im } : voltage;
	}
	function powerKw(z: { re: number; im: number }): number {
		return z.re / 1000;
	}
	function powerKvar(z: { re: number; im: number }): number {
		return z.im / 1000;
	}
	function fixed(value: number, digits = 3): string {
		return Number.isFinite(value) ? value.toFixed(digits) : '—';
	}
	function busNeutral(bus: {
		terminals: string[];
		neutral_terminal?: string | null;
	}): string | null {
		return neutralTerminal(bus.terminals, bus.neutral_terminal);
	}
	function terminalResults(mc: typeof app.activeMulti, bus: string) {
		return mc?.mcResult?.terminals.filter((t) => t.bus === bus) ?? [];
	}
	function edgeResults(mc: typeof app.activeMulti, id: string, kind: string) {
		const result = mc?.mcResult;
		if (!result) return [];
		return result.element_ports.filter((p) => p.element === id && p.kind === kind);
	}
	function sourceTotals(mc: typeof app.activeMulti) {
		return (mc?.mcResult?.source_reactions ?? []).reduce(
			(total, r) => ({
				re: total.re + r.power_into_network.re,
				im: total.im + r.power_into_network.im
			}),
			{ re: 0, im: 0 }
		);
	}
	function passiveLossKw(mc: typeof app.activeMulti) {
		return (mc?.mcResult?.element_ports ?? [])
			.filter((p) => p.kind !== 'load' && p.kind !== 'generator' && p.kind !== 'ibr')
			.reduce((sum, p) => sum + powerKw(p.power_into_element), 0);
	}
</script>

{#if app.activeMulti}
	{@const mc = app.activeMulti}
	{@const s = mc.summary}
	<h2>{mc.label} <span class="region mono">via {mc.fileName}</span></h2>
	{#if s}
		<p class="tag mono">multiconductor &#8901; distribution power flow</p>
		<dl class="mono">
			<div>
				<dt>buses</dt>
				<dd>{s.n_bus}</dd>
			</div>
			<div>
				<dt>lines / switches / xfmrs</dt>
				<dd>{s.n_line} / {s.n_switch} / {s.n_transformer}</dd>
			</div>
			<div>
				<dt>load</dt>
				<dd>{fmt.format(s.load_kw)} kW</dd>
			</div>
			<div>
				<dt>gen capacity</dt>
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

		{#if s.diagnostics.length > 0}
			<ul class="warnings mono">
				{#each s.diagnostics.slice(0, 4) as diagnostic, i (i)}
					<li>{formatPowerIoDiagnostic(diagnostic)}</li>
				{/each}
				{#if s.diagnostics.length > 4}
					<li>+{s.diagnostics.length - 4} more</li>
				{/if}
			</ul>
		{/if}

		<section class="mc-solve" aria-label="Distribution power flow">
			<h3 class="mono">Distribution power flow</h3>
			{#if mc.mcPfSupported && mc.moduleJson}
				<p class="footnote mono">typed PowerIO input retained for the fixed-point solver</p>
				<button
					class="primary mono"
					disabled={mc.mcSolving}
					onclick={() => void ctrl.runMultiSolve(mc)}
					>{mc.mcSolving ? 'running…' : mc.mcResult ? 'run again' : 'run power flow'}</button
				>
			{:else}
				<p class="footnote mono">
					{mc.mcPfReason ??
						'This input is available for inspection; no typed solver module was retained.'}
				</p>
			{/if}
			{#if mc.mcError}<p class="error mono" role="alert">{mc.mcError}</p>{/if}
			{#if mc.mcResult}
				{@const result = mc.mcResult}
				{@const source = sourceTotals(mc)}
				<dl class="mono result-facts">
					<div>
						<dt>status</dt>
						<dd>{result.converged ? 'converged' : 'not converged'}</dd>
					</div>
					<div>
						<dt>iterations</dt>
						<dd>{result.iterations}</dd>
					</div>
					<div>
						<dt>voltage band</dt>
						<dd>
							{result.voltage_valid
								? 'valid'
								: `${result.voltage_violations.length} violation${result.voltage_violations.length === 1 ? '' : 's'}`}
						</dd>
					</div>
					<div>
						<dt>load voltage range</dt>
						<dd>
							{result.min_voltage_pu == null || result.max_voltage_pu == null
								? 'n/a'
								: `${fixed(result.min_voltage_pu)}–${fixed(result.max_voltage_pu)} pu`}
						</dd>
					</div>
					<div>
						<dt>source P / Q</dt>
						<dd>
							{fixed(powerKw(source))} / {fixed(powerKvar(source))} kW / kvar
						</dd>
					</div>
					<div>
						<dt>network loss</dt>
						<dd>{fixed(passiveLossKw(mc))} kW</dd>
					</div>
					<div>
						<dt>KCL residual</dt>
						<dd>{result.physical_kcl_residual.toExponential(3)} A</dd>
					</div>
				</dl>
			{/if}
		</section>

		{#if !mc.placed}
			<p class="footnote mono">
				{s.coords_kind === 'planar'
					? 'coordinates are diagram-only: click the map to place the layout'
					: 'no coordinates in this file: click the map to place the layout'}
			</p>
		{:else if mc.coordsKind === 'geographic'}
			<p class="footnote mono">coordinates: geographic, from the case file</p>
		{:else if mc.coordsKind === 'planar'}
			<p class="footnote mono">coordinates: diagram layout fit where you placed it</p>
		{:else}
			<p class="footnote mono">
				coordinates: synthetic topology layout centered where you placed it
			</p>
		{/if}
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
		{#if mc.mcResult}
			{@const rows = terminalResults(mc, b.id)}
			{@const neutralName = busNeutral(b)}
			{@const neutral = neutralName
				? (rows.find((row) => row.terminal === neutralName) ?? null)
				: null}
			{@const hasNeutral = neutral !== null}
			{#if rows.length > 0}
				<div class="mc-table-scroll">
					<table class="mc-values mono">
						<thead
							><tr
								><th>terminal</th><th>|V| to earth</th><th>angle to earth</th>{#if hasNeutral}<th
										>|V| to neutral</th
									><th>angle to neutral</th>{/if}</tr
							></thead
						>
						<tbody
							>{#each rows as row (row.bus + row.terminal)}
								{@const isNeutral = hasNeutral && row.terminal === neutralName}
								{@const earthVoltage = row.voltage}
								{@const neutralVoltage = isNeutral
									? { re: 0, im: 0 }
									: relativeVoltage(row.voltage, hasNeutral ? neutral!.voltage : null)}
								<tr
									><td>{row.terminal}</td><td>{fixed(magnitude(earthVoltage))} V</td><td
										>{angleText(earthVoltage)}</td
									>{#if hasNeutral}<td>{fixed(magnitude(neutralVoltage))} V</td><td
											>{angleText(neutralVoltage)}</td
										>{/if}</tr
								>
							{/each}</tbody
						>
					</table>
				</div>
			{/if}
		{/if}
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
				<dd>{e.n_phases}&#966; &#8901; {e.conductors.length} conductors</dd>
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
		{#if mc.mcResult}
			{@const ports = edgeResults(mc, e.id, e.kind)}
			{#if ports.length > 0}
				<div class="mc-table-scroll">
					<table class="mc-values mono">
						<thead><tr><th>port</th><th>I</th><th>P / Q</th></tr></thead>
						<tbody
							>{#each ports as port (JSON.stringify( [port.kind, port.element, port.branch, port.bus, port.terminal] ))}
								<tr>
									<td>{port.bus} · {port.terminal}</td>
									<td>{fixed(magnitude(port.current_into_element))} A</td>
									<td
										>{fixed(powerKw(port.power_into_element))} / {fixed(
											powerKvar(port.power_into_element)
										)} kW / kvar</td
									>
								</tr>
							{/each}</tbody
						>
					</table>
				</div>
			{/if}
		{/if}
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

	{#if !mc.placed}
		<button class="reset mono" onclick={() => ctrl.moveMultiCase(mc)}>place on map</button>
	{:else}
		<button class="reset mono" onclick={() => ctrl.moveMultiCase(mc)}>move layout</button>
	{/if}
	<button class="reset mono" onclick={() => ctrl.removeMultiCase(mc)}>remove</button>
{/if}

<style>
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
	.mc-table-scroll {
		max-width: 100%;
		overflow-x: auto;
	}
	.mc-values {
		min-width: 330px;
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

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--text-accent);
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}
</style>
