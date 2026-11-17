<script lang="ts">
	import { untrack } from 'svelte';
	import type { DistGraph } from '@tellegen/engine';
	import type { Network, Solution } from '../api.js';
	import {
		createDrawing,
		arrangeDrawing,
		deleteDrawing,
		drawingLabel,
		drawingExtent,
		restoreDrawing,
		drawingPaths,
		parseDrawing,
		saveDrawing,
		savedDrawings,
		type DrawingDocument,
		type DrawingObject
	} from '../diagram/drawing.js';
	import { downloadText } from '../format.js';
	let {
		network,
		distributionGraph,
		solution = null,
		label,
		onclose,
		onselectbus
	}: {
		network: Network;
		distributionGraph?: DistGraph;
		solution?: Solution | null;
		label: string;
		onclose: () => void;
		onselectbus: (id: number) => void;
	} = $props();
	const reference = untrack(() => createDrawing(network, label, distributionGraph));
	let drawing = $state<DrawingDocument>(structuredClone(reference));
	let selected = $state<string | null>(null);
	let selectedRoute = $state<string | null>(null);
	let selectedPort = $state<string | null>(null);
	let error = $state('');
	let notice = $state('');
	let saved = $state<DrawingDocument[]>([]);
	let history = $state<string[]>([]);
	let future = $state<string[]>([]);
	let recorded = untrack(() => JSON.stringify(drawing));
	// Form bindings and direct gestures share the same undo history.
	$effect(() => {
		const text = JSON.stringify(drawing);
		untrack(() => {
			if (text === recorded) return;
			history = [...history.slice(-99), recorded];
			future = [];
			recorded = text;
			notice = 'Unsaved drawing changes';
		});
	});
	let svg = $state<SVGSVGElement>();
	let zoom = $state(1);
	let offset = $state<[number, number]>([0, 0]);
	let snap = $state(true);
	let showPorts = $state(false);
	const orderedObjects = $derived(
		[...drawing.objects].sort(
			(a, b) =>
				Number(!['frame', 'image'].includes(a.kind)) - Number(!['frame', 'image'].includes(b.kind))
		)
	);
	const object = $derived(drawing.objects.find((o) => o.id === selected));
	const route = $derived(drawing.connections.find((c) => c.id === selectedRoute));
	const shownObjects = $derived(
		new Set(
			drawing.objects
				.filter(
					(o) => !o.hidden && (!drawing.station || o.station === drawing.station || !o.sourceId)
				)
				.map((o) => o.id)
		)
	);
	const flows = $derived(new Map(solution?.flows.map((f) => [f.branch, f]) ?? []));
	const paths = $derived(
		drawingPaths(drawing).filter(
			(p) => shownObjects.has(p.connection.from) && shownObjects.has(p.connection.to)
		)
	);
	const visible = (layer: string) => drawing.layers.find((l) => l.id === layer)?.visible !== false;
	const bounds = $derived.by(() => {
		const [x0, y0, x1, y1] = drawingExtent(drawing.objects.map((o) => [o.x, o.y]));
		const left = Math.min(0, x0) - 100,
			top = Math.min(0, y0) - 100;
		return [left, top, Math.max(800, x1) + 100 - left, Math.max(500, y1) + 100 - top];
	});
	function change(action: () => void) {
		history = [...history.slice(-99), JSON.stringify(drawing)];
		future = [];
		action();
		recorded = JSON.stringify(drawing);
		notice = 'Unsaved drawing changes';
	}
	function undo() {
		const text = history.at(-1);
		if (!text) return;
		future = [...future, JSON.stringify(drawing)];
		history = history.slice(0, -1);
		drawing = parseDrawing(text, drawing.network);
		recorded = JSON.stringify(drawing);
	}
	function redo() {
		const text = future.at(-1);
		if (!text) return;
		history = [...history, JSON.stringify(drawing)];
		future = future.slice(0, -1);
		drawing = parseDrawing(text, drawing.network);
		recorded = JSON.stringify(drawing);
	}
	function move(axis: 'x' | 'y', value: number) {
		if (!object || object.locked || !Number.isFinite(value)) return;
		change(() => {
			object[axis] = snap ? Math.round(value / drawing.grid) * drawing.grid : value;
		});
	}
	function updateNumber(key: 'rotation' | 'width' | 'height', value: number) {
		if (!object || !Number.isFinite(value) || (key !== 'rotation' && value <= 0)) return;
		change(() => {
			object[key] = value;
		});
	}
	function add(kind: 'text' | 'frame') {
		change(() => {
			const id = crypto.randomUUID();
			drawing.objects.push({
				id,
				kind,
				label: kind === 'text' ? 'Annotation' : 'Frame',
				x: 200,
				y: 200,
				width: 200,
				height: 100,
				rotation: 0,
				labelOffset: [0, -20],
				ports: [],
				layer: 'annotations',
				group: '',
				locked: false,
				hidden: false
			});
			selected = id;
		});
	}
	async function addImage(file?: File) {
		if (!file) return;
		if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type) || file.size > 5_000_000) {
			error = 'Choose a PNG, JPEG or WebP image under 5 MB';
			return;
		}
		const reader = new FileReader();
		reader.onload = () =>
			change(() => {
				const id = crypto.randomUUID();
				drawing.objects.push({
					id,
					kind: 'image',
					label: file.name,
					image: String(reader.result),
					x: 200,
					y: 200,
					width: 300,
					height: 200,
					rotation: 0,
					labelOffset: [0, -110],
					ports: [],
					layer: 'annotations',
					group: '',
					locked: false,
					hidden: false
				});
				selected = id;
			});
		reader.onerror = () => {
			error = 'Could not read reference image';
		};
		reader.readAsDataURL(file);
	}
	function relayout() {
		change(() => arrangeDrawing(drawing));
	}
	function restorePositions() {
		const originals = new Map(reference.objects.map((o) => [o.id, o]));
		change(() => {
			for (const o of drawing.objects) {
				const original = originals.get(o.id);
				if (original && !o.locked) {
					o.x = original.x;
					o.y = original.y;
				}
			}
		});
	}

	function align(axis: 'x' | 'y') {
		if (!object || !object.group) return;
		const anchor = object;
		change(() => {
			for (const o of drawing.objects)
				if (o.group === anchor.group && !o.locked) o[axis] = anchor[axis];
		});
	}
	function refreshSaved() {
		try {
			saved = savedDrawings(drawing.network);
			error = '';
		} catch (e) {
			error = String(e);
		}
	}
	function save() {
		try {
			saveDrawing(drawing);
			refreshSaved();
			notice = 'Drawing saved';
		} catch (e) {
			error = `Could not save drawing: ${String(e)}`;
		}
	}
	function open(id: string) {
		const next = saved.find((d) => d.id === id);
		if (!next) return;
		try {
			drawing = restoreDrawing(next, reference);
		} catch (e) {
			error = String(e);
			return;
		}
		recorded = JSON.stringify(drawing);
		history = [];
		future = [];
		selected = null;
		selectedRoute = null;
		notice = 'Drawing opened';
	}
	async function importFile(file?: File) {
		if (!file) return;
		try {
			const next = restoreDrawing(parseDrawing(await file.text(), drawing.network), reference);
			change(() => {
				drawing = next;
			});
			error = '';
		} catch (e) {
			error = String(e);
		}
	}
	function exportSvg() {
		if (!svg) return;
		const copy = svg.cloneNode(true) as SVGSVGElement;
		copy.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
		copy.setAttribute('width', '1200');
		copy.setAttribute('height', '800');
		copy.querySelectorAll('[data-edit-only]').forEach((node) => node.remove());
		downloadText(
			new XMLSerializer().serializeToString(copy),
			`${drawing.name}.svg`,
			'image/svg+xml'
		);
	}
	let drag: { id: string; x: number; y: number; px: number; py: number } | null = null;
	function startDrag(event: PointerEvent, o: DrawingObject) {
		selected = o.id;
		selectedPort = null;
		selectedRoute = null;
		if (o.locked || !svg) return;
		const matrix = svg.getScreenCTM();
		if (!matrix) return;
		const p = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
		history = [...history.slice(-99), JSON.stringify(drawing)];
		future = [];
		drag = { id: o.id, x: o.x, y: o.y, px: p.x, py: p.y };
		(event.currentTarget as Element).setPointerCapture(event.pointerId);
	}
	function dragMove(event: PointerEvent) {
		if (!drag || !svg) return;
		const o = drawing.objects.find((entry) => entry.id === drag!.id);
		const matrix = svg.getScreenCTM();
		if (!o || !matrix) return;
		const p = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
		const grid = snap ? drawing.grid : 0.001;
		o.x = Math.round((drag.x + (p.x - drag.px) / zoom) / grid) * grid;
		o.y = Math.round((drag.y + (p.y - drag.py) / zoom) / grid) * grid;
		recorded = JSON.stringify(drawing);
	}
</script>

<section class="engineering" aria-label="Engineering drawing">
	<header>
		<h2>{drawing.name}</h2>
		<button onclick={onclose}>Close drawing editor</button>
	</header>
	<p>
		Drawing changes affect presentation only. Connections and switch positions come from the
		electrical case.
	</p>
	<div class="toolbar">
		{#if network.equipment?.node_breaker?.stations.length}<label
				>Substation <select bind:value={drawing.station}
					><option value="">All substations</option
					>{#each network.equipment.node_breaker.stations as station (station.id)}<option
							value={station.id}>{station.name}</option
						>{/each}</select
				></label
			>{/if}
		<label>Name <input bind:value={drawing.name} /></label>
		<button onclick={save}>Save drawing</button><button onclick={refreshSaved}
			>List saved drawings</button
		>
		<select aria-label="Saved drawing" onchange={(e) => open(e.currentTarget.value)}
			><option value="">Choose drawing</option>{#each saved as d (d.id)}<option value={d.id}
					>{d.name}</option
				>{/each}</select
		>
		<button
			onclick={() =>
				change(() => {
					drawing.id = crypto.randomUUID();
					drawing.name += ' copy';
				})}>Duplicate drawing</button
		>
		<button
			onclick={() => {
				try {
					deleteDrawing(drawing);
					refreshSaved();
					notice = 'Saved drawing deleted';
				} catch (e) {
					error = String(e);
				}
			}}>Delete saved drawing</button
		>
		<button
			onclick={() =>
				downloadText(
					JSON.stringify(drawing, null, 2),
					`${drawing.name}.drawing.json`,
					'application/json'
				)}>Export drawing</button
		>
		<label
			>Import drawing <input
				type="file"
				accept=".drawing.json,application/json"
				onchange={(e) => void importFile(e.currentTarget.files?.[0])}
			/></label
		>
		<button onclick={exportSvg}>Export drawing SVG</button>
		<button onclick={undo} disabled={!history.length}>Undo</button><button
			onclick={redo}
			disabled={!future.length}>Redo</button
		>
		<button onclick={relayout}>Arrange unlocked objects</button><button onclick={restorePositions}
			>Restore initial positions</button
		><button onclick={() => add('text')}>Add text</button><button onclick={() => add('frame')}
			>Add frame</button
		>
		<label
			>Add reference image <input
				type="file"
				accept="image/png,image/jpeg,image/webp"
				onchange={(e) => void addImage(e.currentTarget.files?.[0])}
			/></label
		>
		<label><input type="checkbox" bind:checked={snap} />Snap to grid</label><label
			>Grid <input type="number" min="1" bind:value={drawing.grid} /></label
		>
		<label><input type="checkbox" bind:checked={showPorts} />Show ports</label>
		<label
			><input type="checkbox" bind:checked={drawing.showFlows} />Show active power and arrows</label
		>
		<label
			>Result decimals <input
				type="number"
				min="0"
				max="8"
				value={drawing.resultPrecision ?? 2}
				onchange={(e) => {
					const n = Number(e.currentTarget.value);
					if (Number.isInteger(n) && n >= 0 && n <= 8) change(() => (drawing.resultPrecision = n));
				}}
			/></label
		>
		<button onclick={() => (zoom *= 1.25)}>Zoom in</button><button onclick={() => (zoom /= 1.25)}
			>Zoom out</button
		><button
			onclick={() => {
				zoom = 1;
				offset = [0, 0];
			}}>Fit drawing</button
		>
		{#each drawing.layers as layer (layer.id)}<label
				><input type="checkbox" bind:checked={layer.visible} />{layer.id}</label
			>{/each}
	</div>
	{#if error}<p role="alert">{error}</p>{/if}
	<p role="status">{notice}</p>
	<div class="workspace">
		<svg
			bind:this={svg}
			viewBox={bounds.join(' ')}
			aria-label="Editable equipment drawing"
			role="img"
			onpointermove={dragMove}
			onpointerup={() => (drag = null)}
		>
			<defs
				><marker
					id="drawing-flow-arrow"
					viewBox="0 0 10 10"
					refX="8"
					refY="5"
					markerWidth="6"
					markerHeight="6"
					orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#273746" /></marker
				></defs
			>
			<rect x={bounds[0]} y={bounds[1]} width={bounds[2]} height={bounds[3]} fill="white" />
			<g
				transform={`translate(${offset.join(' ')}) scale(${zoom})`}
				fill="none"
				stroke="#273746"
				stroke-width="2"
			>
				{#each paths as path (path.connection.id)}
					{@const flow =
						path.connection.branch === undefined ? undefined : flows.get(path.connection.branch)}
					{@const flowVisible = drawing.showFlows && flow && Number.isFinite(flow.mw)}
					{@const middle = path.points[Math.floor(path.points.length / 2)]}
					{#if visible(path.connection.layer)}<polyline
							points={path.points.map((p) => p.join(',')).join(' ')}
							stroke={selectedRoute === path.connection.id
								? '#005bbb'
								: flowVisible && flow.loading > 1
									? '#c0392b'
									: '#273746'}
							marker-end={flowVisible && flow.mw > 0 ? 'url(#drawing-flow-arrow)' : undefined}
							marker-start={flowVisible && flow.mw < 0 ? 'url(#drawing-flow-arrow)' : undefined}
							stroke-dasharray={path.connection.connected ? undefined : '6 5'}
							role="button"
							tabindex="0"
							aria-label={path.connection.label || path.connection.id}
							onclick={() => {
								selectedRoute = path.connection.id;
								selected = null;
							}}
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									selectedRoute = path.connection.id;
									selected = null;
								}
							}}
						/>
						{#if flowVisible}<text
								x={middle[0]}
								y={middle[1] - 12}
								stroke="none"
								fill="#273746"
								font-family="sans-serif"
								font-size="13">{flow.mw.toFixed(drawing.resultPrecision ?? 2)} MW</text
							>{/if}{/if}
				{/each}
				{#each orderedObjects as o (o.id)}
					{#if shownObjects.has(o.id) && visible(o.layer)}
						<g
							transform={`translate(${o.x} ${o.y}) rotate(${o.rotation})`}
							stroke={o.colour || '#273746'}
							role="button"
							tabindex="0"
							aria-label={`${o.kind} ${o.label}${o.open === undefined ? '' : o.open ? ' open' : ' closed'}`}
							onpointerdown={(e) => startDrag(e, o)}
							onkeydown={(e) => {
								if (e.key === 'Enter') {
									selected = o.id;
									selectedRoute = null;
								}
							}}
						>
							<rect
								data-edit-only
								x={-Math.max(30, o.width / 2)}
								y={-Math.max(35, o.height / 2)}
								width={Math.max(60, o.width)}
								height={Math.max(70, o.height)}
								fill="transparent"
								stroke="none"
								pointer-events="all"
							/>
							{#if o.kind === 'busbar'}<line
									x1={-o.width / 2}
									x2={o.width / 2}
									y1="0"
									y2="0"
									stroke-width="6"
								/>
							{:else if o.kind === 'breaker'}<rect
									x="-12"
									y="-12"
									width="24"
									height="24"
									fill={o.open ? 'white' : '#273746'}
								/>
							{:else if o.kind === 'disconnector'}<path
									d={o.open ? 'M-15 0 L10 -15 M12 0 H15' : 'M-15 0 H15'}
								/>
							{:else if o.kind === 'transformer'}<circle cx="-7" r="12" /><circle
									cx="7"
									r="12"
								/>{#if o.ports.length > 2}<circle cy="12" r="12" />{/if}
							{:else if o.kind === 'load'}<path d="M-12 -10 H12 L0 14 Z" />
							{:else if o.kind === 'generator'}<circle r="15" /><text
									x="0"
									y="5"
									stroke="none"
									fill="#273746"
									text-anchor="middle"
									font-size="15">G</text
								>
							{:else if o.kind === 'node'}<circle r="5" fill="#273746" />
							{:else if o.kind === 'image'}<image
									href={o.image}
									x={-o.width / 2}
									y={-o.height / 2}
									width={o.width}
									height={o.height}
								/>
							{:else if o.kind === 'frame'}<rect
									x={-o.width / 2}
									y={-o.height / 2}
									width={o.width}
									height={o.height}
								/>
							{:else if o.kind !== 'text'}<rect x="-15" y="-10" width="30" height="20" />{/if}
							<text
								x={o.labelOffset[0]}
								y={o.labelOffset[1]}
								fill={o.colour || '#273746'}
								stroke="none"
								font-family="sans-serif"
								font-size="13"
								text-anchor="middle">{drawingLabel(o, solution)}</text
							>
							{#if showPorts}{#each o.ports as port (port.id)}<circle
										cx={port.x}
										cy={port.y}
										r="3"
										fill={selectedPort === port.id && selected === o.id ? '#c0392b' : '#005bbb'}
										role="button"
										tabindex="0"
										aria-label={`Terminal ${port.id} of ${o.label}`}
										onpointerdown={(e) => e.stopPropagation()}
										onclick={(e) => {
											e.stopPropagation();
											selected = o.id;
											selectedPort = port.id;
											selectedRoute = null;
										}}
										onkeydown={(e) => {
											if (e.key === 'Enter') {
												e.stopPropagation();
												selected = o.id;
												selectedPort = port.id;
												selectedRoute = null;
											}
										}}><title>Terminal {port.id}</title></circle
									>{/each}{/if}
							{#if selected === o.id}<rect
									data-edit-only
									x={-Math.max(20, o.width / 2) - 5}
									y={-Math.max(20, o.height / 2) - 5}
									width={Math.max(40, o.width) + 10}
									height={Math.max(40, o.height) + 10}
									stroke="#005bbb"
									stroke-dasharray="4 4"
								/>{/if}
						</g>
					{/if}
				{/each}
			</g>
		</svg>
		<aside aria-label="Drawing object properties">
			<label
				>Object <select
					value={selected ?? ''}
					onchange={(e) => {
						selected = e.currentTarget.value;
						selectedRoute = null;
					}}
					><option value="">Choose object</option>{#each drawing.objects as o (o.id)}<option
							value={o.id}>{o.kind}: {o.label}</option
						>{/each}</select
				></label
			>
			{#if object}
				<p>{object.sourceId ?? 'Drawing annotation'}</p>
				{#if selectedPort}<p>Selected terminal: {selectedPort}</p>{/if}
				<label
					>Label <input
						value={object.label}
						onchange={(e) =>
							change(() => {
								object.label = e.currentTarget.value;
							})}
					/></label
				>
				{#each ['x', 'y'] as axis (axis)}<label
						>{axis.toUpperCase()}
						<input
							type="number"
							value={object[axis as 'x' | 'y']}
							disabled={object.locked}
							onchange={(e) => move(axis as 'x' | 'y', e.currentTarget.valueAsNumber)}
						/></label
					>{/each}
				{#each ['rotation', 'width', 'height'] as key (key)}<label
						>{key}
						<input
							type="number"
							value={object[key as 'rotation' | 'width' | 'height']}
							onchange={(e) =>
								updateNumber(key as 'rotation' | 'width' | 'height', e.currentTarget.valueAsNumber)}
						/></label
					>{/each}
				<label
					>Label X <input
						type="number"
						value={object.labelOffset[0]}
						onchange={(e) =>
							change(() => {
								object.labelOffset[0] = e.currentTarget.valueAsNumber;
							})}
					/></label
				>
				<label
					>Label Y <input
						type="number"
						value={object.labelOffset[1]}
						onchange={(e) =>
							change(() => {
								object.labelOffset[1] = e.currentTarget.valueAsNumber;
							})}
					/></label
				>
				<label
					>Label value <select bind:value={object.field}
						><option value="name">Name</option><option value="voltage">Voltage (pu)</option><option
							value="price">Price ($/MWh)</option
						></select
					></label
				>
				<label
					>Decimal places <input
						type="number"
						min="0"
						max="8"
						value={object.precision ?? 3}
						onchange={(e) => {
							const value = e.currentTarget.valueAsNumber;
							if (Number.isInteger(value) && value >= 0 && value <= 8)
								change(() => {
									object.precision = value;
								});
						}}
					/></label
				>
				<label>Group <input bind:value={object.group} /></label><button onclick={() => align('x')}
					>Align group vertically</button
				><button onclick={() => align('y')}>Align group horizontally</button>
				<label
					><input type="checkbox" bind:checked={object.locked} />Keep position during layout</label
				><label><input type="checkbox" bind:checked={object.hidden} />Hide object</label>
				<label
					>Colour <input
						type="color"
						value={object.colour ?? '#273746'}
						onchange={(e) =>
							change(() => {
								object.colour = e.currentTarget.value;
							})}
					/></label
				>
				{#each object.ports as port (port.id)}<fieldset>
						<legend>Port {port.id}</legend><label
							>X <input type="number" bind:value={port.x} /></label
						><label>Y <input type="number" bind:value={port.y} /></label>
					</fieldset>{/each}
				{#if object.bus !== undefined}<button onclick={() => onselectbus(object.bus!)}
						>Select electrical bus</button
					>{/if}
				{#if !object.sourceId}<button
						onclick={() =>
							change(() => {
								drawing.objects = drawing.objects.filter((o) => o.id !== selected);
								selected = null;
							})}>Delete annotation</button
					>{/if}
			{:else if route}
				<p>{route.label || route.id}</p>
				<p>{route.connected ? 'Connected' : 'Disconnected in source'}</p>
				<button
					onclick={() =>
						change(() => {
							route.bends.push([200, 200]);
						})}>Add route bend</button
				>
				{#each route.bends as bend, index (index)}<fieldset>
						<legend>Bend {index + 1}</legend><label
							>X <input type="number" bind:value={bend[0]} /></label
						><label>Y <input type="number" bind:value={bend[1]} /></label><button
							onclick={() =>
								change(() => {
									route.bends.splice(index, 1);
								})}>Remove bend</button
						>
					</fieldset>{/each}
			{:else}<p>Select an object or connection to edit its drawing.</p>{/if}
		</aside>
	</div>
</section>

<style>
	.engineering {
		position: absolute;
		inset: 0;
		z-index: var(--z-panel, 50);
		background: var(--panel-bg, white);
		color: var(--ink, #273746);
		padding: 16px;
		overflow: auto;
	}
	header,
	.toolbar {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	header {
		justify-content: space-between;
	}
	h2 {
		margin: 0;
	}
	p {
		font-size: 12px;
	}
	button,
	input,
	select {
		font: inherit;
		padding: 4px;
	}
	input[type='number'] {
		width: 70px;
	}
	input[type='file'] {
		max-width: 180px;
	}
	.workspace {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 240px;
		min-height: 500px;
	}
	svg {
		width: 100%;
		height: 65vh;
		min-height: 400px;
		border: 1px solid #aaa;
		touch-action: none;
	}
	svg [role='button'] {
		cursor: pointer;
	}
	aside {
		padding: 12px;
		overflow: auto;
		max-height: 65vh;
	}
	aside label {
		display: block;
		margin: 6px 0;
	}
	aside p {
		overflow-wrap: anywhere;
	}
	fieldset {
		margin: 6px 0;
	}
	@media (max-width: 700px) {
		.workspace {
			grid-template-columns: 1fr;
		}
		aside {
			max-height: none;
		}
	}
</style>
