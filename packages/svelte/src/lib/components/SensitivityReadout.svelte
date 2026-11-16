<script lang="ts">
	import { sensGradient } from '../colors.js';
	import { getAppState, getController } from '../context.svelte.js';
	import { dollarUnits } from '../display.js';
	import { lineName, signedSmall, small } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
	/** The engine's units, read out: ($/MWh)/MW becomes "$/MWh per MW". A rating's
	 * ($/MWh)/MVA is read per MW too, the unit the rating slider beside it moves in. */
	const perMw = (units: string) =>
		dollarUnits(units).replace(/\(\$\/MWh\)\/(MW|MVA)\b/, '$$/MWh per MW');
	const reportedUnits = $derived(perMw(ctrl.selectedSensitivity?.units ?? '($/MWh)/MW'));

	// Branch mode: the selection is a line and the column is price/rating; the
	// legend and preview pipeline are identical (both columns are bus-keyed).
	const branchMode = $derived(app.selectedBranch !== null);
	const previewUnits = $derived(dollarUnits(app.previewPrices?.units ?? '$/MWh'));

	const previewStep = $derived(
		branchMode
			? ctrl.ratingSliderValue - ctrl.committedRating
			: ctrl.sliderValue - ctrl.committedDelta
	);

	// Name the selected line as BindingLines and the diagram do; fall back to the raw
	// branch id while the network loads.
	const title = $derived.by(() => {
		if (!branchMode) return `Bus ${app.selectedBus}`;
		const b = ctrl.selectedBranchData;
		return b ? lineName(b) : `Line ${app.selectedBranch}`;
	});
</script>

<div class="mode readout-head">
	<h3 class="readout-title" data-testid="sensitivity-title">{title}</h3>
	<button onclick={ctrl.clearSelection} aria-keyshortcuts="Escape" title="Esc">Clear</button>
</div>
<div class="sensitivity-readout" data-testid="sensitivity-readout" aria-live="polite">
	{#if ctrl.previewing}
		<p class="dim small">{ctrl.activeSolvable?.solving ? 'Solving…' : 'Estimate'}</p>
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels single">
				<span
					>Uniform: {signedSmall(ctrl.sensSummary.mean * previewStep)}
					{previewUnits}</span
				>
			</div>
		{:else if ctrl.previewScale}
			<!-- The bounds are fixed for the whole drag (column scale × full slider
			     deflection), so the ramp ends label the colors: full green/purple is
			     the predicted price change at full deflection, and intensity grows with the
			     step instead of renormalizing every frame. -->
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels">
				<span>&minus;{small(ctrl.previewScale)}</span>
				<span>Price change ({previewUnits})</span>
				<span>+{small(ctrl.previewScale)}</span>
			</div>
		{/if}
	{:else}
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels single">
				<span>Uniform: {signedSmall(ctrl.sensSummary.mean)} {reportedUnits}</span>
			</div>
		{:else if ctrl.sensSummary}
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels">
				<span>&minus;{small(ctrl.sensSummary.scale)}</span>
				<span>Price change ({reportedUnits})</span>
				<span>+{small(ctrl.sensSummary.scale)}</span>
			</div>
		{:else if app.sensitivityLoading}
			<div class="legend" style:background="var(--line)" style:opacity="0.4"></div>
			<div class="legend-labels single">
				<span class="blink">Computing&hellip;</span>
			</div>
		{/if}
	{/if}
</div>

<style>
	/* The title on the left and Clear on the right, on a phone too. */
	.readout-head {
		align-items: flex-start;
		justify-content: space-between;
		flex-wrap: nowrap;
	}

	.readout-title {
		margin: 0;
		font-size: var(--fs-body);
		font-weight: 600;
		line-height: var(--lh-snug);
		color: var(--ink);
	}

	/* .mode > button lives in the global .mode block in app.css, not here: scoping
	   it would raise its specificity above the global @media (max-width: 760px)
	   override and break the mobile layout. See the note by .mode in app.css. */

	.sensitivity-readout {
		margin-bottom: 4px;
	}
</style>
