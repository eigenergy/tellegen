<script lang="ts">
	import { onMount } from 'svelte';
	import { ingestCase } from '@tellegen/engine';
	import {
		getAppState,
		getController
	} from '../../../../../packages/svelte/src/lib/context.svelte.js';
	import { LocalCase } from '../../../../../packages/svelte/src/lib/state.svelte.js';
	const app = getAppState();
	const ctrl = getController();
	let current: LocalCase | null = null;
	onMount(() => {
		window.acOpfLifecycle = {
			available: () => ctrl.acOpfAvailable,
			async start(text) {
				const input = await ingestCase(new TextEncoder().encode(text), 'matpower');
				current = new LocalCase({
					id: 'lifecycle',
					label: 'Lifecycle',
					fileName: 'case.m',
					formulation: 'acopf',
					studyInputJson: input.module_json
				});
				app.addLocal(current);
				ctrl.runSolve(current, null);
			},
			solved: () => current?.solution != null
		};
		ctrl.probeAcOpf();
	});
</script>
