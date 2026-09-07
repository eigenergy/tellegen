<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { browserAnalytics } from './client.js';
	onMount(() => {
		browserAnalytics()?.start();
	});
	afterNavigate(() => browserAnalytics()?.page(location.pathname));
</script>
