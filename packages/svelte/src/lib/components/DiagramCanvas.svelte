<script lang="ts">
	import { untrack } from 'svelte';
	import { on } from 'svelte/events';
	import type { Network, NetworkBus } from '../api.js';
	import { getAppState, getController } from '../context.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import { branchColor, busNeutral, busRadius, priceColor, scalarDomain } from '../colors.js';
	import { rgbaCss } from '../format.js';
	import { diagramLabels } from '../diagram-labels.js';

	let {
		network,
		caseId,
		onbusclick,
		onbranchclick,
		onclear
	}: {
		network: Network;
		caseId: string;
		onbusclick: (id: number) => void;
		onbranchclick: (id: number) => void;
		onclear: () => void;
	} = $props();
	const app = getAppState();
	const ctrl = getController();
	const panels = getPanelLayout();
	let width = $state(0),
		height = $state(0);
	let scale = $state(1),
		x = $state(0),
		y = $state(0);
	let dragging = $state(false);
	let gesture: { x: number; y: number; offsetX: number; offsetY: number; moved: boolean } | null =
		null;
	let fittedCase: string | null = null;
	let lastSize = { width: 0, height: 0 };
	let handledFrame = -1;
	let handledCamera = -1;
	const buses = $derived(
		network.buses.filter((bus) => Number.isFinite(bus.lon) && Number.isFinite(bus.lat))
	);
	const paths = $derived(
		network.branches
			.map((branch) => ({
				branch,
				points: branch.path.filter((point) => point.every(Number.isFinite))
			}))
			.filter((path) => path.points.length > 1)
	);
	const series = $derived.by(() => {
		if (!app.studyView) return ctrl.activeDisplay?.values ?? [];
		const solution = app.studyView.solution;
		if (app.displayMode === 'angle') return solution?.va ?? [];
		if (app.displayMode === 'voltage')
			return (
				solution?.vm ??
				(solution?.w ?? []).map((value) => ({
					...value,
					value: Math.sqrt(Math.max(0, value.value))
				}))
			);
		return solution?.lmp ?? [];
	});
	const displayMode = $derived(
		app.studyView ? app.displayMode : (ctrl.activeDisplay?.mode ?? app.displayMode)
	);
	const domain = $derived(
		scalarDomain(
			displayMode,
			series.map((value) => value.value)
		)
	);
	const values = $derived(new Map(series.map((value) => [value.bus, value.value])));
	const labels = $derived(diagramLabels(buses, scale, x, y, width, height, app.selectedBus));
	const loading = $derived(
		new Map(
			(app.studyView
				? (app.studyView.solution?.flows ?? [])
				: (ctrl.activeSolvable?.solution?.flows ?? [])
			).map((flow) => [flow.branch, flow.loading])
		)
	);
	function color(bus: NetworkBus) {
		const value = values.get(bus.id);
		return rgbaCss(
			value === undefined
				? busNeutral
				: priceColor((value - domain.lo) / Math.max(domain.hi - domain.lo, 1e-12))
		);
	}
	function bounds(points: [number, number][]) {
		let minX = Infinity,
			maxX = -Infinity,
			minY = Infinity,
			maxY = -Infinity;
		for (const [px, py] of points) {
			if (!Number.isFinite(px) || !Number.isFinite(py)) continue;
			minX = Math.min(minX, px);
			maxX = Math.max(maxX, px);
			minY = Math.min(minY, py);
			maxY = Math.max(maxY, py);
		}
		return Number.isFinite(minX) ? { minX, maxX, minY, maxY } : null;
	}
	function available() {
		const left = panels.compact ? 28 : panels.dockWidth('left') + 44;
		const right = panels.compact ? 28 : panels.dockWidth('right') + 44;
		const top = panels.top + 24;
		const bottom = panels.compact ? app.sheetInset + 24 : 88;
		return {
			left,
			top,
			width: Math.max(80, width - left - right),
			height: Math.max(60, height - top - bottom)
		};
	}
	function fit(points?: [number, number][], maxScale = Infinity) {
		const extent = bounds(
			points ?? [
				...buses.map((bus): [number, number] => [bus.lon, bus.lat]),
				...paths.flatMap((path) => path.points)
			]
		);
		if (!extent || width <= 0 || height <= 0) return;
		const space = available();
		const spanX = Math.max(extent.maxX - extent.minX, 1),
			spanY = Math.max(extent.maxY - extent.minY, 1);
		scale = Math.min(maxScale, 0.85 * Math.min(space.width / spanX, space.height / spanY));
		x = space.left + space.width / 2 - ((extent.minX + extent.maxX) * scale) / 2;
		y = space.top + space.height / 2 - ((extent.minY + extent.maxY) * scale) / 2;
	}
	function zoom(factor: number, px = width / 2, py = height / 2) {
		const next = Math.max(1e-6, Math.min(1e6, scale * factor));
		const ratio = next / scale;
		x = px - (px - x) * ratio;
		y = py - (py - y) * ratio;
		scale = next;
	}
	function restore(snapshot: { center: [number, number]; scale: number }) {
		scale = snapshot.scale;
		x = width / 2 - snapshot.center[0] * scale;
		y = height / 2 - snapshot.center[1] * scale;
	}
	function start(event: PointerEvent) {
		if (
			!event.isPrimary ||
			event.button !== 0 ||
			(event.target as Element).closest('[data-element]')
		)
			return;
		gesture = { x: event.clientX, y: event.clientY, offsetX: x, offsetY: y, moved: false };
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}
	function move(event: PointerEvent) {
		if (!gesture) return;
		const dx = event.clientX - gesture.x,
			dy = event.clientY - gesture.y;
		gesture.moved ||= Math.hypot(dx, dy) > 3;
		dragging = gesture.moved;
		x = gesture.offsetX + dx;
		y = gesture.offsetY + dy;
	}
	function stop() {
		if (gesture && !gesture.moved) onclear();
		gesture = null;
		dragging = false;
	}
	function wheel(node: HTMLDivElement) {
		return {
			destroy: on(
				node,
				'wheel',
				(event) => {
					if (!(event instanceof WheelEvent)) return;
					event.preventDefault();
					const rect = node.getBoundingClientRect();
					zoom(
						Math.exp(-Math.max(-100, Math.min(100, event.deltaY)) * 0.003),
						event.clientX - rect.x,
						event.clientY - rect.y
					);
				},
				{ passive: false }
			)
		};
	}
	function key(event: KeyboardEvent) {
		if ((event.target as Element).closest('[data-element]')) return;
		const step = event.shiftKey ? 80 : 30;
		if (event.key === 'ArrowLeft') x += step;
		else if (event.key === 'ArrowRight') x -= step;
		else if (event.key === 'ArrowUp') y += step;
		else if (event.key === 'ArrowDown') y -= step;
		else if (event.key === '+' || event.key === '=') zoom(1.25);
		else if (event.key === '-') zoom(0.8);
		else if (event.key === '0' || event.key.toLowerCase() === 'f') fit();
		else return;
		event.preventDefault();
	}
	function choose(event: KeyboardEvent, callback: () => void) {
		if (event.key !== 'Enter' && event.key !== ' ') return;
		event.preventDefault();
		event.stopPropagation();
		callback();
	}
	$effect(() => {
		const id = caseId,
			w = width,
			h = height;
		if (w <= 0 || h <= 0) return;
		untrack(() => {
			if (fittedCase !== id) {
				if (app.diagramCamera?.caseId === id) restore(app.diagramCamera);
				else fit();
				fittedCase = id;
			} else {
				x += (w - lastSize.width) / 2;
				y += (h - lastSize.height) / 2;
			}
			lastSize = { width: w, height: h };
		});
	});
	$effect(() => {
		const seq = app.frameSeq;
		if (!width || !height || seq === handledFrame) return;
		const target = app.frameTarget;
		handledFrame = seq;
		untrack(() => {
			if (typeof target === 'string') {
				if (target === 'all' || target === caseId) fit();
			} else if (target.caseId === caseId) {
				if ('busId' in target) {
					const bus = buses.find((bus) => bus.id === target.busId);
					if (bus) fit([[bus.lon, bus.lat]], scale * 2);
				} else {
					const branch = paths.find(({ branch }) => branch.id === target.branchId);
					if (branch) fit(branch.points, scale * 2);
				}
			}
			app.settleFrame();
		});
	});
	$effect(() => {
		const seq = app.diagramCameraSeq;
		const request = app.diagramCameraRequest;
		if (!width || !height || !request || request.caseId !== caseId || handledCamera === seq) return;
		untrack(() => {
			restore(request);
			handledCamera = seq;
			app.diagramCameraRequest = null;
		});
	});
	$effect(() => {
		if (width > 0 && height > 0 && scale > 0) {
			app.diagramCamera = {
				caseId,
				center: [(width / 2 - x) / scale, (height / 2 - y) / scale],
				scale
			};
		}
	});
</script>

<div class="diagram-stage" bind:clientWidth={width} bind:clientHeight={height}>
	<p id="diagram-keys" class="sr-only">Arrow keys pan; plus and minus zoom; F fits the drawing.</p>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (The diagram supports keyboard panning, zooming, and equipment selection.) -->
	<div
		class="interaction"
		use:wheel
		class:dragging
		role="application"
		aria-label="Network diagram"
		aria-describedby="diagram-keys"
		tabindex="0"
		onpointerdown={start}
		onpointermove={move}
		onpointerup={stop}
		onpointercancel={() => {
			gesture = null;
			dragging = false;
		}}
		onkeydown={key}
	>
		<svg aria-label="Network diagram" viewBox={`0 0 ${Math.max(1, width)} ${Math.max(1, height)}`}>
			<g transform={`translate(${x} ${y}) scale(${scale})`} data-diagram-transform>
				{#each paths as { branch, points } (branch.id)}
					<path
						d={`M${points.map((point) => point.join(',')).join(' L')}`}
						fill="none"
						stroke={app.selectedBranch === branch.id
							? '#2f6fbb'
							: rgbaCss(branchColor(loading.get(branch.id) ?? 0, branch.status !== 0))}
						stroke-width={app.selectedBranch === branch.id ? 4 : 2}
						vector-effect="non-scaling-stroke"
						role="button"
						tabindex={app.selectedBranch === branch.id ? 0 : -1}
						aria-label={`Line ${branch.id}, bus ${branch.from} to bus ${branch.to}`}
						data-element="branch"
						data-branch-id={branch.id}
						onclick={() => onbranchclick(branch.id)}
						onkeydown={(event) => choose(event, () => onbranchclick(branch.id))}
						><title>Line {branch.id}, bus {branch.from} to bus {branch.to}</title></path
					>
				{/each}
				{#each buses as bus, index (bus.id)}
					<g
						class="bus"
						role="button"
						tabindex={app.selectedBus === bus.id || (app.selectedBus === null && index === 0)
							? 0
							: -1}
						aria-label={`Bus ${bus.id}${bus.name ? `, ${bus.name}` : ''}`}
						data-element="bus"
						data-bus-id={bus.id}
						onclick={() => onbusclick(bus.id)}
						onkeydown={(event) => choose(event, () => onbusclick(bus.id))}
					>
						<circle
							cx={bus.lon}
							cy={bus.lat}
							r={Math.max(5, busRadius(Math.max(bus.demand_mw, bus.gen_mw))) / scale}
							fill={color(bus)}
							stroke={app.selectedBus === bus.id ? '#2f6fbb' : '#756a5b'}
							stroke-width={app.selectedBus === bus.id ? 3 : 1}
							vector-effect="non-scaling-stroke"
						/>
						<text
							class:hidden={!labels.has(bus.id)}
							x={bus.lon + 10 / scale}
							y={bus.lat - 10 / scale}
							font-size={11 / scale}>{bus.name || bus.id}</text
						>
						<title
							>Bus {bus.id}{bus.name ? `, ${bus.name}` : ''}, {bus.demand_mw.toFixed(1)} MW demand{values.has(
								bus.id
							)
								? `, ${values.get(bus.id)!.toFixed(4)} ${displayMode === 'price' ? 'objective units/MW LMP' : displayMode === 'voltage' ? 'pu voltage' : 'rad voltage angle'}`
								: ''}</title
						>
					</g>
				{/each}
			</g>
		</svg>
	</div>
	{#if !buses.length}<p class="empty">This drawing has no matched bus positions.</p>{/if}
	<div class="controls" aria-label="Diagram controls">
		<button onclick={() => fit()} title="Fit diagram (F)">Fit diagram</button>
		<div class="zoom">
			<button aria-label="Zoom in" onclick={() => zoom(1.25)}>+</button><button
				aria-label="Zoom out"
				onclick={() => zoom(0.8)}>-</button
			>
		</div>
	</div>
	<p class="coordinate-label">Diagram coordinates</p>
</div>

<style>
	.bus text.hidden {
		visibility: hidden;
	}
	.bus:hover text.hidden,
	.bus:focus-visible text.hidden {
		visibility: visible;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.diagram-stage {
		position: absolute;
		inset: 0;
		background: var(--bg);
		color: var(--ink);
	}
	svg {
		width: 100%;
		height: 100%;
	}
	.interaction {
		position: absolute;
		inset: 0;
		touch-action: none;
		cursor: grab;
	}
	.interaction.dragging {
		cursor: grabbing;
	}
	.interaction:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -3px;
	}
	[data-element] {
		cursor: pointer;
	}
	[data-element]:focus-visible {
		outline: none;
		stroke: var(--accent);
	}
	text {
		fill: var(--text-secondary);
		font-family: var(--font-mono);
		pointer-events: none;
	}
	.controls {
		position: absolute;
		right: 20px;
		bottom: 12px;
		display: flex;
		gap: 12px;
		align-items: flex-end;
	}
	button {
		padding: 6px 10px;
		border: 1px solid var(--line);
		border-radius: 4px;
		color: var(--ink);
		background: var(--paper);
		font: 12px/1.5 var(--font-display);
		cursor: pointer;
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.zoom {
		display: grid;
	}
	.zoom button {
		width: 32px;
		height: 30px;
		font: 18px/1 var(--font-display);
	}
	.coordinate-label {
		position: absolute;
		bottom: 44px;
		left: 20px;
		margin: 0;
		font: 10px/1.4 var(--font-mono);
		color: var(--text-secondary);
	}
	.empty {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		font: 13px var(--font-display);
	}
</style>
