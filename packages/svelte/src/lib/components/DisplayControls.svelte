<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { displayFmt } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
	/** The single-line diagram replaces the map: these colours then apply only to its Map display
	 * view, and the diagram carries its own key. */
	const diagramOpen = $derived(!!ctrl.singleLineShown);
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
	{#if app.sensitivityLoading}
		<span class="dim blink">Loading&hellip;</span>
	{/if}
</div>
{#if diagramOpen}
	<!-- The diagram carries its own key. -->
{:else if ctrl.activeDisplay && ctrl.displayStats}
	<p class="legend-title">
		{ctrl.activeDisplay.copy}{ctrl.activeDisplay.unit ? ` (${ctrl.activeDisplay.unit})` : ''}
	</p>
	<div class="legend" style:background={ctrl.activeDisplay.gradient}></div>
	<div class="legend-labels">
		{#if ctrl.displayStats.uniform !== null}
			<span>
				Uniform: {displayFmt(ctrl.activeDisplay.mode, ctrl.displayStats.uniform)}
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
{:else if ctrl.activeSolvable?.solving}
	<p class="dim small blink">Solving&hellip;</p>
{:else}
	<p class="dim small">No result</p>
{/if}

<style>
	.legend-title {
		margin: 10px 0 0;
		font-size: 12px;
		font-weight: 600;
		color: var(--ink);
	}
</style>
