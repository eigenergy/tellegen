<script lang="ts">
	import { sensGradient } from '../colors.js';
	import { getAppState, getController } from '../context.svelte.js';
	import { signedExp } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
	const unitTitle =
		'LMP is measured in $/MWh and demand is perturbed in MW, so dLMP/dd has units ($/MWh)/MW. ' +
		'DC LMPs move only when a binding constraint changes, so most nudges in this slider range ' +
		'shift prices very little unless the active set changes.';

	const previewStep = $derived(ctrl.sliderValue - ctrl.committedDelta);
</script>

<div class="mode">
	<span class="chip">{ctrl.previewing ? 'LMP preview' : '∂LMP/∂d'}</span>
	<span class="mono dim">bus {app.selectedBus}</span>
	<button class="mono" onclick={ctrl.clearSelection}>esc&nbsp;clear</button>
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
				<span>uniform {signedExp(ctrl.sensSummary.mean * previewStep)} $/MWh</span>
			</div>
		{:else if ctrl.previewScale}
			<!-- The bounds are fixed for the whole drag (column scale × full slider
			     deflection), so the ramp ends label the colors: full green/purple is
			     the predicted ΔLMP at full deflection, and intensity grows with the
			     step instead of renormalizing every frame. -->
			<div class="legend" style:background={sensGradient}></div>
			<div class="legend-labels mono">
				<span>&minus;{ctrl.previewScale.toExponential(1)}</span>
				<span>&Delta;LMP $/MWh</span>
				<span>+{ctrl.previewScale.toExponential(1)}</span>
			</div>
		{/if}
	{:else}
		<p class="dim small sensitivity-copy" title={unitTitle}>
			LMP response per MW of demand at bus {app.selectedBus}.
			<span class="hint-dot mono" title={unitTitle} aria-label={unitTitle}>i</span>
		</p>
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
</style>
