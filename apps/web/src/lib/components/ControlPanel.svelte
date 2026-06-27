<script lang="ts">
	import { busRadius, sensGradient } from '$lib/colors';
	import { getAppState, getController } from '$lib/context.svelte';
	import type { SolvableCase } from '$lib/controller.svelte';
	import {
		displayFmt,
		fmt,
		formulationHint,
		formulationLabel,
		signed,
		signedExp,
		splitName
	} from '$lib/format';
	import { FORMULATIONS, type Formulation } from '$lib/wasm';

	const app = getAppState();
	const ctrl = getController();

	const SIZE_SAMPLES = [10, 100, 500];
</script>

<aside class="panel">
	{#if app.error}
		<p class="error mono">{app.error}</p>
	{/if}
	{#if app.parsingFile}
		<p class="dim mono blink">parsing&hellip;</p>
	{/if}
	{#if app.activeLocal}
		{@const lc = app.activeLocal}
		<h2>{lc.label} <span class="region mono">via {lc.fileName}</span></h2>
		{#if lc.substations}
			<dl class="mono">
				<div>
					<dt>substations</dt>
					<dd>{lc.substations.points.length}</dd>
				</div>
			</dl>
			<p class="footnote mono">
				display only &mdash; positions inferred from the PowerWorld diagram, not surveyed latitude
				and longitude
			</p>
			<p class="footnote mono">decoded in your browser by powerio (wasm); never uploaded</p>
		{:else if lc.summary}
			<dl class="mono">
				<div>
					<dt>buses</dt>
					<dd>{lc.summary.n_bus}</dd>
				</div>
				<div>
					<dt>branches</dt>
					<dd>{lc.summary.n_branch}</dd>
				</div>
				<div>
					<dt>generators</dt>
					<dd>{lc.summary.n_gen}</dd>
				</div>
				<div>
					<dt>load</dt>
					<dd>{fmt.format(lc.summary.load_mw)} MW</dd>
				</div>
				<div>
					<dt>gen capacity</dt>
					<dd>{fmt.format(lc.summary.gen_mw)} MW</dd>
				</div>
				<div>
					<dt>base MVA</dt>
					<dd>{fmt.format(lc.summary.base_mva)}</dd>
				</div>
			</dl>
			{#if lc.summary.warnings.length > 0}
				<ul class="warnings mono">
					{#each lc.summary.warnings.slice(0, 4) as w, i (i)}
						<li>{w}</li>
					{/each}
					{#if lc.summary.warnings.length > 4}
						<li>+{lc.summary.warnings.length - 4} more</li>
					{/if}
				</ul>
			{/if}
			{#if !lc.view}
				<p class="footnote mono">
					no coordinates in this file &mdash; click the map or drop a geographic file
				</p>
			{:else if lc.coordsKind === 'synthetic'}
				<p class="footnote mono">
					coordinates: synthetic topology layout centered where you placed it
				</p>
			{:else if lc.coordsKind === 'geofile'}
				<p class="footnote mono">
					coordinates: geographic file data from {lc.geoSource}
				</p>
			{/if}
			{#if lc.geoWarnings && lc.geoWarnings.length > 0}
				<ul class="warnings mono">
					{#each lc.geoWarnings.slice(0, 4) as w, i (i)}
						<li>{w}</li>
					{/each}
					{#if lc.geoWarnings.length > 4}
						<li>+{lc.geoWarnings.length - 4} more</li>
					{/if}
				</ul>
			{/if}
			<p class="footnote mono">parsed in your browser by powerio (wasm); never uploaded</p>
		{/if}
		{#if lc.topology && lc.coordsKind !== 'file'}
			<button class="reset mono" onclick={() => ctrl.moveLocalCase(lc)}>
				{lc.coordsKind === 'synthetic_pending'
					? 'place on map'
					: lc.coordsKind === 'geofile'
						? 'place manually'
						: 'move layout'}
			</button>
		{/if}
		<button class="reset mono" onclick={() => ctrl.removeLocalCase(lc)}>remove</button>
	{/if}
	{#if !ctrl.networkStats}
		{#if !app.error && !app.activeLocal}
			{#if ctrl.casesLoaded && app.cases.length === 0}
				<p class="dim mono">no default cases loaded</p>
				<button class="reset mono" onclick={ctrl.restoreDefaultCases}>restore defaults</button>
			{:else if ctrl.loadingBackendCase}
				<p class="dim mono blink">loading selected case&hellip;</p>
			{:else}
				<p class="dim mono blink">loading cases&hellip;</p>
			{/if}
		{/if}
	{:else}
		{#if !app.activeLocal}
			{@const [cname, cregion] = splitName(app.active?.name ?? '')}
			<h2>{cname} <span class="region mono">{cregion}</span></h2>
			{@const deltaObjective = ctrl.stats?.deltaObjective}
			<dl class="mono">
				<div>
					<dt>buses</dt>
					<dd>{ctrl.networkStats.buses}</dd>
				</div>
				<div>
					<dt>branches</dt>
					<dd>{ctrl.networkStats.branches}</dd>
				</div>
				<div>
					<dt>binding lines</dt>
					<dd>{ctrl.networkStats.binding ?? '…'}</dd>
				</div>
				<div>
					<dt>cost</dt>
					<dd>
						{#if ctrl.networkStats.objective === null}
							<span class="blink">solving&hellip;</span>
						{:else}
							{fmt.format(ctrl.networkStats.objective)} $/h
						{/if}
					</dd>
				</div>
				{#if ctrl.isPerturbed(ctrl.activeSolvable) && deltaObjective !== null && deltaObjective !== undefined}
					<div class="delta">
						<dt>vs base</dt>
						<dd>{signed(deltaObjective)} $/h</dd>
					</div>
				{/if}
			</dl>
		{/if}

		{#if ctrl.activeSolvable}
			{@const c = ctrl.activeSolvable}
			<div class="formulation">
				<label
					class="formulation-row mono"
					for="formulation-select"
					title={formulationHint(c.formulation)}
				>
					<span>formulation</span>
					<select
						id="formulation-select"
						class="mono"
						disabled={c.solving}
						value={c.formulation}
						onchange={(e) => ctrl.changeFormulation(c, e.currentTarget.value as Formulation)}
					>
						{#each FORMULATIONS as f (f.id)}
							<option value={f.id} disabled={f.disabled}>
								{f.label}{f.disabled ? ' (coming soon)' : ''}
							</option>
						{/each}
					</select>
				</label>
			</div>
		{/if}

		<hr />

		{#if app.selectedBus !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)}
			{@const c = ctrl.activeSolvable as SolvableCase}
			<div class="mode">
				<span class="chip">{ctrl.previewing ? 'LMP preview' : '∂LMP/∂d'}</span>
				<span class="mono dim">bus {app.selectedBus}</span>
				<button class="mono" onclick={ctrl.clearSelection}>esc&nbsp;clear</button>
			</div>
			<div class="sensitivity-readout" aria-live="polite">
				{#if ctrl.previewing}
					<p class="dim small">
						{c.solving
							? 'Exact solve running; the map keeps the LMP preview.'
							: 'First order LMP preview. Release for the exact solve.'}
					</p>
				{:else}
					<p class="dim small sensitivity-copy">LMP response per MW at bus {app.selectedBus}.</p>
					{#if ctrl.sensSummary?.flat}
						<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
						<div class="legend-labels mono single">
							<span>uniform {signedExp(ctrl.sensSummary.mean)} ($/MWh)/MW</span>
						</div>
					{:else if ctrl.sensSummary}
						<div class="legend" style:background={sensGradient}></div>
						<div class="legend-labels mono">
							<span>&minus;{ctrl.sensSummary.scale.toExponential(1)}</span>
							<span>0</span>
							<span>+{ctrl.sensSummary.scale.toExponential(1)}</span>
						</div>
					{:else if app.sensitivityLoading}
						<div class="legend" style:background="var(--line)" style:opacity="0.4"></div>
						<div class="legend-labels mono single">
							<span class="blink">computing &part;LMP/&part;d&hellip;</span>
						</div>
					{/if}
				{/if}
			</div>

			<div class="slider-block">
				<div class="slider-head mono">
					<span>&Delta; demand</span>
					<span class="val">{signed(ctrl.sliderValue)} MW</span>
				</div>
				<div class="range-mode">
					<div class="segment mono" aria-label="demand range">
						<button
							type="button"
							class:active={app.demandRangeMode === 'local'}
							aria-pressed={app.demandRangeMode === 'local'}
							aria-label="nearby demand range"
							title="range near the selected demand setting"
							onclick={() => ctrl.setDemandRangeMode('local')}>nearby</button
						>
						<button
							type="button"
							class:active={app.demandRangeMode === 'full'}
							aria-pressed={app.demandRangeMode === 'full'}
							aria-label="full demand range"
							title="range from zero load to the local physical limit"
							onclick={() => ctrl.setDemandRangeMode('full')}>full range</button
						>
					</div>
					<span class="mono dim">{fmt.format(ctrl.sliderMin)} to {fmt.format(ctrl.sliderMax)} MW</span>
				</div>
				<input
					type="range"
					min={ctrl.sliderMin}
					max={ctrl.sliderMax}
					step="0.5"
					bind:value={ctrl.sliderCurrent, ctrl.setSliderPreview}
					aria-label="demand delta at selected bus"
					onpointerdown={() => ctrl.setSliderPreview(ctrl.sliderValue)}
					onkeydown={() => ctrl.setSliderPreview(ctrl.sliderValue)}
					onpointerup={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
					onmouseup={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
					onclick={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
					onkeyup={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
					onblur={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
					onchange={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
				/>
				<div class="demand-feedback" class:idle={!ctrl.previewing && !ctrl.isPerturbed(c)}>
					<p class="pred mono dim" aria-hidden={!(ctrl.predictedDeltaObj !== null && ctrl.previewing)}>
						{#if ctrl.predictedDeltaObj !== null && ctrl.previewing}
							predicted &Delta;cost {signed(ctrl.predictedDeltaObj)} $/h
						{:else}
							&nbsp;
						{/if}
					</p>
					<p class="score mono" aria-hidden={!(ctrl.gradientScore && ctrl.isPerturbed(c))}>
						{#if ctrl.gradientScore && ctrl.isPerturbed(c)}
							gradient {signed(ctrl.gradientScore.pred)} &middot; exact {signed(ctrl.gradientScore.exact)}
							$/h
						{:else}
							&nbsp;
						{/if}
					</p>
					<div class="reset-row">
						{#if ctrl.isPerturbed(c)}
							<button class="reset mono" onclick={() => ctrl.resetCase(c)}>reset demand</button>
						{/if}
					</div>
				</div>
			</div>

			{#if ctrl.showMoverSlot}
				<div class="movers-block">
					{#if !ctrl.previewing && ctrl.topMovers.length > 0}
						<table class="mono">
							<tbody>
								{#each ctrl.topMovers as mover (mover.bus)}
									<tr>
										<td>bus {mover.bus}</td>
										<td class:pos={mover.value > 0} class:neg={mover.value < 0}>
											{mover.value >= 0 ? '+' : ''}{mover.value.toExponential(2)}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					{/if}
				</div>
			{/if}
		{:else}
			<div class="mode display-mode">
				<div class="segment mono" aria-label="bus color variable">
					{#each ctrl.displayOptions as option (option.mode)}
						<button
							type="button"
							class:active={app.displayMode === option.mode}
							aria-pressed={app.displayMode === option.mode}
							onclick={() => (app.displayMode = option.mode)}>{option.label}</button
						>
					{/each}
				</div>
				<span class="mono dim">{ctrl.activeDisplay?.unit ?? ''}</span>
				{#if app.sensitivityLoading}
					<span class="mono dim blink">&part; loading&hellip;</span>
				{/if}
			</div>
			{#if ctrl.activeDisplay && ctrl.displayStats}
				<p class="dim small">{ctrl.activeDisplay.copy}</p>
				<div class="legend" style:background={ctrl.activeDisplay.gradient}></div>
				<div class="legend-labels mono">
					{#if ctrl.displayStats.uniform !== null}
						<span>
							uniform {displayFmt(ctrl.activeDisplay.mode, ctrl.displayStats.uniform)}
							{ctrl.activeDisplay.unit}
						</span>
					{:else}
						<span>
							{ctrl.displayStats.lo.clamped ? '≤' : ''}{displayFmt(
								ctrl.activeDisplay.mode,
								ctrl.displayStats.lo.value
							)}
						</span>
						<span>
							{ctrl.displayStats.hi.clamped ? '≥' : ''}{displayFmt(
								ctrl.activeDisplay.mode,
								ctrl.displayStats.hi.value
							)}
						</span>
					{/if}
				</div>
			{:else}
				<p class="dim small blink">Solving {formulationLabel(ctrl.activeFormulation)}&hellip;</p>
			{/if}
		{/if}

		<hr />

		<div class="sizes">
			{#each SIZE_SAMPLES as mw (mw)}
				<span class="size mono">
					<i style:width="{2 * busRadius(mw)}px" style:height="{2 * busRadius(mw)}px"></i>
					{mw}
				</span>
			{/each}
			<span class="mono dim caption">MW, max(load,&#8201;gen)</span>
		</div>
	{/if}
</aside>

<style>
	.panel {
		position: absolute;
		top: 64px;
		left: 20px;
		z-index: 10;
		width: 312px;
		max-height: calc(100% - 110px);
		overflow-y: auto;
		padding: 16px 18px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.5s 0.12s ease-out both;
	}

	h2 {
		margin: 0 0 12px;
		font-size: 16px;
		font-weight: 600;
	}

	.region {
		font-size: 10px;
		font-weight: 400;
		color: var(--ink-dim);
		text-transform: uppercase;
		letter-spacing: 0;
		margin-left: 4px;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
	}

	dl .delta dd {
		color: var(--accent);
	}

	dt {
		color: var(--ink-dim);
	}

	dd {
		margin: 0;
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 10px;
		color: var(--ink-faint);
		letter-spacing: 0;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--accent);
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}

	.mode {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 12px;
	}

	.display-mode {
		gap: 8px;
	}

	.display-mode .segment button {
		font-size: 11px;
		padding: 2px 9px;
	}

	.chip {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 2px 8px;
		border: 1px solid var(--accent);
		color: var(--accent);
		background: var(--accent-soft);
		border-radius: 2px;
		white-space: nowrap;
	}

	.mode > button {
		margin-left: auto;
		font-size: 10.5px;
		padding: 2px 7px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.mode > button:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.small {
		font-size: 12px;
		line-height: 1.55;
	}

	.error {
		color: var(--red);
		font-size: 12px;
	}

	.sensitivity-readout {
		min-height: 58px;
	}

	.sensitivity-copy {
		font-size: 11.5px;
		line-height: 1.35;
		white-space: nowrap;
	}

	.legend {
		height: 6px;
		border-radius: 3px;
		margin-top: 6px;
	}

	.legend-labels {
		display: flex;
		justify-content: space-between;
		font-size: 10.5px;
		color: var(--ink-faint);
		margin-top: 4px;
	}

	.legend-labels.single {
		justify-content: center;
		text-align: center;
	}

	.slider-block {
		margin-top: 14px;
	}

	.slider-head {
		display: flex;
		justify-content: space-between;
		font-size: 11.5px;
		color: var(--ink-dim);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	.formulation {
		margin-top: 10px;
	}

	.formulation-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: 12px;
		color: var(--ink);
	}

	.formulation-row select {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 3px 22px 3px 8px;
		border: 1px solid var(--line);
		border-radius: 2px;
		background: rgba(252, 251, 247, 0.55);
		color: var(--ink);
		cursor: pointer;
		/* Native arrow on the right, drawn so the control reads as a control in the panel. */
		appearance: none;
		-webkit-appearance: none;
		background-image:
			linear-gradient(45deg, transparent 50%, var(--ink-dim) 50%),
			linear-gradient(135deg, var(--ink-dim) 50%, transparent 50%);
		background-position:
			right 10px center,
			right 6px center;
		background-size:
			4px 4px,
			4px 4px;
		background-repeat: no-repeat;
	}

	.formulation-row select:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--accent);
	}

	.formulation-row select:disabled {
		opacity: 0.55;
		cursor: progress;
	}

	.range-mode {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 6px 0 7px;
		font-size: 10.5px;
	}

	.segment {
		display: inline-flex;
		border: 1px solid var(--line);
		border-radius: 2px;
		overflow: hidden;
		background: rgba(252, 251, 247, 0.55);
	}

	.segment button {
		padding: 2px 8px;
		border: 0;
		background: transparent;
		color: var(--ink-dim);
		font: inherit;
		cursor: pointer;
	}

	.segment button + button {
		border-left: 1px solid var(--line);
	}

	.segment button.active {
		background: var(--accent-soft);
		color: var(--accent);
	}

	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: 4px;
		background: var(--line);
		border-radius: 2px;
		outline-offset: 4px;
		margin: 6px 0;
	}

	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	input[type='range']::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	.pred {
		margin: 2px 0 0;
		font-size: 11px;
		min-height: 16px;
	}

	.score {
		margin: 8px 0 0;
		font-size: 11px;
		color: var(--ink);
		min-height: 16px;
	}

	.demand-feedback {
		min-height: 78px;
	}

	/* On a fresh selection (no preview, no perturbation) the predicted/gradient/
	   reset rows are empty; collapse the reserved block so the panel isn't padded
	   with whitespace. The reservation returns during interaction to avoid jumps. */
	.demand-feedback.idle {
		min-height: 0;
	}

	.demand-feedback.idle .pred,
	.demand-feedback.idle .score {
		display: none;
	}

	.reset-row {
		min-height: 28px;
	}

	.movers-block {
		min-height: 114px;
	}

	.reset {
		margin-top: 10px;
		font-size: 10.5px;
		padding: 3px 10px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.reset:hover {
		border-color: var(--red);
		color: var(--red);
	}

	table {
		width: 100%;
		margin-top: 12px;
		border-collapse: collapse;
		font-size: 12px;
	}

	td {
		padding: 3px 0;
		border-top: 1px solid var(--line);
	}

	td:last-child {
		text-align: right;
	}

	.pos {
		color: var(--pos);
	}

	.neg {
		color: var(--neg);
	}

	.sizes {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 10px;
		color: var(--ink-dim);
	}

	.size {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.size i {
		display: inline-block;
		border-radius: 50%;
		background: rgba(212, 116, 34, 0.55);
		border: 1px solid rgba(46, 42, 34, 0.45);
	}

	.caption {
		margin-left: auto;
		font-size: 9.5px;
	}

	.blink {
		animation: blink 1.2s steps(2) infinite;
	}

	@media (max-width: 760px) {
		.panel {
			top: auto;
			left: 10px;
			right: 10px;
			bottom: 40px;
			width: auto;
			max-height: 44dvh;
			padding: 14px 16px;
		}

		.mode {
			flex-wrap: wrap;
			gap: 7px;
		}

		.mode button {
			margin-left: 0;
		}

		.sizes {
			flex-wrap: wrap;
			gap: 8px 12px;
		}

		.caption {
			margin-left: 0;
			flex-basis: 100%;
		}
	}

	@media (max-width: 420px) {
		.panel {
			bottom: 34px;
			max-height: 46dvh;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.panel {
			animation: none;
		}
	}
</style>
