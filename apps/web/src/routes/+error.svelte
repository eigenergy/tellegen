<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
</script>

<svelte:head
	><title>{page.status === 404 ? 'Page not found' : 'Error'} - tellegen</title></svelte:head
>

<main class="error-page">
	<h1>{page.status === 404 ? 'Page not found' : 'Something went wrong'}</h1>
	<p class="status">{page.status}</p>
	<a href={resolve('/')}>Back to the map</a>
</main>

<style>
	.error-page {
		display: grid;
		place-content: center;
		justify-items: start;
		gap: 12px;
		min-height: 100dvh;
		padding: 0 16px;
		color: var(--ink);
		background: var(--bg);
		font-family: var(--font-sans);
	}
	h1 {
		margin: 0;
		font-size: 28px;
		font-weight: 600;
	}
	.status {
		margin: 0;
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}
	a {
		color: var(--text-accent);
		font-size: var(--fs-body);
	}
</style>
