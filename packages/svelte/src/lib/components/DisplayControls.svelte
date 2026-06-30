<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { displayFmt, formulationLabel } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
</script>

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
