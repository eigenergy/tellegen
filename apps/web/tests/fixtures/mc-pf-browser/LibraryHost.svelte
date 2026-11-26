<script lang="ts">
	import { TellegenProvider } from '@tellegen/svelte';
</script>

<a href="#host" data-testid="host-link">Host link</a>
<input aria-label="Host input" />
<TellegenProvider theme="light">
	<a href="#light" data-testid="light-link">Light app</a>
</TellegenProvider>
<TellegenProvider theme="dark">
	<a href="#dark" data-testid="dark-link">Dark app</a>
</TellegenProvider>
