<script lang="ts">
	import { getAppState } from '../context.svelte.js';

	const app = getAppState();
</script>

{#if app.placingId}
	<div class="placement-cue mono">click the map to place the topology layout</div>
{/if}

<style>
	.placement-cue {
		position: absolute;
		left: 50%;
		bottom: 52px;
		z-index: 14;
		transform: translateX(-50%);
		padding: 8px 12px;
		background: var(--panel);
		border: 1px solid var(--accent);
		border-radius: 3px;
		color: var(--text-accent);
		font-size: 11px;
		box-shadow: 0 4px 18px rgba(32, 36, 43, 0.1);
		pointer-events: none;
	}

	@media (max-width: 420px) {
		.placement-cue {
			bottom: 38px;
			width: calc(100% - 28px);
			text-align: center;
		}
	}
</style>
