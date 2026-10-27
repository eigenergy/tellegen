<script lang="ts">
	import { getController } from '../context.svelte.js';

	const ctrl = getController();

	/** Render in the parent's flow instead of pinned to the bottom of the map. */
	let { inline = false }: { inline?: boolean } = $props();
</script>

<footer class:inline>
	<a href="/credits/">Credits</a>
	<i class="sep"></i>
	<a href="/privacy/">Privacy</a>
	{#if ctrl.showFileDropUi}
		<i class="sep filedrop-ui"></i>
		<span class="drophint filedrop-ui"
			><span class="arrow">&#8675;</span> Drop a case or geographic file anywhere</span
		>
	{/if}
</footer>

<style>
	footer {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		padding: 8px 20px;
		font-size: 10.5px;
		color: var(--text-secondary);
		background: linear-gradient(rgb(var(--bg-rgb) / 0), rgb(var(--bg-rgb) / 0.9));
		animation: rise 0.5s 0.24s ease-out both;
		pointer-events: none;
	}

	footer a {
		pointer-events: auto;
		color: var(--text-secondary);
	}

	footer a:hover {
		color: var(--text-accent);
	}

	.drophint {
		color: var(--text-tertiary);
	}

	.drophint .arrow {
		color: var(--text-accent);
	}

	/* Inside the Network panel on narrow screens, panel text size applies. */
	footer.inline {
		position: static;
		margin-top: 14px;
		padding: 0;
		font-size: 12px;
		background: none;
		animation: none;
		pointer-events: auto;
	}

	@media (hover: none), (pointer: coarse) {
		footer a {
			display: inline-flex;
			align-items: center;
			min-height: 44px;
		}

		footer.inline {
			margin-top: 4px;
		}
	}

	@media (max-width: 760px) {
		footer {
			padding: 7px 10px;
			overflow-x: auto;
			font-size: 9.5px;
			white-space: nowrap;
			pointer-events: auto;
			scrollbar-width: none;
		}

		footer::-webkit-scrollbar {
			display: none;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		footer {
			animation: none;
		}
	}
</style>
