<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
</script>

<svelte:head
	><title>{page.status === 404 ? 'Page not found' : 'Error'} - tellegen</title></svelte:head
>

<div class="error-page">
	<!-- The app's own top bar, so the page reads as part of tellegen. -->
	<header>
		<a class="brand" href={resolve('/')}>
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
				<path class="brand-stroke" d="M4 18 L12 6 L20 18" stroke-width="1.6" fill="none" />
				<circle class="brand-node" cx="4" cy="18" r="2.4" />
				<circle class="brand-node" cx="12" cy="6" r="2.4" />
				<circle class="brand-node" cx="20" cy="18" r="2.4" />
			</svg>
			<span>tellegen</span>
		</a>
	</header>
	<main>
		<h1>{page.status === 404 ? 'Page not found' : 'Something went wrong'}</h1>
		<a class="home" href={resolve('/')}>Open the map</a>
	</main>
</div>

<style>
	.error-page {
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		color: var(--ink);
		background: var(--paper);
		font-family: var(--font-sans);
	}
	header {
		display: flex;
		align-items: center;
		height: 48px;
		padding: 0 16px;
		background: var(--panel);
		border-bottom: 1px solid var(--line);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--ink);
		font-size: var(--fs-title);
		font-weight: 600;
		text-decoration: none;
	}
	.brand-stroke {
		stroke: var(--ink);
	}
	.brand-node {
		fill: var(--accent);
	}
	main {
		flex: 1;
		display: grid;
		place-content: center;
		justify-items: start;
		gap: 8px;
		padding: 0 16px 48px;
	}
	h1 {
		margin: 0 0 8px;
		font-size: 28px;
		font-weight: 600;
	}
	.home {
		padding: 6px 12px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font-size: var(--fs-body);
		text-decoration: none;
	}
	.home:hover {
		border-color: var(--border-strong);
	}
	a:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
</style>
