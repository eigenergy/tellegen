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
	import { PanelLayout, setPanelLayout } from './panels.svelte.js';
	import { NoticeCenter, setNoticeCenter } from './notices.svelte.js';
	import { CommandRegistry, setCommandRegistry } from './commands.svelte.js';

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

	const layout = setPanelLayout(new PanelLayout());
	setNoticeCenter(new NoticeCenter());
	const commands = setCommandRegistry(new CommandRegistry());
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
</script>

{@render children?.()}
