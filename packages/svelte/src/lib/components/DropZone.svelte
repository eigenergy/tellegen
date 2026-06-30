<script lang="ts">
	import { getAppState } from '../context.svelte.js';

	const app = getAppState();
</script>

{#if app.dragOver}
	<div class="dropzone" aria-hidden="true">
		<div class="dropframe">
			<p class="mono">drop to parse: case files or geographic files</p>
			<p class="mono hint">parsed in your browser; the file never uploads</p>
		</div>
	</div>
{/if}

<style>
	.dropzone {
		position: fixed;
		inset: 0;
		z-index: 20;
		pointer-events: none;
		background: rgba(236, 233, 226, 0.75);
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

	.dropframe .hint {
		font-size: 11px;
		color: var(--ink-dim);
	}
</style>
