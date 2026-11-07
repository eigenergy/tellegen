<script lang="ts">
	import { onDestroy, onMount, untrack, type Snippet } from 'svelte';
	import {
		resolveCartoApiKey,
		resolveTellegenUiConfig,
		resolveTheme,
		setAppState,
		setController,
		setUiConfig,
		type TellegenUiConfig,
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

	// Read once, at mount: `apiBase`, `acOpfWasmUrl` and `loadDefaultCases`
	// configure the controller this provider owns, and the links and footer are
	// fixed copy. `theme` and `cartoApiKey` stay live, so a host can switch them
	// and the root attribute and the default shell's map follow.
	const config: TellegenUiConfig = {
		...untrack(() =>
			resolveTellegenUiConfig({
				apiBase,
				acOpfWasmUrl,
				loadDefaultCases,
				docsHref,
				orgHref,
				orgLabel,
				showFooter
			})
		),
		get theme() {
			return app.preferences.theme ?? resolveTheme(theme);
		},
		get cartoApiKey() {
			return resolveCartoApiKey(cartoApiKey);
		}
	};
	const app = createAppState();
	onMount(() => {
		app.preferences.initialize(typeof localStorage === 'undefined' ? undefined : localStorage);
		app.displayMode = app.preferences.coloring;
	});
	const ctrl = createController(app, {
		apiBase: config.apiBase,
		acOpfWasmUrl: config.acOpfWasmUrl ?? undefined,
		loadDefaultCases: config.loadDefaultCases
	});
	// A host that opens its own cases never calls load(), so the shell must not
	// sit in its loading state waiting for a fetch that will not come.
	if (!config.loadDefaultCases) ctrl.casesLoaded = true;

	setPanelLayout(new PanelLayout());
	setNoticeCenter(new NoticeCenter());
	setUiConfig(config);
	setAppState(app);
	setController(ctrl);
	// The controller's Studies and multiconductor sessions live in the engine
	// worker; release them with the provider.
	onDestroy(() => ctrl.dispose());

	// Hosts may own the root theme themselves. Only manage the attribute when
	// the host supplied a theme or the user explicitly saved a viewer preference.
	// A server-rendered host can set it before first paint in its app template.
	$effect(() => {
		if (theme === undefined && app.preferences.theme === null) return;
		const root = document.documentElement;
		const previous = root.dataset.theme;
		root.dataset.theme = config.theme;
		return () => {
			if (previous === undefined) delete root.dataset.theme;
			else root.dataset.theme = previous;
		};
	});
</script>

{@render children?.()}
