<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { browserAnalytics } from './client.js';
	// A page view for the first page and each navigation. The client sends
	// nothing off the public hosts, under a browser privacy signal, or after
	// the visitor opted out on the privacy page.
	afterNavigate(() => browserAnalytics()?.page(location.pathname));
</script>
