<script lang="ts">
	import TellegenProvider from '../../../../../packages/svelte/src/lib/TellegenProvider.svelte';
	import TellegenShell from '../../../../../packages/svelte/src/lib/TellegenShell.svelte';
	import Host from './GeneratedHandoffHost.svelte';
</script>

<TellegenProvider apiBase={null} loadDefaultCases={false}>
	<TellegenShell />
	<Host />
</TellegenProvider>
