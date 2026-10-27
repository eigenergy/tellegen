<script lang="ts">
	import { getController, getUiConfig } from '../context.svelte.js';

	const ctrl = getController();
	const config = getUiConfig();
</script>

{#if config.loadDefaultCases && ctrl.casesLoaded && ctrl.hiddenDefaults.size > 0}
	<button class="restore-defaults" onclick={ctrl.restoreDefaultCases}>Restore default cases</button>
{/if}

<style>
	.restore-defaults {
		position: absolute;
		bottom: 34px;
		left: 20px;
		z-index: 10;
		padding: 6px 11px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		color: var(--text-secondary);
		font-family: inherit;
		font-size: 11px;
		cursor: pointer;
		box-shadow: var(--elev-1);
	}

	.restore-defaults:hover {
		border-color: var(--accent);
		color: var(--text-accent);
	}
</style>
