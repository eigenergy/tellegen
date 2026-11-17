<script lang="ts">
	import { onDestroy, onMount, untrack, type Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
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
	import { CommandRegistry, setCommandRegistry } from './commands.svelte.js';

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

	// A root theme already in the document before this provider runs belongs to the
	// host's template; leave it alone. Otherwise, with no host or viewer choice, follow
	// the operating system (styles.css covers first paint for the same case).
	const hostRootTheme =
		typeof document === 'undefined' ? undefined : document.documentElement.dataset.theme;
	const systemDark = new MediaQuery('(prefers-color-scheme: dark)', false);

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
			if (app.preferences.theme) return app.preferences.theme;
			if (theme !== undefined) return resolveTheme(theme);
			if (hostRootTheme === 'dark' || hostRootTheme === 'light') return hostRootTheme;
			return systemDark.current ? 'dark' : 'light';
		},
		get cartoApiKey() {
			return resolveCartoApiKey(cartoApiKey);
		}
	};
	const app = createAppState();
	onMount(() => {
		let storage: Storage | undefined;
		try {
			storage = globalThis.localStorage;
		} catch {
			// Browsers may deny access to the storage object itself.
		}
		const previousRevision = app.preferences.revision;
		app.preferences.initialize(storage);
		// Host initialization may already have changed preferences. Only a restored
		// record advances this revision and should replace the host's display mode.
		if (app.preferences.revision > previousRevision) app.displayMode = app.preferences.coloring;
	});
	const ctrl = createController(app, {
		apiBase: config.apiBase,
		acOpfWasmUrl: config.acOpfWasmUrl ?? undefined,
		loadDefaultCases: config.loadDefaultCases
	});
	// A host that opens its own cases never calls load(), so the shell must not
	// sit in its loading state waiting for a fetch that will not come.
	if (!config.loadDefaultCases) ctrl.casesLoaded = true;

	const layout = setPanelLayout(new PanelLayout());
	const commands = setCommandRegistry(new CommandRegistry());
	// A transmission case gets one command per calculation (AppHeader); a distribution
	// case has one calculation, its power flow.
	commands.registerSource(() => {
		const c = app.activeMulti;
		if (ctrl.activeSolvable || !c) return [];
		return [
			{
				id: 'run-calculation',
				title: 'Run power flow',
				keywords: ['solve', 'power flow', 'calculation', 'compute'],
				run: () =>
					void ctrl
						.runSettings(c)
						.catch((error: unknown) =>
							app.reportError(error, { title: 'Power flow failed', stage: 'Solve' })
						)
			}
		];
	});
	commands.register({
		id: 'fit-view',
		title: 'Fit network to view',
		keywords: ['zoom', 'frame', 'center'],
		run: () =>
			void app.requestFrame(app.activeCaseId ?? app.activeLocalId ?? app.activeMultiId ?? 'all')
	});
	commands.register({
		id: 'theme',
		title: () => (config.theme === 'dark' ? 'Use light theme' : 'Use dark theme'),
		keywords: ['dark', 'light', 'appearance', 'color'],
		run: () => app.preferences.update({ theme: config.theme === 'dark' ? 'light' : 'dark' })
	});
	commands.register({
		id: 'zen',
		title: () => (app.zen ? 'Show panels' : 'Hide panels'),
		keywords: ['hide', 'show', 'panels', 'focus', 'present'],
		run: () => (app.zen = !app.zen)
	});
	commands.register({
		id: 'reset-layout',
		title: 'Reset panel layout',
		keywords: ['panels', 'dock', 'float', 'default'],
		run: () => layout.reset()
	});
	setUiConfig(config);
	setAppState(app);
	setController(ctrl);
	// The controller's Studies and multiconductor sessions live in the engine
	// worker; release them with the provider.
	onDestroy(() => ctrl.dispose());

	// Hosts may own the root theme themselves. Leave the attribute to a host whose
	// template set it unless the user explicitly saved a viewer preference; otherwise
	// write the resolved theme (the host's prop, else the operating system's).
	$effect(() => {
		if (theme === undefined && app.preferences.theme === null && hostRootTheme !== undefined)
			return;
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
