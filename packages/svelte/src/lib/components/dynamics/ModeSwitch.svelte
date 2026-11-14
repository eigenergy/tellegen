<script lang="ts">
	import { getAppState } from '../../context.svelte.js';
	const app = getAppState();
</script>

<div class="mode" role="tablist" aria-label="Analysis mode">
	<button
		role="tab"
		aria-selected={app.mode === 'steady'}
		onclick={() => (app.mode = 'steady')}
		data-testid="mode-steady">Steady state</button
	>
	<button
		role="tab"
		aria-selected={app.mode === 'dynamics'}
		onclick={() => (app.mode = 'dynamics')}
		data-testid="mode-dynamics">Dynamics</button
	>
</div>

<style>
	.mode {
		display: inline-flex;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		overflow: hidden;
		background: var(--surface-control);
	}
	button {
		padding: 4px 10px;
		border: none;
		background: none;
		color: var(--text-secondary);
		font: inherit;
		font-size: var(--fs-control);
		cursor: pointer;
		white-space: nowrap;
	}
	button[aria-selected='true'] {
		background: var(--accent-soft);
		color: var(--ink);
		box-shadow: var(--inset-active);
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -2px;
	}
</style>
