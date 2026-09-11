<script lang="ts">
	import { tick } from 'svelte';
	import { getPanelLayout, type PanelRegistration, type PanelSide } from '../panels.svelte.js';
	let {
		panel,
		floating = false,
		mobile = false
	}: {
		panel: PanelRegistration;
		floating?: boolean;
		mobile?: boolean;
	} = $props();
	const layout = getPanelLayout();
	let menu = $state(false);
	let card: HTMLElement;
	const bounds = $derived(layout.bounds(panel));
	const preferred = $derived(!floating && !mobile && layout.preferred(panel));
	const dockReserve = $derived.by(() => {
		const side = layout.position(panel);
		return side === 'floating'
			? 0
			: Math.max(0, layout.ordered(side).filter((candidate) => candidate.open).length - 1) * 142;
	});
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
		layout.close(panel);
		await tick();
		document
			.querySelector<HTMLElement>(`.panel-toolbar button[aria-label="${CSS.escape(title)}"]`)
			?.focus();
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') menu = false;
	}}
/>
<aside
	bind:this={card}
	class="panel-frame"
	class:floating
	class:mobile
	class:preferred
	data-panel={panel.id}
	aria-label={panel.title}
	style:left={floating ? `${bounds.x}px` : undefined}
	style:top={floating ? `${bounds.y}px` : undefined}
	style:width={floating ? `${bounds.width}px` : undefined}
	style:height={floating ? `${bounds.height}px` : undefined}
	style:max-height={preferred ? `calc(100% - ${dockReserve}px)` : undefined}
>
	<header>
		{#if mobile}
			<h2 class="title">{panel.title}</h2>
		{:else}
			<button
				class="title"
				aria-label={`Move ${panel.title} panel`}
				title="Click to expand. Drag or use arrow keys to move. Open panel options to dock."
				onpointerdown={(event) => start(event, 'move')}
				onkeydown={key}
				onclick={() => layout.activate(panel)}>{panel.title}</button
			>
		{/if}
		<div class="header-actions">{@render panel.headerActions?.()}</div>
		{#if !mobile}<div class="menu-anchor">
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
						<button onclick={() => position()}>Detach</button>
					</div>{/if}
			</div>{/if}
		<button
			class="icon"
			aria-label={`Close ${panel.title.toLowerCase()} panel`}
			title="Close panel"
			onclick={close}
		>
			<svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"
				><path d="m4 4 8 8m0-8-8 8" stroke="currentColor" stroke-width="1.5" fill="none" /></svg
			>
		</button>
	</header>
	<div class="panel-content">{@render panel.children()}</div>
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
		background: var(--paper);
		color: var(--ink);
		border: 1px solid var(--line);
		border-radius: 5px;
		box-shadow: var(--elev-1);
		font: 13px/1.5 var(--font-display);
		position: relative;
	}
	header {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 8px 10px 8px 14px;
		flex: none;
		border-bottom: 1px solid var(--line);
	}
	button {
		color: inherit;
		font: inherit;
		cursor: pointer;
		border: 0;
		background: transparent;
		border-radius: 3px;
	}
	button:hover {
		background: var(--accent-soft);
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.title {
		flex: 1;
		min-width: 0;
		text-align: left;
		font-size: inherit;
		font-weight: 600;
		margin: 0;
		padding: 3px 0;
		cursor: grab;
		touch-action: none;
	}
	.title:active {
		cursor: grabbing;
	}
	.icon {
		display: grid;
		place-items: center;
		width: 26px;
		height: 28px;
		padding: 0;
		flex: none;
	}
	.icon svg {
		fill: currentColor;
	}
	.header-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.header-actions :global(button) {
		font: 11px/1.3 var(--font-display);
		padding: 4px;
		border: 0;
		background: none;
		color: var(--text-secondary);
	}
	.panel-content {
		min-height: 0;
		overflow: auto;
		overscroll-behavior: contain;
		display: flex;
		flex-direction: column;
		border-radius: 0 0 5px 5px;
	}
	.menu-anchor {
		position: relative;
	}
	.panel-menu {
		position: absolute;
		z-index: 3;
		top: 100%;
		right: 0;
		min-width: 130px;
		padding: 5px;
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 4px;
		box-shadow: var(--elev-2);
	}
	.panel-menu button {
		display: block;
		padding: 7px 10px;
		width: 100%;
		text-align: left;
		white-space: nowrap;
		font-size: 12px;
	}
	.preferred {
		flex-shrink: 0;
	}
	.floating {
		position: fixed;
		min-height: 0;
		z-index: 40;
		max-height: none;
		box-shadow: var(--elev-2);
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
		border-radius: 10px 10px 0 0;
	}
	.mobile .panel-content {
		flex: 1;
	}
	.mobile .title {
		cursor: default;
	}
</style>
