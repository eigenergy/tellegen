<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { displayFmt, formulationLabel } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
</script>

<div class="mode display-mode">
	<div class="segment" aria-label="Bus color variable">
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
		<span class="dim blink">Loading&hellip;</span>
	{/if}
</div>
{#if ctrl.activeDisplay && ctrl.displayStats}
	<p class="legend-title">{ctrl.activeDisplay.copy}</p>
	<div class="legend" style:background={ctrl.activeDisplay.gradient}></div>
	<div class="legend-labels mono">
		{#if ctrl.displayStats.uniform !== null}
			<span>
				Uniform {displayFmt(ctrl.activeDisplay.mode, ctrl.displayStats.uniform)}
				{ctrl.activeDisplay.unit}
			</span>
		{:else}
			<span>
				{ctrl.displayStats.lo.clamped ? '≤' : ''}{displayFmt(
					ctrl.activeDisplay.mode,
					ctrl.displayStats.lo.value
				)}
				{ctrl.activeDisplay.unit}
			</span>
			<span>
				{ctrl.displayStats.hi.clamped ? '≥' : ''}{displayFmt(
					ctrl.activeDisplay.mode,
					ctrl.displayStats.hi.value
				)}
				{ctrl.activeDisplay.unit}
			</span>
		{/if}
	</div>
{:else if ctrl.activeSolvable?.solving}
	<p class="dim small blink">
		Solving {formulationLabel(ctrl.activeFormulation)}&hellip;
	</p>
{:else}
	<p class="dim small">
		No {formulationLabel(ctrl.activeFormulation)} results available.
	</p>
{/if}

<style>
	.legend-title {
		margin: 10px 0 0;
		font-size: 12px;
		font-weight: 600;
		color: var(--ink);
	}
</style>
