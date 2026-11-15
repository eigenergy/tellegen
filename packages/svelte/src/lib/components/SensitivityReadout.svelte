<script lang="ts">
	import { sensGradient } from '../colors.js';
	import { getAppState, getController } from '../context.svelte.js';
	import { dollarUnits } from '../display.js';
	import { signedSmall, small } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
	/** The engine's units, read out: ($/MWh)/MW becomes "$/MWh per MW", and a
	 * rating's ($/MWh)/MVA becomes "$/MWh per MVA". */
	const perMw = (units: string) =>
		dollarUnits(units).replace(/\(\$\/MWh\)\/(MW|MVA)\b/, '$$/MWh per $1');
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

	// Label the selected line by its bus pair, matching BindingLines and
	// RatingSlider; fall back to the raw branch id while the network loads.
	const branchLabel = $derived.by(() => {
		const b = ctrl.selectedBranchData;
		return b ? `${b.from} – ${b.to}` : String(app.selectedBranch);
	});
	const title = $derived(
		ctrl.previewing
			? branchMode
				? `Price preview for line ${branchLabel}`
				: `Price preview for bus ${app.selectedBus}`
			: branchMode
				? `Price change per ${reportedUnits.endsWith('MVA') ? 'MVA' : 'MW'} of rating on line ${branchLabel}`
				: `Price change per MW of demand at bus ${app.selectedBus}`
	);
</script>

<div class="mode readout-head">
	<h3 class="readout-title" data-testid="sensitivity-title">{title}</h3>
	<button onclick={ctrl.clearSelection} aria-keyshortcuts="Escape" title="Esc">Clear</button>
</div>
<div class="sensitivity-readout" data-testid="sensitivity-readout" aria-live="polite">
	{#if ctrl.previewing}
		<p class="dim small">
			{ctrl.activeSolvable?.solving
				? 'Solving…'
				: 'Estimated from the current prices. Release to solve.'}
		</p>
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels single">
				<span
					>Every price changes by {signedSmall(ctrl.sensSummary.mean * previewStep)}
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
				<span>Every price changes by {signedSmall(ctrl.sensSummary.mean)} {reportedUnits}</span>
			</div>
		{:else if ctrl.sensSummary}
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels">
				<span>&minus;{small(ctrl.sensSummary.scale)}</span>
				<span>0</span>
				<span>+{small(ctrl.sensSummary.scale)}</span>
			</div>
			<p class="dim small units">In {reportedUnits}.</p>
		{:else if app.sensitivityLoading}
			<div class="legend" style:background="var(--line)" style:opacity="0.4"></div>
			<div class="legend-labels single">
				<span class="blink">Computing&hellip;</span>
			</div>
		{/if}
	{/if}
</div>

<style>
	.readout-head {
		align-items: flex-start;
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
		min-height: 58px;
	}

	.units {
		margin: 4px 0 0;
	}
</style>
