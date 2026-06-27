<script lang="ts">
	import { sensGradient } from '$lib/colors';
	import { getAppState, getController } from '$lib/context.svelte';
	import { signedExp } from '$lib/format';

	const app = getAppState();
	const ctrl = getController();
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

<style>
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

	.sensitivity-readout {
		min-height: 58px;
	}

	.sensitivity-copy {
		font-size: 11.5px;
		line-height: 1.35;
		white-space: nowrap;
	}
</style>
