<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt, rgbaCss } from '../format.js';
	import {
		attachmentColor,
		attachmentGlyph,
		isPhaseTerminal,
		phaseColor
	} from '../multiconductor.js';
	import type { DistAttachmentKind } from '@tellegen/engine';

	const app = getAppState();
	const ctrl = getController();

	const NEUTRAL_RGBA = [120, 114, 102, 255] as const;
	const ATTACHMENT_LEGEND: DistAttachmentKind[] = [
		'source',
		'generator',
		'ibr',
		'load',
		'shunt'
	];

	function terminalColor(t: string): string {
		return rgbaCss(isPhaseTerminal(t) ? phaseColor(t) : [...NEUTRAL_RGBA]);
	}
</script>

{#if app.activeMulti}
	{@const mc = app.activeMulti}
	{@const s = mc.summary}
	<h2>{mc.label} <span class="region mono">via {mc.fileName}</span></h2>
	{#if s}
		<p class="tag mono">multiconductor &#8901; viewing only</p>
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
			{#if s.n_ibr > 0 || s.n_shunt > 0}
				<div>
					<dt>IBRs / shunts</dt>
					<dd>{s.n_ibr} / {s.n_shunt}</dd>
				</div>
			{/if}
		</dl>

		{#if s.warnings.length > 0}
			<ul class="warnings mono">
				{#each s.warnings.slice(0, 4) as w, i (i)}
					<li>{w}</li>
				{/each}
				{#if s.warnings.length > 4}
					<li>+{s.warnings.length - 4} more</li>
				{/if}
			</ul>
		{/if}

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
			<p class="footnote mono">coordinates: synthetic topology layout centered where you placed it</p>
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
	{:else if mc.placed}
		<p class="footnote mono">select a bus to expand its terminals and conductors</p>
	{/if}

	<div class="legend mono">
		<span class="legend-row">
			<i class="swatch" style={`--sc:${rgbaCss([...phaseColor('1')])}`}></i>a
			<i class="swatch" style={`--sc:${rgbaCss([...phaseColor('2')])}`}></i>b
			<i class="swatch" style={`--sc:${rgbaCss([...phaseColor('3')])}`}></i>c
			<i class="swatch" style={`--sc:${rgbaCss([...NEUTRAL_RGBA])}`}></i>n
		</span>
		<span class="legend-row">
			{#each ATTACHMENT_LEGEND as kind (kind)}
				<i class="swatch" style={`--sc:${rgbaCss([...attachmentColor(kind)])}`}></i>{attachmentGlyph(
					kind
				)}
			{/each}
		</span>
	</div>

	<p class="footnote mono">parsed in your browser by powerio (wasm); never uploaded</p>

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

	.legend {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 12px;
		font-size: 10px;
		color: var(--text-secondary);
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
