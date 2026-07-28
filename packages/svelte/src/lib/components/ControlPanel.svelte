<script lang="ts">
	import { untrack } from 'svelte';
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import { splitName } from '../format.js';
	import AppFooter from './AppFooter.svelte';
	import BindingLines from './BindingLines.svelte';
	import BusPicker from './BusPicker.svelte';
	import DemandSlider from './DemandSlider.svelte';
	import DisplayControls from './DisplayControls.svelte';
	import FormulationSelector from './FormulationSelector.svelte';
	import LocalCaseDetails from './LocalCaseDetails.svelte';
	import MulticonductorDetails from './MulticonductorDetails.svelte';
	import NetworkStats from './NetworkStats.svelte';
	import RatingSlider from './RatingSlider.svelte';
	import SensitivityReadout from './SensitivityReadout.svelte';
	import SizeLegend from './SizeLegend.svelte';
	import TopMovers from './TopMovers.svelte';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();

	// Bottom sheet heights, compact layout only. peek is measured from the grab
	// bar; half and full are fractions of the viewport.
	type Snap = 'peek' | 'half' | 'full';
	const SNAPS: Snap[] = ['peek', 'half', 'full'];
	const SNAP_FRACTION: Record<Snap, number> = { peek: 0, half: 0.46, full: 0.92 };
	/** Below this the viewport is a landscape phone and half has to give up room. */
	const SHORT_VIEWPORT_PX = 560;
	/** px/ms above which a release snaps in the drag direction instead of to the nearest. */
	const FLICK_SPEED = 0.35;
	/** Map left visible above a full sheet, on top of the header it also clears. */
	const FULL_SNAP_MAP_BAND = 14;

	let snap = $state<Snap>('half');
	let headEl = $state.raw<HTMLElement | undefined>(undefined);
	let bodyEl = $state.raw<HTMLElement | undefined>(undefined);
	let headHeight = $state(52);
	/** Height under an active drag; null when resting on a snap. */
	let dragHeight = $state<number | null>(null);

	const fractionFor = (s: Snap) =>
		s === 'half' && app.viewportHeight < SHORT_VIEWPORT_PX ? 0.38 : SNAP_FRACTION[s];

	const snapHeight = (s: Snap) =>
		s === 'peek'
			? headHeight
			: Math.min(
					Math.round(app.viewportHeight * fractionFor(s)),
					app.viewportHeight - app.headerInset - FULL_SNAP_MAP_BAND
				);
	const sheetHeight = $derived(dragHeight ?? snapHeight(snap));

	const caseLabel = $derived(
		app.active
			? splitName(app.active.name)[0]
			: (app.activeLocal?.label ?? app.activeMulti?.label ?? '')
	);

	/** Single line shown on the grab bar while collapsed. */
	const summary = $derived.by(() => {
		if (app.error) return 'error — open for details';
		if (app.selectedBus !== null) return `bus ${app.selectedBus} · ∂LMP/∂d`;
		if (app.selectedBranch !== null) return `line ${app.selectedBranch} · ∂LMP/∂rating`;
		const stats = ctrl.networkStats;
		if (!stats) return app.parsingFile ? 'parsing…' : 'loading…';
		return `${caseLabel} · ${stats.buses} buses · ${stats.branches} lines`;
	});

	const expandLabel = $derived(
		snap === 'full' ? 'collapse the control panel' : 'expand the control panel'
	);

	/** A selection whose readout has resolved (or is resolving). */
	const busReadout = $derived(
		app.selectedBus !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)
	);
	const branchReadout = $derived(
		app.selectedBranch !== null && (ctrl.selectedSensitivity || app.sensitivityLoading)
	);
	/** Compact only: a resolved selection outranks the case stats it was read
	 * against. Gated on networkStats because that is what renders the readout. */
	const selectionLeads = $derived(
		app.compactLayout && !!ctrl.networkStats && (busReadout || branchReadout)
	);

	$effect(() => {
		app.sheetInset = app.compactLayout ? sheetHeight : 0;
	});

	// peek height tracks the grab bar, which resizes when the summary line changes.
	$effect(() => {
		const el = headEl;
		if (!el) return;
		const measure = () => (headHeight = Math.round(el.getBoundingClientRect().height));
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(el);
		return () => observer.disconnect();
	});

	// A map selection renders the sensitivity readout and the demand control, both
	// of which are below the fold at peek. The body also has to go back to the top:
	// reaching the bus lookup scrolls it down, and the readout that replaces it as
	// the first block would otherwise open out of view.
	$effect(() => {
		const selected = app.selectedBus ?? app.selectedBranch;
		if (selected === null) return;
		untrack(() => {
			if (!app.compactLayout) return;
			if (snap === 'peek') snap = 'half';
			if (bodyEl) bodyEl.scrollTop = 0;
		});
	});

	function cycle() {
		snap = snap === 'peek' ? 'half' : snap === 'half' ? 'full' : 'peek';
	}

	function nearestSnap(height: number): Snap {
		return SNAPS.reduce((best, s) =>
			Math.abs(snapHeight(s) - height) < Math.abs(snapHeight(best) - height) ? s : best
		);
	}

	let dragStartY = 0;
	let dragStartHeight = 0;
	let dragMoved = false;
	let lastY = 0;
	let lastT = 0;
	let speed = 0;

	function onPointerDown(e: PointerEvent) {
		if (!app.compactLayout || !e.isPrimary) return;
		if (e.currentTarget instanceof HTMLElement) e.currentTarget.setPointerCapture(e.pointerId);
		dragStartY = e.clientY;
		dragStartHeight = sheetHeight;
		dragHeight = dragStartHeight;
		dragMoved = false;
		speed = 0;
		lastY = e.clientY;
		lastT = e.timeStamp;
	}

	function onPointerMove(e: PointerEvent) {
		if (dragHeight === null) return;
		const dy = dragStartY - e.clientY;
		if (Math.abs(dy) > 4) dragMoved = true;
		const dt = e.timeStamp - lastT;
		if (dt > 0) speed = (lastY - e.clientY) / dt;
		lastY = e.clientY;
		lastT = e.timeStamp;
		dragHeight = Math.max(
			snapHeight('peek'),
			Math.min(snapHeight('full'), dragStartHeight + dy)
		);
	}

	function onPointerUp() {
		if (dragHeight === null) return;
		const height = dragHeight;
		dragHeight = null;
		if (!dragMoved) return; // a tap; the click handler cycles
		const landed = nearestSnap(height);
		if (Math.abs(speed) > FLICK_SPEED) {
			const next = SNAPS.indexOf(landed) + (speed > 0 ? 1 : -1);
			snap = SNAPS[Math.max(0, Math.min(SNAPS.length - 1, next))];
		} else {
			snap = landed;
		}
	}

	function onHeadKeydown(e: KeyboardEvent) {
		const step = e.key === 'ArrowUp' ? 1 : e.key === 'ArrowDown' ? -1 : 0;
		if (step === 0) return;
		e.preventDefault();
		const next = SNAPS.indexOf(snap) + step;
		snap = SNAPS[Math.max(0, Math.min(SNAPS.length - 1, next))];
	}
</script>

<aside
	class="panel"
	class:sheet={app.compactLayout}
	class:dragging={dragHeight !== null}
	style:height={app.compactLayout ? `${sheetHeight}px` : null}
>
	{#if app.compactLayout}
		<button
			type="button"
			class="sheet-head"
			bind:this={headEl}
			aria-expanded={snap !== 'peek'}
			aria-controls="control-panel-body"
			aria-label={expandLabel}
			onpointerdown={onPointerDown}
			onpointermove={onPointerMove}
			onpointerup={onPointerUp}
			onpointercancel={onPointerUp}
			onkeydown={onHeadKeydown}
			onclick={() => {
				// click fires after pointerup; a keyboard activation leaves dragMoved
				// unset, so clear it here rather than in the pointer handlers
				const moved = dragMoved;
				dragMoved = false;
				if (!moved) cycle();
			}}
		>
			<span class="grabber" aria-hidden="true"></span>
			{#if snap === 'peek'}
				<span class="sheet-summary mono">{summary}</span>
			{/if}
		</button>
	{/if}

	<div class="panel-body" id="control-panel-body" bind:this={bodyEl}>
		{#snippet selectionBlock()}
			{#if busReadout}
				<SensitivityReadout />

				<DemandSlider />

				{#if ctrl.showMoverSlot}
					<TopMovers />
				{/if}
			{:else if branchReadout}
				<SensitivityReadout />

				<RatingSlider />

				{#if ctrl.showMoverSlot}
					<TopMovers />
				{/if}
			{:else}
				<DisplayControls />
			{/if}
		{/snippet}

		{#if app.error}
			<p class="error mono">{app.error}</p>
			<div class="error-actions">
				<button class="reset mono" onclick={ctrl.retryError}>retry</button>
				<button class="reset mono" onclick={() => (app.error = null)}>dismiss</button>
			</div>
		{/if}
		{#if app.parsingFile}
			<p class="dim mono blink">parsing&hellip;</p>
		{/if}

		<!-- The sheet shows a few hundred px at most, so what the last tap produced
		     leads; the case provenance and stats it was read against follow. -->
		{#if selectionLeads}
			{@render selectionBlock()}

			<hr />
		{/if}

		{#if app.activeLocal}
			<LocalCaseDetails />
		{/if}
		{#if app.activeMulti}
			<MulticonductorDetails />
		{/if}
		{#if !ctrl.networkStats}
			{#if !app.error && !app.activeLocal && !app.activeMulti}
				{#if ctrl.casesLoaded && app.cases.length === 0}
					<p class="dim mono">
						{config.loadDefaultCases ? 'no default cases loaded' : 'drop a case file to begin'}
					</p>
					{#if config.loadDefaultCases}
						<button class="reset mono" onclick={ctrl.restoreDefaultCases}
							>restore default cases</button
						>
					{/if}
				{:else if ctrl.loadingBackendCase}
					<p class="dim mono blink">loading selected case&hellip;</p>
				{:else}
					<p class="dim mono blink">loading cases&hellip;</p>
				{/if}
			{/if}
		{:else}
			{#if !app.activeLocal}
				<NetworkStats />
			{/if}

			{#if ctrl.activeSolvable}
				<FormulationSelector />
			{/if}

			{#if app.compactLayout}
				<BusPicker inline />
			{/if}

			{#if !app.placingLocalId}
				<BindingLines />
			{/if}

			{#if !selectionLeads}
				<hr />

				{@render selectionBlock()}
			{/if}

			<hr />

			<SizeLegend />
		{/if}

		{#if app.compactLayout && config.showFooter}
			<AppFooter inline />
		{/if}
	</div>
</aside>

<style>
	.panel {
		position: absolute;
		top: 76px;
		left: 20px;
		z-index: 10;
		display: flex;
		flex-direction: column;
		width: 312px;
		max-height: calc(100% - 122px);
		overflow: hidden;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.5s 0.12s ease-out both;
	}

	.panel-body {
		flex: 1 1 auto;
		min-height: 0;
		overflow-y: auto;
		/* A flick that runs past the end of the panel must not scroll the page or
		   pan the map underneath. */
		overscroll-behavior: contain;
		padding: 16px 18px;
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 12px 0;
	}

	.error {
		color: var(--red);
		font-size: 12px;
	}

	.error-actions {
		display: flex;
		gap: 6px;
	}

	/* The grab bar: a 44px strip that drags the sheet and cycles it on tap. */
	.sheet-head {
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 7px;
		width: 100%;
		min-height: 44px;
		padding: 9px 16px 11px;
		background: none;
		border: 0;
		border-bottom: 1px solid transparent;
		color: inherit;
		font: inherit;
		cursor: grab;
		/* the browser must not claim the vertical gesture */
		touch-action: none;
	}

	.sheet-head:active {
		cursor: grabbing;
	}

	.grabber {
		width: 40px;
		height: 4px;
		border-radius: 2px;
		background: var(--line);
	}

	.sheet-head:hover .grabber,
	.sheet-head:focus-visible .grabber {
		background: var(--accent);
	}

	.sheet-summary {
		max-width: 100%;
		overflow: hidden;
		font-size: 11.5px;
		color: var(--text-secondary);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sheet {
		top: auto;
		right: 0;
		bottom: 0;
		left: 0;
		width: auto;
		max-height: none;
		background: var(--paper);
		border-width: 1px 0 0;
		border-radius: 12px 12px 0 0;
		box-shadow: 0 -6px 28px rgba(32, 36, 43, 0.14);
		transition: height var(--dur-med) var(--ease-out);
	}

	.sheet .panel-body {
		padding: 0 16px 14px;
		/* clear the home indicator on a phone with no bezel */
		padding-bottom: max(14px, env(safe-area-inset-bottom));
		-webkit-overflow-scrolling: touch;
	}

	.sheet .sheet-head {
		border-bottom-color: var(--line);
	}

	/* A dragged sheet tracks the finger; the snap animation is for releases. */
	.sheet.dragging {
		transition: none;
	}

	@media (prefers-reduced-motion: reduce) {
		.panel {
			animation: none;
		}
	}
</style>
