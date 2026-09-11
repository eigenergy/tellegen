<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { getAppState } from '../context.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { getNoticeCenter } from '../notices.svelte.js';
	import PanelCard from './PanelCard.svelte';
	const app = getAppState();
	const layout = getPanelLayout();
	const notices = getNoticeCenter();
	const bottomReserve = $derived(notices.entries.length ? 120 : 88);
	const left = $derived(layout.ordered('left').filter((p) => p.open));
	const right = $derived(layout.ordered('right').filter((p) => p.open));
	const floating = $derived(
		layout.ordered().filter((p) => p.open && layout.position(p) === 'floating')
	);
	const drawer = $derived(layout.panels.find((p) => p.id === layout.drawer));
	const toolbarTop = $derived(app.headerInset + 6);
	const panelTop = $derived(toolbarTop + 42);
	const drawerHeight = $derived(
		Math.max(
			1,
			Math.min(
				layout.viewportHeight * 0.6,
				(layout.viewportHeight - panelTop - bottomReserve - 8) * 0.75
			)
		)
	);
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
	$effect(() => {
		app.sheetInset = layout.compact && drawer ? drawerHeight + bottomReserve : 0;
	});
</script>

<svelte:window
	onresize={resize}
	onpointermove={(event) => layout.moveGesture(event.clientX, event.clientY)}
	onpointerup={() => layout.finishGesture()}
	onpointercancel={() => layout.finishGesture()}
/>
<div class="panel-toolbar" style:top="{toolbarTop}px" aria-label="Panels">
	<div class="panel-buttons">
		{#each layout.ordered().filter((p) => p.side === 'left') as panel (panel.id)}
			<button
				aria-label={panel.title}
				aria-pressed={layout.compact ? layout.drawer === panel.id : panel.open}
				onclick={() => layout.toggle(panel)}>{panel.title}</button
			>
		{/each}
	</div>
	<div class="panel-buttons right">
		{#each layout.ordered().filter((p) => p.side === 'right') as panel (panel.id)}
			<button
				aria-label={panel.title}
				aria-pressed={layout.compact ? layout.drawer === panel.id : panel.open}
				onclick={() => layout.toggle(panel)}>{panel.title}</button
			>
		{/each}
		<button
			class="reset-layout"
			onclick={() => layout.reset()}
			title="Restore the default panel positions">Reset layout</button
		>
	</div>
</div>
{#if layout.storageError}<p class="layout-error" role="status" style:top="{panelTop}px">
		{layout.storageError}
	</p>{/if}
{#if layout.compact}
	{#if drawer}<div class="drawer" style:bottom="{bottomReserve}px" style:height="{drawerHeight}px">
			<PanelCard panel={drawer} mobile />
		</div>{/if}
{:else}
	<div
		class="dock left"
		style:bottom="{Math.max(100, bottomReserve)}px"
		data-panel-dock="left"
		style:top="{panelTop}px"
		style:width="{layout.dockWidth('left')}px"
	>
		{#each left as panel (panel.id)}<PanelCard {panel} />{/each}
	</div>
	<div
		class="dock right"
		style:bottom="{Math.max(100, bottomReserve)}px"
		data-panel-dock="right"
		style:top="{panelTop}px"
		style:width="{layout.dockWidth('right')}px"
	>
		{#each right as panel (panel.id)}<PanelCard {panel} />{/each}
	</div>
	{#each floating as panel (panel.id)}<PanelCard {panel} floating />{/each}
{/if}

<style>
	.panel-toolbar {
		position: fixed;
		z-index: 30;
		left: 20px;
		right: 20px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		pointer-events: none;
		height: 34px;
	}
	.panel-buttons {
		display: flex;
		gap: 6px;
		align-items: center;
		pointer-events: auto;
	}
	button {
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--paper);
		color: var(--ink);
		font: 12px/1.4 var(--font-display);
		min-height: 32px;
		padding: 6px 12px;
		cursor: pointer;
		box-shadow: var(--elev-1);
	}
	button[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	button:hover {
		border-color: var(--accent);
	}
	.reset-layout {
		color: var(--text-secondary);
		font-size: 11px;
		padding: 6px 8px;
		background: var(--panel);
	}
	.dock {
		position: fixed;
		z-index: 25;
		bottom: 100px;
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 12px;
		pointer-events: none;
	}
	.dock.left {
		left: 20px;
	}
	.dock.right {
		right: 20px;
	}
	.drawer {
		position: fixed;
		bottom: 88px;
		left: 12px;
		right: 12px;
		z-index: 25;
	}
	.layout-error {
		position: fixed;
		left: 50%;
		transform: translateX(-50%);
		z-index: 42;
		max-width: 400px;
		margin: 0;
		padding: 8px 12px;
		font: 12px/1.5 var(--font-display);
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
	}
	@media (max-width: 960px) {
		.panel-toolbar {
			left: 12px;
			right: 12px;
			gap: 6px;
		}
		.panel-buttons {
			gap: 4px;
		}
		button {
			padding: 6px 8px;
			font-size: 11px;
		}
		.reset-layout {
			display: none;
		}
	}
</style>
