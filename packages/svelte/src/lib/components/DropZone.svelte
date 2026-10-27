<script lang="ts">
	import type { Snippet } from 'svelte';
	import { getAppState } from '../context.svelte.js';

	/** Optional host content rendered under the drop prompt. */
	let { children }: { children?: Snippet } = $props();

	const app = getAppState();
</script>

{#if app.dragOver}
	<div class="dropzone" aria-hidden="true">
		<div class="dropframe">
			<p>Drop case or geographic files</p>
			{#if children}<div class="note">{@render children()}</div>{/if}
		</div>
	</div>
{/if}

<style>
	.dropzone {
		position: fixed;
		inset: 0;
		z-index: 20;
		pointer-events: none;
		background: rgb(var(--bg-rgb) / 0.75);
	}

	.dropframe {
		position: absolute;
		inset: 14px;
		border: 1.5px dashed var(--accent);
		border-radius: 3px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}

	.dropframe p {
		margin: 0;
		font-size: 13px;
	}

	.note {
		font-size: 11px;
		color: var(--text-secondary);
	}
</style>
