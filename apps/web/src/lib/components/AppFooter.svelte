<script lang="ts">
	import { getController } from '$lib/context.svelte';

	const ctrl = getController();
</script>

<footer class="mono">
	<a href="/credits/">credits</a>
	<i class="sep"></i>
	<a href="/privacy/">privacy</a>
	{#if ctrl.showFileDropUi}
		<i class="sep filedrop-ui"></i>
		<span class="drophint filedrop-ui"
			><span class="arrow">&#8675;</span> drop a case or coordinate file anywhere</span
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
		color: var(--ink-dim);
		background: linear-gradient(rgba(236, 233, 226, 0), rgba(236, 233, 226, 0.9));
		animation: rise 0.5s 0.24s ease-out both;
		pointer-events: none;
	}

	footer a {
		pointer-events: auto;
		color: var(--ink-dim);
	}

	footer a:hover {
		color: var(--accent);
	}

	.drophint {
		color: var(--ink-faint);
	}

	.drophint .arrow {
		color: var(--accent);
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
