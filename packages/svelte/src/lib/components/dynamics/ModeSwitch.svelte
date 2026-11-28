<script lang="ts">
	/** The switch between steady state and dynamics, which changes the whole app.
	 * @block Mode switch */
	import { getAppState } from '../../context.svelte.js';
	const app = getAppState();
</script>

<!-- A two-way toggle, not tabs: the mode changes the whole app, not one panel. -->
<div class="mode" role="group" aria-label="Analysis mode">
	<button
		aria-pressed={app.mode === 'steady'}
		onclick={() => (app.mode = 'steady')}
		data-testid="mode-steady">Steady state</button
	>
	<button
		aria-pressed={app.mode === 'dynamics'}
		onclick={() => (app.mode = 'dynamics')}
		data-testid="mode-dynamics">Dynamics</button
	>
</div>

<style>
	.mode {
		display: inline-flex;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-control);
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
	button + button {
		border-left: 1px solid var(--border-default);
	}
	button[aria-pressed='true'] {
		background: var(--accent-fill);
		color: var(--on-accent);
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -2px;
	}
</style>
