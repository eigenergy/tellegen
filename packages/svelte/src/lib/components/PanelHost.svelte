<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { getAppState, getUiConfig } from '../context.svelte.js';
	import { getPanelLayout, sheetInset } from '../panels.svelte.js';
	import PanelCard from './PanelCard.svelte';
	import { getCommandRegistry } from '../commands.svelte.js';

	// The dock insets come from `--tellegen-dock-left/right/bottom` on an
	// ancestor (defaults 16px, 16px and 100px; 12px at the sides on a phone),
	// so a custom shell with its own rail or status bar can move the docks
	// without restyling them.
	let { showToolbar = true }: { showToolbar?: boolean } = $props();

	const app = getAppState();
	const config = getUiConfig();
	const layout = getPanelLayout();
	const commands = getCommandRegistry();
	// Each registered panel, including host panels such as the Agent panel, is
	// also a palette command.
	onDestroy(
		commands.registerSource(() =>
			layout.panels.map((panel) => {
				// A panel open behind another one is not on screen, so the
				// command offers to show it rather than to close it.
				const open = layout.shown(panel);
				return {
					id: `panel:${panel.id}`,
					title: `${open ? 'Close' : 'Open'} ${panel.title} panel`,
					keywords: ['panel', 'show', 'hide'],
					group: 'Panels',
					run: () => {
						if (app.zen) app.zen = false;
						layout.toggle(panel);
					}
				};
			})
		)
	);
	/** Phone with the sheet open: the sheet runs to the bottom edge, and the map's credit
	 * and the notices sit on the map above it. */
	const sheetOpen = $derived(layout.compact && !!layout.drawer && !app.zen);
	// An open sheet stands on the dynamics playback bar, which then sits flush on the bottom edge.
	const bottomReserve = $derived(sheetOpen ? app.dockInset : 88);
	/** The tab strip: the panels that dock on the left by default, plus any that
	 * ask for a tab (Dynamics). Settings opens from the header menu instead. */
	const tabs = $derived(layout.ordered().filter((p) => p.tab ?? p.side === 'left'));
	/** The tabs of the panel under the strip. A panel that docks elsewhere (Dynamics, on the
	 * right) opens from the top bar on a wide screen, so the strip has one selected tab. */
	const dockTabs = $derived(tabs.filter((p) => layout.compact || layout.position(p) === 'left'));
	/** A panel floated over the map keeps its tab, set apart after a divider. */
	const floatTabs = $derived(
		tabs.filter((p) => !layout.compact && layout.position(p) === 'floating')
	);
	const left = $derived(layout.ordered('left').filter((p) => p.open));
	const right = $derived(layout.ordered('right').filter((p) => p.open));
	const floating = $derived(
		layout.ordered().filter((p) => p.open && layout.position(p) === 'floating')
	);
	const drawer = $derived(layout.panels.find((p) => p.id === layout.drawer));
	/** Phone: a panel with no tab of its own (Settings, from the menu) is open in the
	 * sheet. The strip then names it and offers a way back to the tabs. */
	const away = $derived(layout.compact && !!drawer && !dockTabs.some((p) => p.id === drawer.id));
	/** The last tab shown in the sheet: where Back returns. */
	let lastTab = $state('network');
	$effect(() => {
		const id = drawer?.id;
		if (id && dockTabs.some((p) => p.id === id)) lastTab = id;
	});
	/** Height of the tab strip, which the panel below it continues. */
	let stripHeight = $state(36);
	const toolbarTop = $derived(app.headerInset + 8);
	const stripShown = $derived(showToolbar && !app.zen && dockTabs.length + floatTabs.length > 0);
	const panelTop = $derived(toolbarTop + (stripShown && !layout.compact ? stripHeight : 0));
	const leftShown = $derived(left.some((p) => layout.preferred(p)));
	/** The panel under the strip, whose controls may share the strip's row. */
	const front = $derived(layout.compact ? drawer : left.find((p) => layout.preferred(p)));
	/** Room on the strip's row for the front panel's own controls. */
	const STRIP_CONTROLS = 68;
	const reserve = $derived(
		front && !front.headerActions ? (layout.compact ? 52 : STRIP_CONTROLS) : 0
	);
	/** The tabs' natural width; the left dock grows to fit them and the controls. */
	let tabsWidth = $state(0);
	$effect(() => {
		layout.stripMin = stripShown && !layout.compact ? tabsWidth + STRIP_CONTROLS + 10 : 0;
	});
	/** The shortest the phone sheet is while open. */
	const SHEET_MIN = 140;
	/** The tallest the phone sheet may be: up to the top bar. */
	const sheetMax = $derived(
		Math.max(1, layout.viewportHeight - toolbarTop - bottomReserve - stripHeight - 8)
	);
	/** A height the reader dragged the sheet to; null keeps the default. */
	let sheetSize = $state<number | null>(null);
	/** The open panel's own height in the sheet, so a short panel gets a short sheet. */
	let natural = $state<number | null>(null);
	let drawerEl = $state<HTMLElement | null>(null);
	const drawerHeight = $derived(
		Math.max(
			1,
			Math.min(
				sheetMax,
				sheetSize ??
					Math.min(
						layout.viewportHeight * 0.42,
						sheetMax * 0.7,
						natural === null ? Infinity : Math.max(96, natural)
					)
			)
		)
	);
	// Measure the panel's content (not its scroll box, which fills the sheet) whenever the
	// content or its children change.
	$effect(() => {
		const el = drawerEl;
		const id = drawer?.id;
		if (!el || !id) {
			natural = null;
			return;
		}
		const content = el.querySelector<HTMLElement>('.panel-content');
		if (!content) return;
		const measure = () => {
			const kids = [...content.children] as HTMLElement[];
			if (!kids.length) return;
			const first = kids[0].getBoundingClientRect();
			const last = kids[kids.length - 1].getBoundingClientRect();
			const style = getComputedStyle(content);
			const margins =
				parseFloat(getComputedStyle(kids[0]).marginTop) +
				parseFloat(getComputedStyle(kids[kids.length - 1]).marginBottom);
			natural = Math.ceil(
				last.bottom -
					first.top +
					margins +
					parseFloat(style.paddingTop) +
					parseFloat(style.paddingBottom)
			);
		};
		const sizes = new ResizeObserver(measure);
		const watch = () => {
			sizes.disconnect();
			for (const kid of content.children) sizes.observe(kid);
			measure();
		};
		const children = new MutationObserver(watch);
		children.observe(content, { childList: true });
		watch();
		return () => {
			sizes.disconnect();
			children.disconnect();
		};
	});
	// The sheet's handle: drag to resize, drag down to close, or tap to switch
	// between a low and a tall sheet. Arrow keys do the same from the keyboard.
	let drag: { y: number; height: number; moved: boolean } | null = null;
	function setSheet(height: number) {
		if (height < 90) {
			layout.drawer = null;
			sheetSize = null;
			return;
		}
		if (!layout.drawer) layout.drawer = layout.lastActive ?? 'network';
		sheetSize = Math.max(SHEET_MIN, Math.min(sheetMax, height));
	}
	function handleDown(event: PointerEvent) {
		if (!event.isPrimary) return;
		drag = { y: event.clientY, height: drawer ? drawerHeight : 0, moved: false };
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}
	function handleMove(event: PointerEvent) {
		if (!drag) return;
		const dy = drag.y - event.clientY;
		if (!drag.moved && Math.abs(dy) < 4) return;
		drag.moved = true;
		setSheet(drag.height + dy);
	}
	function handleUp() {
		const moved = drag?.moved;
		drag = null;
		if (moved) return;
		// A tap: open a closed sheet, else switch between low and tall.
		if (!drawer) setSheet(sheetMax * 0.42);
		else setSheet(drawerHeight > sheetMax * 0.6 ? sheetMax * 0.42 : sheetMax * 0.9);
	}
	function handleKey(event: KeyboardEvent) {
		const step = event.key === 'ArrowUp' ? 60 : event.key === 'ArrowDown' ? -60 : 0;
		if (!step) return;
		event.preventDefault();
		setSheet((drawer ? drawerHeight : 0) + step);
	}
	/** Compact with no panel open: the tab strip sits on the bottom edge (unless the
	 * dynamics playback dock is there), and the map's bottom controls clear it. */
	const docked = $derived(layout.compact && stripShown && !drawer && app.dockInset === 0);
	/** The sheet's grab bar above the strip: map labels must clear it too. */
	const HANDLE_HEIGHT = 22;
	/** Compact: the strip sits on top of the sheet, or on the bottom edge when no
	 * panel is open. */
	const stripBottom = $derived(drawer ? bottomReserve + drawerHeight : docked ? 0 : bottomReserve);
	function resize() {
		layout.resizeViewport(window.innerWidth, window.innerHeight, panelTop);
	}
	onMount(() => {
		resize();
		try {
			layout.initialize(window.localStorage);
		} catch {
			layout.initialize();
		}
	});
	$effect(() => {
		const top = panelTop;
		const bottom = bottomReserve;
		untrack(() => {
			layout.bottom = bottom;
			layout.resizeViewport(window.innerWidth, window.innerHeight, top);
		});
	});
	// With the strip hidden, a panel's overlay header still sits above the sheet.
	const inset = $derived(
		sheetInset({
			active: !app.zen && layout.compact,
			stripShown,
			stripBottom,
			stripHeight,
			handleHeight: HANDLE_HEIGHT,
			open: !!drawer,
			headerAbove: !!drawer && !drawer.headerActions
		})
	);
	$effect(() => {
		app.sheetInset = inset;
	});
	// The main map's attribution and zoom, and the notices, sit above the strip or the
	// open sheet.
	$effect(() => {
		const root = config.element?.style;
		if (!root) return;
		if ((docked || (sheetOpen && stripShown)) && !app.zen)
			root.setProperty(
				'--tellegen-sheet-clear',
				`${(docked ? 0 : stripBottom) + stripHeight + HANDLE_HEIGHT + 4}px`
			);
		else root.removeProperty('--tellegen-sheet-clear');
		return () => root.removeProperty('--tellegen-sheet-clear');
	});
</script>

<svelte:window
	onresize={resize}
	onpointermove={(event) => layout.moveGesture(event.clientX, event.clientY)}
	onpointerup={() => layout.finishGesture()}
	onpointercancel={() => layout.finishGesture()}
/>
{#if stripShown && layout.compact}
	<button
		class="sheet-handle"
		style:bottom="{stripBottom + stripHeight}px"
		aria-label={drawer ? 'Resize the panel' : 'Open the panel'}
		title="Drag to resize"
		onpointerdown={handleDown}
		onpointermove={handleMove}
		onpointerup={handleUp}
		onpointercancel={() => (drag = null)}
		onkeydown={handleKey}><span aria-hidden="true"></span></button
	>
{/if}
{#if stripShown}
	<div
		class="panel-toolbar"
		class:compact={layout.compact}
		class:docked
		class:attached={layout.compact ? !!drawer : leftShown}
		style:top={layout.compact ? undefined : `${toolbarTop}px`}
		style:bottom={layout.compact ? `${stripBottom}px` : undefined}
		style:width={!layout.compact && leftShown ? `${layout.dockWidth('left')}px` : undefined}
		style:padding-right={reserve ? `${reserve}px` : undefined}
		role="group"
		aria-label="Panels"
		bind:offsetHeight={stripHeight}
	>
		{#if away && drawer}
			<div class="away">
				<button class="back" onclick={() => layout.show(lastTab)}
					><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
						><path
							d="M7.5 2.5 4 6l3.5 3.5"
							stroke="currentColor"
							stroke-width="1.5"
							fill="none"
						/></svg
					>Back</button
				>
				<h2>{drawer.title}</h2>
			</div>
		{:else}
			<div class="tabs" bind:clientWidth={tabsWidth}>
				{#each dockTabs as panel (panel.id)}
					<button
						aria-label={panel.badge ? `${panel.title}, ${panel.badge} new` : panel.title}
						aria-pressed={layout.shown(panel)}
						onclick={() => {
							if (app.zen) app.zen = false;
							layout.toggle(panel);
						}}
						>{panel.title}{#if panel.badge}<span class="badge">({panel.badge})</span>{/if}</button
					>
				{/each}
				{#if floatTabs.length}<span class="divider" aria-hidden="true"></span>{/if}
				{#each floatTabs as panel (panel.id)}
					<button
						aria-label={panel.title}
						aria-pressed={layout.shown(panel)}
						onclick={() => {
							if (app.zen) app.zen = false;
							layout.toggle(panel);
						}}>{panel.title}</button
					>
				{/each}
			</div>
		{/if}
	</div>
{/if}
{#if !app.zen}
	{#if layout.storageError}<p class="layout-error" role="status" style:top="{panelTop}px">
			{layout.storageError}
		</p>{/if}
	<!-- The docks and the drawer take focus (tabindex -1) when a closed panel has
	 no toolbar button or opener to hand it back to. The drawer stays rendered,
	 at zero height, while no panel is open in it. A dock shows one panel at a
	 time; the others stay mounted, hidden, so they keep their state. -->
	{#if layout.compact}
		<div
			class="drawer"
			bind:this={drawerEl}
			data-panel-drawer
			role="region"
			aria-label="Panels"
			tabindex="-1"
			style:bottom="{bottomReserve}px"
			style:height="{drawer ? drawerHeight : 0}px"
			style:--tellegen-strip-height="{stripHeight}px"
		>
			{#if drawer}<PanelCard panel={drawer} mobile />{/if}
		</div>
	{:else}
		<div
			class="dock left"
			class:attached={stripShown}
			style:bottom="max(var(--tellegen-dock-bottom, 100px), {bottomReserve}px)"
			data-panel-dock="left"
			role="region"
			aria-label="Left panels"
			tabindex="-1"
			style:top="{panelTop}px"
			style:width="{layout.dockWidth('left')}px"
			style:--tellegen-strip-height="{stripHeight}px"
		>
			{#each left as panel (panel.id)}<PanelCard
					{panel}
					attached={stripShown}
					concealed={!layout.preferred(panel)}
				/>{/each}
		</div>
		<div
			class="dock right"
			style:bottom="max(var(--tellegen-dock-bottom, 100px), {bottomReserve}px)"
			data-panel-dock="right"
			role="region"
			aria-label="Right panels"
			tabindex="-1"
			style:top="{toolbarTop}px"
			style:width="{layout.dockWidth('right')}px"
		>
			{#each right as panel (panel.id)}<PanelCard
					{panel}
					concealed={!layout.preferred(panel)}
				/>{/each}
		</div>
		{#each floating as panel (panel.id)}<PanelCard {panel} floating />{/each}
	{/if}
{/if}

<style>
	/* The tabs and the panel under them read as one surface: the strip's lower
	   edge is the panel's upper edge. */
	.panel-toolbar {
		position: fixed;
		/* Under the docks, so a panel's own controls can sit on the strip's row. */
		z-index: var(--z-panel-strip);
		left: var(--tellegen-dock-left, 16px);
		display: flex;
		align-items: stretch;
		gap: 0;
		height: 36px;
		max-width: calc(100vw - 32px);
		padding: 0 4px;
		overflow-x: auto;
		scrollbar-width: none;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		box-shadow: var(--elev-1);
	}
	.tabs {
		display: inline-flex;
		align-items: stretch;
		flex: none;
	}
	.panel-toolbar::-webkit-scrollbar {
		display: none;
	}
	.panel-toolbar.attached {
		border-bottom-color: var(--line);
		border-radius: var(--radius-panel) var(--radius-panel) 0 0;
		box-shadow: none;
	}
	.panel-toolbar.compact {
		left: var(--tellegen-dock-left, 12px);
		right: var(--tellegen-dock-right, 12px);
		max-width: none;
	}
	/* Closed sheet: the tabs are a bar on the bottom edge, not a card floating
	   above dead map. */
	.panel-toolbar.compact.docked {
		left: 0;
		right: 0;
		border-width: 1px 0 0;
		border-radius: 0;
		box-shadow: none;
	}
	/* Open sheet: edge to edge, so no sliver of map shows beside it. */
	.panel-toolbar.compact.attached {
		left: 0;
		right: 0;
		background: linear-gradient(var(--panel), var(--panel));
		border-width: 1px 0;
		border-radius: var(--radius-panel) var(--radius-panel) 0 0;
		border-bottom-color: var(--line);
		box-shadow: none;
	}
	button {
		flex: none;
		position: relative;
		border: 0;
		background: none;
		color: var(--text-secondary);
		font: 500 var(--fs-body) / 1 var(--font-sans);
		padding: 0 10px;
		cursor: pointer;
	}
	/* Hover only where there is a pointer: on a touch screen the last tapped tab keeps its
	   hover, and would read as open beside the open tab. */
	@media (hover: hover) {
		button:hover {
			color: var(--ink);
		}
	}
	.badge {
		margin-left: 4px;
		color: var(--text-secondary);
		font-weight: 400;
	}
	button[aria-pressed='true'] {
		color: var(--ink);
	}
	.away {
		display: flex;
		align-items: stretch;
		gap: 4px;
	}
	.away h2 {
		align-self: center;
		margin: 0 0 0 6px;
		font: 600 var(--fs-title) / 1 var(--font-sans);
		color: var(--ink);
	}
	button.back {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--text-accent);
	}
	/* The open tab is underlined in the accent, the one place it appears here. */
	button[aria-pressed='true']::after {
		content: '';
		position: absolute;
		left: 10px;
		right: 10px;
		bottom: -1px;
		height: 2px;
		background: var(--accent);
	}
	/* The strip clips its overflow, so on a phone the underline sits inside its lower edge. */
	.panel-toolbar.compact button[aria-pressed='true']::after {
		bottom: 0;
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -4px;
		border-radius: var(--radius-control);
	}
	.divider {
		align-self: center;
		width: 1px;
		height: 18px;
		margin: 0 6px;
		background: var(--line);
	}
	.dock {
		position: fixed;
		z-index: var(--z-panel);
		bottom: var(--tellegen-dock-bottom, 100px);
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 12px;
		pointer-events: none;
	}
	/* A dock or the sheet takes focus only to hold it after its panel closes: it is not a
	   control, so it draws no focus ring. */
	.dock:focus,
	.drawer:focus {
		outline: none;
	}
	.dock.left {
		left: var(--tellegen-dock-left, 16px);
	}
	.dock.right {
		right: var(--tellegen-dock-right, 16px);
	}
	.drawer {
		position: fixed;
		bottom: 0;
		left: 0;
		right: 0;
		z-index: var(--z-panel);
	}
	/* The phone sheet's grab bar, just above its tabs. */
	.sheet-handle {
		position: fixed;
		z-index: var(--z-dock);
		left: 50%;
		transform: translateX(-50%);
		display: grid;
		place-items: center;
		width: 88px;
		height: 22px;
		min-height: 0;
		padding: 0;
		touch-action: none;
		cursor: ns-resize;
	}
	.sheet-handle span {
		width: 40px;
		height: 5px;
		border-radius: 3px;
		background: var(--border-strong);
		box-shadow: 0 0 0 1px rgb(var(--paper-rgb) / 0.8);
	}
	.sheet-handle:focus-visible {
		outline-offset: -2px;
	}
	.layout-error {
		position: fixed;
		left: 50%;
		transform: translateX(-50%);
		z-index: var(--z-notice);
		max-width: 400px;
		margin: 0;
		padding: 8px 12px;
		font: var(--fs-label) / 1.5 var(--font-sans);
		color: var(--ink);
		background: var(--raised);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
	}
	@media (pointer: coarse) {
		button {
			min-height: 44px;
		}
		/* The 44 px tabs plus the strip's borders, so the open tab's underline is not clipped. */
		.panel-toolbar {
			height: 46px;
		}
	}
</style>
