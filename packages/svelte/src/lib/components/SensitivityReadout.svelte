<script lang="ts">
	import { sensGradient } from '../colors.js';
	import { getAppState, getController } from '../context.svelte.js';
	import { dollarUnits } from '../display.js';
	import { lineName, readable, signedSmall, small } from '../format.js';
	import { dispatchByBus } from '../diagram/transmission-inspection.js';

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

	/** The selection's own numbers from the solved case, as labelled values. */
	const values = $derived.by(() => {
		const solution = ctrl.activeSolvable?.solution;
		if (branchMode) {
			const b = ctrl.selectedBranchData;
			const flow = solution?.flows.find((f) => f.branch === app.selectedBranch);
			return [
				['Flow', readable(flow ? Math.abs(flow.mw) : null, 'MW').text],
				['Loading', readable(flow && b && b.rate_mw > 0 ? flow.loading * 100 : null, '%').text],
				['Rating', readable(b && b.rate_mw > 0 ? b.rate_mw : null, 'MW').text]
			] as const;
		}
		const bus = ctrl.selectedBusData;
		const price = solution?.prices.find((p) => p.bus === app.selectedBus)?.value;
		const generation = solution
			? dispatchByBus(solution.dispatch).get(app.selectedBus!)
			: undefined;
		return [
			['Price', readable(price, '$/MWh').text],
			['Demand', readable(bus ? bus.demand_mw + ctrl.committedDelta : null, 'MW').text],
			['Generation', readable(generation ?? (solution && bus ? 0 : null), 'MW').text]
		] as const;
	});

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
<dl class="values" aria-label="Selection values">
	{#each values as [label, value] (label)}
		<div>
			<dt>{label}</dt>
			<dd>{value}</dd>
		</div>
	{/each}
</dl>
<div class="sensitivity-readout" data-testid="sensitivity-readout" aria-live="polite">
	{#if ctrl.previewing}
		<p class="dim small">{ctrl.activeSolvable?.solving ? 'Solving…' : 'Estimate'}</p>
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels single">
				<span
					>{signedSmall(ctrl.sensSummary.mean * previewStep)}
					{previewUnits} at every bus</span
				>
			</div>
		{:else if ctrl.previewScale}
			<!-- The bounds are fixed for the whole drag (column scale × full slider
			     deflection), so the ramp ends label the colors: full green/purple is
			     the predicted price change at full deflection, and intensity grows with the
			     step instead of renormalizing every frame. -->
			<p class="legend-caption">Price change ({previewUnits})</p>
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels">
				<span>&minus;{small(ctrl.previewScale)}</span>
				<span>+{small(ctrl.previewScale)}</span>
			</div>
		{/if}
	{:else}
		{#if ctrl.sensSummary?.flat}
			<div class="legend flat" style:background={ctrl.flatSensBackground}></div>
			<div class="legend-labels single">
				<span>Price change {signedSmall(ctrl.sensSummary.mean)} {reportedUnits} at every bus</span>
			</div>
		{:else if ctrl.sensSummary}
			<p class="legend-caption">Price change ({reportedUnits})</p>
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels">
				<span>&minus;{small(ctrl.sensSummary.scale)}</span>
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
	/* The colour bar's name on its own line, so its two ends read as the only values under it. */
	.legend-caption {
		margin: var(--space-xs) 0 0;
		font-size: var(--fs-label);
		color: var(--text-secondary);
	}

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
	.values {
		display: grid;
		gap: 2px;
		margin: 6px 0 8px;
		font-size: var(--fs-label);
	}
	.values div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
	}
	.values dt {
		color: var(--text-secondary);
	}
	.values dd {
		margin: 0;
		font-variant-numeric: tabular-nums;
	}
</style>
