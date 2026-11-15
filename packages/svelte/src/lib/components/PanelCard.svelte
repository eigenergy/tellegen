<script lang="ts">
	import { tick } from 'svelte';
	import {
		focusFirst,
		getPanelLayout,
		type PanelRegistration,
		type PanelSide
	} from '../panels.svelte.js';
	let {
		panel,
		floating = false,
		mobile = false,
		attached = false,
		concealed = false
	}: {
		panel: PanelRegistration;
		floating?: boolean;
		mobile?: boolean;
		/** Docked under the tab strip, which names the panel. */
		attached?: boolean;
		/** Open but behind another panel in its dock: kept mounted, not shown. */
		concealed?: boolean;
	} = $props();
	const layout = getPanelLayout();
	let menu = $state(false);
	let menuAnchor: HTMLDivElement | undefined = $state();
	let card: HTMLElement;
	const bounds = $derived(layout.bounds(panel));
	const preferred = $derived(!floating && !mobile && layout.preferred(panel));
	/** The tab strip names a docked panel; only a floating one shows its title,
	 * which is also the handle that moves it. */
	const named = $derived(floating || (!mobile && !attached));
	/** Under the tab strip with nothing of its own to show in a header row: the
	 * panel's controls sit at the strip's right end instead. */
	const overlay = $derived((attached || mobile) && !panel.headerActions);
	function start(event: PointerEvent, kind: 'move' | 'resize') {
		if (mobile || !event.isPrimary || event.button !== 0) return;
		const rect = card.getBoundingClientRect();
		layout.beginGesture(panel, kind, event.clientX, event.clientY, {
			x: rect.x,
			y: rect.y,
			width: rect.width,
			height: rect.height
		});
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
		event.preventDefault();
	}
	async function key(event: KeyboardEvent) {
		if (mobile) return;
		const panelId = panel.id;
		const step = event.shiftKey ? 40 : 10;
		const directions: Record<string, [number, number]> = {
			ArrowLeft: [-step, 0],
			ArrowRight: [step, 0],
			ArrowUp: [0, -step],
			ArrowDown: [0, step]
		};
		if (directions[event.key]) {
			event.preventDefault();
			const [x, y] = directions[event.key];
			const rect = card.getBoundingClientRect();
			layout.float(panel, {
				x: rect.x + x,
				y: rect.y + y,
				width: rect.width,
				height: rect.height
			});
			layout.save();
			await tick();
			document.querySelector<HTMLElement>(`[data-panel="${CSS.escape(panelId)}"] .title`)?.focus();
		} else if (event.key === 'Escape') {
			menu = false;
		}
	}
	function resizeKey(event: KeyboardEvent) {
		const step = event.shiftKey ? 40 : 10;
		if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
		event.preventDefault();
		layout.float(panel, {
			...bounds,
			width:
				bounds.width + (event.key === 'ArrowRight' ? step : event.key === 'ArrowLeft' ? -step : 0),
			height:
				bounds.height + (event.key === 'ArrowDown' ? step : event.key === 'ArrowUp' ? -step : 0)
		});
		layout.save();
	}
	async function position(side?: PanelSide) {
		const panelId = panel.id;
		menu = false;
		if (side) layout.dock(panel, side);
		else {
			layout.float(panel);
			layout.save();
		}
		await tick();
		document.querySelector<HTMLElement>(`[data-panel="${CSS.escape(panelId)}"] .title`)?.focus();
	}
	async function close() {
		const title = panel.title;
		const position = layout.position(panel);
		const dock = position === 'floating' ? panel.side : position;
		const opener = layout.opener(panel);
		layout.close(panel);
		await tick();
		// The toolbar button for the panel; a host without the toolbar gets the
		// control that opened it, else the panel's dock (the drawer when narrow).
		focusFirst([
			document.querySelector<HTMLElement>(
				`.panel-toolbar button[aria-label="${CSS.escape(title)}"]`
			),
			opener,
			document.querySelector<HTMLElement>(`[data-panel-dock="${dock}"], [data-panel-drawer]`)
		]);
	}
	// Keyboard users can scroll an overflowing panel only through focus, so the
	// content becomes a labelled, focusable region exactly while it overflows.
	let overflowing = $state(false);
	function trackOverflow(content: HTMLElement) {
		const sync = () => (overflowing = content.scrollHeight > content.clientHeight + 1);
		const resize = new ResizeObserver(sync);
		const observe = () => {
			resize.disconnect();
			resize.observe(content);
			for (const child of content.children) resize.observe(child);
			sync();
		};
		const children = new MutationObserver(observe);
		children.observe(content, { childList: true });
		observe();
		return () => {
			resize.disconnect();
			children.disconnect();
		};
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') menu = false;
	}}
	onpointerdown={(event) => {
		if (menu && !menuAnchor?.contains(event.target as Node)) menu = false;
	}}
/>
<aside
	bind:this={card}
	class="panel-frame"
	class:floating
	class:mobile
	class:preferred
	class:attached
	class:overlay
	hidden={concealed}
	data-panel={panel.id}
	aria-label={panel.title}
	style:left={floating ? `${bounds.x}px` : undefined}
	style:top={floating ? `${bounds.y}px` : undefined}
	style:width={floating ? `${bounds.width}px` : undefined}
	style:height={floating ? `${bounds.height}px` : undefined}
>
	<header>
		{#if mobile}
			<h2 class="title sr-only">{panel.title}</h2>
			<span class="spacer"></span>
		{:else if named}
			<button
				class="title"
				aria-label={`Move ${panel.title} panel`}
				title="Drag or use arrow keys to move"
				onpointerdown={(event) => start(event, 'move')}
				onkeydown={key}
				onclick={() => layout.activate(panel)}>{panel.title}</button
			>
		{:else}
			<span class="spacer"></span>
		{/if}
		<div class="header-actions">{@render panel.headerActions?.()}</div>
		{#if !mobile}<div class="menu-anchor" bind:this={menuAnchor}>
				<button
					class="icon"
					aria-label={`${panel.title} panel options`}
					aria-expanded={menu}
					onclick={() => (menu = !menu)}
					title="Panel options"
				>
					<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"
						><circle cx="3" cy="8" r="1" /><circle cx="8" cy="8" r="1" /><circle
							cx="13"
							cy="8"
							r="1"
						/></svg
					>
				</button>
				{#if menu}<div class="panel-menu">
						<button onclick={() => position('left')}>Dock left</button>
						<button onclick={() => position('right')}>Dock right</button>
						<button onclick={() => position()}>Float over the map</button>
					</div>{/if}
			</div>{/if}
		<button
			class="icon close"
			aria-label={`Close ${panel.title.toLowerCase()} panel`}
			title="Close panel"
			onclick={close}
		>
			<svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"
				><path d="m4 4 8 8m0-8-8 8" stroke="currentColor" stroke-width="1.5" fill="none" /></svg
			>
		</button>
	</header>
	<!-- A scroll container must take focus for keyboard scrolling, and this one
		 takes it only while it overflows. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div
		class="panel-content"
		{@attach trackOverflow}
		tabindex={overflowing ? 0 : undefined}
		role={overflowing ? 'region' : undefined}
		aria-label={overflowing ? `${panel.title} panel content` : undefined}
	>
		{@render panel.children()}
	</div>
	{#if floating}<button
			class="resize"
			aria-label={`Resize ${panel.title} panel`}
			title="Drag to resize, or use arrow keys"
			onpointerdown={(event) => start(event, 'resize')}
			onkeydown={resizeKey}
		>
			<svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
				><path d="m3 10 7-7m-3 7 3-3" stroke="currentColor" /></svg
			>
		</button>{/if}
</aside>

<style>
	.panel-frame {
		display: flex;
		flex-direction: column;
		min-height: 130px;
		max-height: 100%;
		flex: 0 1 auto;
		pointer-events: auto;
		background: var(--panel);
		color: var(--ink);
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		box-shadow: var(--elev-1);
		font: var(--fs-body) / 1.5 var(--font-sans);
		position: relative;
	}
	.panel-frame[hidden] {
		display: none;
	}
	/* Continues the tab strip above it. */
	.attached {
		border-top: 0;
		border-radius: 0 0 var(--radius-panel) var(--radius-panel);
	}
	header {
		display: flex;
		align-items: center;
		gap: 4px;
		min-height: 36px;
		padding: 4px 6px 4px 12px;
		flex: none;
		border-bottom: 1px solid var(--line);
	}
	.attached header {
		padding-left: 4px;
	}
	.overlay > header {
		position: absolute;
		z-index: 1;
		top: calc(-1 * var(--tellegen-strip-height, 36px));
		right: 0;
		height: var(--tellegen-strip-height, 36px);
		min-height: 0;
		padding: 0 4px;
		border: 0;
		background: none;
	}
	.overlay > header .spacer {
		display: none;
	}
	button {
		color: inherit;
		font: inherit;
		cursor: pointer;
		border: 0;
		background: transparent;
		border-radius: var(--radius-control);
	}
	button:hover {
		background: var(--surface-hover);
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.title {
		flex: 1;
		min-width: 0;
		text-align: left;
		font-size: var(--fs-title);
		font-weight: 600;
		margin: 0;
		padding: 2px 0;
		cursor: grab;
		touch-action: none;
	}
	.title:active {
		cursor: grabbing;
	}
	.spacer {
		flex: 1;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		padding: 0;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.icon {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		padding: 0;
		flex: none;
		color: var(--text-secondary);
	}
	.icon:hover {
		color: var(--ink);
	}
	.icon svg {
		fill: currentColor;
	}
	/* Coarse pointers: a 44px hit area around the close button. The
	   pseudo-element reaches into the header padding and the gap, so nothing
	   moves. */
	@media (pointer: coarse) {
		.close {
			position: relative;
		}
		.close::after {
			content: '';
			position: absolute;
			inset: -8px -10px -8px -8px;
		}
	}
	.header-actions {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
	}
	.header-actions :global(button) {
		font: var(--fs-label) / 1.3 var(--font-sans);
		padding: 4px 6px;
		border: 0;
		background: none;
		color: var(--text-accent);
	}
	.panel-content {
		min-height: 0;
		overflow: auto;
		overscroll-behavior: contain;
		display: flex;
		flex-direction: column;
		border-radius: 0 0 var(--radius-panel) var(--radius-panel);
	}
	.panel-content:focus-visible {
		outline-offset: -2px;
	}
	.menu-anchor {
		position: relative;
	}
	.panel-menu {
		position: absolute;
		z-index: 3;
		top: 100%;
		right: 0;
		min-width: 160px;
		padding: 4px;
		background: var(--surface-menu);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		box-shadow: var(--elev-menu);
	}
	.panel-menu button {
		display: block;
		padding: 6px 10px;
		width: 100%;
		text-align: left;
		white-space: nowrap;
		font-size: var(--fs-body);
	}
	.preferred {
		flex-shrink: 0;
	}
	.floating {
		position: fixed;
		min-height: 0;
		z-index: 40;
		max-height: none;
	}
	.floating .panel-content {
		flex: 1;
		padding-bottom: 12px;
	}
	.resize {
		position: absolute;
		bottom: 0;
		right: 0;
		width: 20px;
		height: 20px;
		cursor: nwse-resize;
		touch-action: none;
		color: var(--text-secondary);
	}
	.mobile {
		min-height: 0;
		height: 100%;
		border-top: 0;
		border-radius: 0 0 var(--radius-panel) var(--radius-panel);
		box-shadow: none;
	}
	.mobile header {
		min-height: 44px;
	}
	.mobile .panel-content {
		flex: 1;
	}
</style>
