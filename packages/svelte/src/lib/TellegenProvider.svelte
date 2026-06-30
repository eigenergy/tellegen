<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import {
		resolveTellegenUiConfig,
		setAppState,
		setController,
		setUiConfig,
		type TellegenUiOptions
	} from './context.svelte.js';
	import { createController } from './controller.svelte.js';
	import { createAppState } from './state.svelte.js';

	interface Props extends TellegenUiOptions {
		children?: Snippet;
	}

	let { apiBase, loadDefaultCases, docsHref, orgHref, orgLabel, showFooter, children }: Props =
		$props();

	const config = untrack(() =>
		resolveTellegenUiConfig({
			apiBase,
			loadDefaultCases,
			docsHref,
			orgHref,
			orgLabel,
			showFooter
		})
	);
	const app = createAppState();
	const ctrl = createController(app, { apiBase: config.apiBase });

	setUiConfig(config);
	setAppState(app);
	setController(ctrl);
</script>

{@render children?.()}
