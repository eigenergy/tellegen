<script lang="ts">
	import { sensGradient } from '../colors.js';
	import { getAppState, getController } from '../context.svelte.js';
	import { signedExp } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
	const reportedUnits = $derived(
		(ctrl.selectedSensitivity?.units ?? '(objective units/MW)/MW').replaceAll(
			'objective_unit',
			'objective units'
		)
	);
	const busUnitTitle = $derived(
		`LMP response to active demand, in ${reportedUnits}. The response is small while the active set stays unchanged.`
	);
	const branchUnitTitle = $derived(
		`LMP response to the thermal rating, in ${reportedUnits}. The column is zero when the limit is inactive.`
	);

	// Branch mode: the selection is a line and the column is price/rating; the
	// legend and preview pipeline are identical (both columns are bus-keyed).
	const branchMode = $derived(app.selectedBranch !== null);
	const unitTitle = $derived(branchMode ? branchUnitTitle : busUnitTitle);
	const previewUnits = $derived(app.previewPrices?.units ?? 'objective units/MW');

	const previewStep = $derived(
		branchMode
			? ctrl.ratingSliderValue - ctrl.committedRating
			: ctrl.sliderValue - ctrl.committedDelta
	);

	// Label the selected line by its bus pair, matching BindingLines and
	// RatingSlider; fall back to the raw branch id while the network loads.
	const branchLabel = $derived.by(() => {
		const b = ctrl.selectedBranchData;
		return b ? `${b.from} – ${b.to}` : String(app.selectedBranch);
	});
</script>

<div class="mode">
	<span class="chip">
		{ctrl.previewing ? 'LMP preview' : branchMode ? '∂LMP/∂rating' : '∂LMP/∂d'}
	</span>
	<span class="mono dim">
		{#if branchMode}line {branchLabel}{:else}bus {app.selectedBus}{/if}
	</span>
	<button class="mono" onclick={ctrl.clearSelection}
		><span class="key-hint">esc&nbsp;</span>clear</button
	>
</div>
<div class="sensitivity-readout" aria-live="polite">
	{#if ctrl.previewing}
		<p class="dim small">
			{ctrl.activeSolvable?.solving
				? 'Exact solve running; the map keeps the LMP preview.'
				: 'First order LMP preview. Release for the exact solve.'}
		</p>
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels mono single">
				<span
					>uniform {signedExp(ctrl.sensSummary.mean * previewStep)}
					{previewUnits}</span
				>
			</div>
		{:else if ctrl.previewScale}
			<!-- The bounds are fixed for the whole drag (column scale × full slider
			     deflection), so the ramp ends label the colors: full green/purple is
			     the predicted LMP change at full deflection, and intensity grows with the
			     step instead of renormalizing every frame. -->
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels mono">
				<span>&minus;{ctrl.previewScale.toExponential(1)}</span>
				<span>&Delta; LMP {previewUnits}</span>
				<span>+{ctrl.previewScale.toExponential(1)}</span>
			</div>
		{/if}
	{:else}
		<p class="dim small sensitivity-copy" title={unitTitle}>
			{#if branchMode}
				LMP response to the rating on line {branchLabel}.
			{:else}
				LMP response to demand at bus {app.selectedBus}.
			{/if}
			<span class="hint-dot mono" title={unitTitle} aria-label={unitTitle}>i</span>
		</p>
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels mono single">
				<span>uniform {signedExp(ctrl.sensSummary.mean)} {reportedUnits}</span>
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
				<span class="blink">
					computing {#if branchMode}&part;value/&part;fmax{:else}&part;value/&part;d{/if}&hellip;
				</span>
			</div>
		{/if}
	{/if}
</div>

<style>
	.chip {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 2px 8px;
		border: 1px solid var(--accent);
		color: var(--text-accent);
		background: var(--accent-soft);
		border-radius: 2px;
		white-space: nowrap;
	}

	/* .mode > button lives in the global .mode block in app.css, not here: scoping
	   it would raise its specificity above the global @media (max-width: 760px)
	   override and break the mobile layout. See the note by .mode in app.css. */

	.sensitivity-readout {
		min-height: 58px;
	}

	.sensitivity-copy {
		font-size: 11.5px;
		line-height: 1.35;
		white-space: nowrap;
	}

	/* No Esc key to name on a touch device. */
	@media (hover: none), (pointer: coarse) {
		.key-hint {
			display: none;
		}
	}
</style>
