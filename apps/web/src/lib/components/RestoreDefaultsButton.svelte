<script lang="ts">
	import { getController } from '../context.svelte.js';

	const ctrl = getController();
</script>

{#if ctrl.casesLoaded && ctrl.hiddenDefaults.size > 0}
	<button class="restore-defaults mono" onclick={ctrl.restoreDefaultCases}>
		&#8634; restore default cases
	</button>
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
		color: var(--ink-dim);
		font-size: 11px;
		cursor: pointer;
		box-shadow: 0 2px 10px rgba(32, 36, 43, 0.1);
	}

	.restore-defaults:hover {
		border-color: var(--accent);
		color: var(--accent);
	}
</style>
