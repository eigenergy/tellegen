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
	const notices = setNoticeCenter(new NoticeCenter());
	const commands = setCommandRegistry(new CommandRegistry());
	commands.register({
		id: 'run-calculation',
		title: 'Run calculation',
		hint: () => {
			const c = ctrl.activeSolvable ?? app.activeMulti;
			return c ? 'Solve the displayed case with its selected settings' : 'Open a case first';
		},
		keywords: ['solve', 'power flow', 'opf', 'compute'],
		run: () => {
			const c = ctrl.activeSolvable ?? app.activeMulti;
			if (!c) return;
			ctrl.runSettings(c).catch((error: unknown) =>
				notices.push({
					kind: 'error',
					title: 'Calculation failed',
					details: error instanceof Error ? error.message : String(error)
				})
			);
		}
	});
	commands.register({
		id: 'fit-view',
		title: 'Fit network to view',
		hint: 'Reframe the map on the displayed case',
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
		title: () => (app.zen ? 'Exit zen mode' : 'Enter zen mode'),
		hint: () => (app.zen ? 'Bring the panels back' : 'Hide the panels and focus on the map'),
		keywords: ['focus', 'hide panels', 'distraction'],
		run: () => (app.zen = !app.zen)
	});
	commands.register({
		id: 'reset-layout',
		title: 'Reset panel layout',
		hint: 'Put detached or redocked panels back in their default docks',
		keywords: ['panels', 'dock', 'float', 'default'],
		run: () => layout.reset()
	});
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
