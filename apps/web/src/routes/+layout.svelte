<script lang="ts">
	import '@fontsource-variable/bricolage-grotesque';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '../app.css';
	import { setAppState, setController } from '$lib/context.svelte';
	import { createController } from '$lib/controller.svelte';
	import { createAppState } from '$lib/state.svelte';

	let { children } = $props();

	// App-scoped state lives on the persistent root layout, not the home page, so
	// in-browser local cases and committed perturbations survive client-side
	// navigation to /credits and back (the home page remounts; this layout does not).
	// The page and its children read these through getAppState/getController.
	const app = createAppState();
	const ctrl = createController(app);
	setAppState(app);
	setController(ctrl);
</script>

{@render children()}
