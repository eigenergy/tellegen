<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import {
		resolveTellegenUiConfig,
		setAppState,
		setController,
		setUiConfig,
		type TellegenUiOptions
	} from './context.svelte.js';
	import { createController } from './controller.svelte.js';
	import { createAppState } from './state.svelte.js';
	import { PanelLayout, setPanelLayout } from './panels.svelte.js';
	import { NoticeCenter, setNoticeCenter } from './notices.svelte.js';

	interface Props extends TellegenUiOptions {
		children?: Snippet;
	}

	let {
		apiBase,
		acOpfWasmUrl,
		loadDefaultCases,
		docsHref,
		orgHref,
		orgLabel,
		showFooter,
		theme,
		cartoApiKey,
		children
	}: Props = $props();

	const config = untrack(() =>
		resolveTellegenUiConfig({
			apiBase,
			acOpfWasmUrl,
			loadDefaultCases,
			docsHref,
			orgHref,
			orgLabel,
			showFooter,
			theme,
			cartoApiKey
		})
	);
	const app = createAppState();
	const ctrl = createController(app, {
		apiBase: config.apiBase,
		acOpfWasmUrl: config.acOpfWasmUrl ?? undefined,
		loadDefaultCases: config.loadDefaultCases
	});
	// A host that opens its own cases never calls load(), so the shell must not
	// sit in its loading state waiting for a fetch that will not come.
	if (!config.loadDefaultCases) ctrl.casesLoaded = true;
	onDestroy(ctrl.dispose);

	setPanelLayout(new PanelLayout());
	setNoticeCenter(new NoticeCenter());
	setUiConfig(config);
	setAppState(app);
	setController(ctrl);

	// The dark tokens in styles.css hang off `:root[data-theme='dark']`. Set it
	// only for dark, and only while mounted, so a light host is untouched. A
	// server-rendered host that wants no light flash can also put the
	// attribute in its app template.
	$effect(() => {
		if (config.theme !== 'dark') return;
		const root = document.documentElement;
		const previous = root.dataset.theme;
		root.dataset.theme = 'dark';
		return () => {
			if (previous === undefined) delete root.dataset.theme;
			else root.dataset.theme = previous;
		};
	});
</script>

{@render children?.()}
