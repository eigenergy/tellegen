<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { displayFmt } from '../format.js';
	import { LOADING_AMBER, LOADING_ATTENTION, LOADING_BASE, LOADING_RED } from '../colors.js';

	const app = getAppState();
	const ctrl = getController();
	/** The single-line diagram replaces the map: these colours then apply only to its Map display
	 * view, and the diagram carries its own key. */
	const diagramOpen = $derived(!!ctrl.singleLineShown);
	const rgb = (c: readonly number[]) => `rgb(${c.slice(0, 3).join(' ')})`;
	/** The map's line colours, in the same three steps as the diagram's Limits key. */
	const LINE_KEY = [
		{ label: `Below ${Math.round(LOADING_ATTENTION * 100)}%`, color: rgb(LOADING_BASE) },
		{ label: 'Near limit', color: rgb(LOADING_AMBER) },
		{ label: 'At limit', color: rgb(LOADING_RED) }
	];
	/** The last calculation failed: the status line above already says so. */
	const failed = $derived.by(() => {
		const c = ctrl.activeSolvable;
		return (
			!!c && !c.solving && (!!c.failedCalculation || (c.solveBackend != null && c.solveMs == null))
		);
	});
</script>

{#if ctrl.displayOptions.length > 0 || app.sensitivityLoading}
	<div class="mode display-mode">
		{#if ctrl.displayOptions.length > 0}
			<div class="segment" aria-label="Bus color variable">
				{#each ctrl.displayOptions as option (option.mode)}
					<button
						type="button"
						class={{ active: app.displayMode === option.mode }}
						aria-pressed={app.displayMode === option.mode}
						onclick={() => (app.displayMode = option.mode)}>{option.label}</button
					>
				{/each}
			</div>
		{/if}
		{#if app.sensitivityLoading}
			<span class="dim blink">Loading&hellip;</span>
		{/if}
	</div>
{/if}
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
				{displayFmt(ctrl.activeDisplay.mode, ctrl.displayStats.uniform)}{ctrl.activeDisplay.unit
					? ` ${ctrl.activeDisplay.unit}`
					: ''} at every bus
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
	<p class="legend-title">Lines</p>
	<ul class="line-key" aria-label="Line colors">
		{#each LINE_KEY as item (item.label)}
			<li><i style:background={item.color}></i>{item.label}</li>
		{/each}
	</ul>
{:else if ctrl.activeSolvable?.solving}
	<p class="dim small blink">Solving&hellip;</p>
{:else if !failed}
	<p class="dim small">No result</p>
{/if}

<style>
	.legend-title {
		margin: 10px 0 0;
		font-size: 12px;
		font-weight: 600;
		color: var(--ink);
	}
	.line-key {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		margin: 4px 0 0;
		padding: 0;
		list-style: none;
		font-size: 12px;
		color: var(--text-secondary);
	}
	.line-key li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.line-key i {
		width: 18px;
		height: 3px;
		border-radius: 2px;
	}
</style>
