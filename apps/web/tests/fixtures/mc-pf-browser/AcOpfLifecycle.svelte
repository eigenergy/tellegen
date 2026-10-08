<script lang="ts">
	import TellegenProvider from '../../../../../packages/svelte/src/lib/TellegenProvider.svelte';
	import Host from './AcOpfLifecycleHost.svelte';
	import wasmUrl from '../../../../../target/experimental-acopf/tellegen_acopf_wasi.wasm?url';
</script>

<TellegenProvider acOpfWasmUrl={wasmUrl} loadDefaultCases={false}>
	<Host />
</TellegenProvider>
