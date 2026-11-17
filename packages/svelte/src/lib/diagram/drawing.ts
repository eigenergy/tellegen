import type { Network, Solution } from '../api.js';
import type { NodeBreakerDrawing, DistGraph } from '@tellegen/engine';

export type DrawingPoint = [number, number];
export type DrawingKind =
	| 'busbar'
	| 'node'
	| 'breaker'
	| 'disconnector'
	| 'switch'
	| 'transformer'
	| 'load'
	| 'generator'
	| 'shunt'
	| 'equipment'
	| 'text'
	| 'frame'
	| 'image';
export interface DrawingObject {
	id: string;
	sourceId?: string;
	station?: string;
	kind: DrawingKind;
	label: string;
	x: number;
	y: number;
	rotation: number;
	width: number;
	height: number;
	labelOffset: DrawingPoint;
	ports: { id: string; x: number; y: number }[];
	layer: string;
	group: string;
	locked: boolean;
	hidden: boolean;
	open?: boolean;
	bus?: number;
	field?: 'name' | 'voltage' | 'price';
	colour?: string;
	image?: string;
	precision?: number;
}
export interface DrawingConnection {
	id: string;
	from: string;
	to: string;
	fromPort?: string;
	toPort?: string;
	bends: DrawingPoint[];
	label: string;
	connected: boolean;
	branch?: number;
	layer: string;
}
export interface DrawingDocument {
	format: 'tellegen-drawing';
	version: 1;
	id: string;
	name: string;
	network: string;
	objects: DrawingObject[];
	connections: DrawingConnection[];
	layers: { id: string; visible: boolean }[];
	grid: number;
	station?: string;
	showFlows?: boolean;
	resultPrecision?: number;
}

/** Identity follows source objects and connections, independent of positions and results. */
export function drawingNetworkKey(network: Network, distribution?: DistGraph): string {
	const detailed = network.equipment?.node_breaker;
	const ids: string[] = [];
	if (distribution) {
		for (const bus of distribution.buses) {
			ids.push(JSON.stringify(['bus', bus.id, [...bus.terminals].sort()]));
			for (const a of bus.attachments ?? [])
				ids.push(JSON.stringify(['attachment', a.kind, a.id, bus.id, [...a.terminals].sort()]));
		}
		for (const edge of distribution.edges)
			ids.push(
				JSON.stringify([
					'edge',
					edge.kind,
					edge.id,
					edge.from,
					edge.to,
					edge.conductors,
					edge.windings?.map((w) => [w.bus, w.terminals])
				])
			);
	} else if (detailed?.nodes.length) {
		for (const n of detailed.nodes)
			ids.push(JSON.stringify(['node', n.id, n.voltage_level, n.station, n.container]));
		for (const b of detailed.busbars) ids.push(JSON.stringify(['busbar', b.id, b.node]));
		for (const s of detailed.switches)
			ids.push(JSON.stringify(['switch', s.id, s.kind, s.from, s.to]));
		for (const c of detailed.connections) ids.push(JSON.stringify(['connection', c.from, c.to]));
		for (const t of detailed.terminals)
			ids.push(JSON.stringify(['terminal', t.id, t.equipment, t.kind, t.number, t.node]));
	} else {
		for (const b of network.buses) ids.push(JSON.stringify(['bus', b.uid ?? b.id]));
		for (const b of network.branches)
			ids.push(JSON.stringify(['branch', b.uid ?? b.id, b.from, b.to]));
		for (const t of network.equipment?.three_winding ?? [])
			ids.push(JSON.stringify(['transformer', t.uid ?? t.id, t.buses, t.branches]));
	}
	let hash = 2166136261;
	for (const char of ids.sort().join('\n')) hash = Math.imul(hash ^ char.charCodeAt(0), 16777619);
	return `${ids.length}-${(hash >>> 0).toString(16)}`;
}

export function drawingExtent(points: Iterable<DrawingPoint>): [number, number, number, number] {
	let x0 = Infinity,
		y0 = Infinity,
		x1 = -Infinity,
		y1 = -Infinity;
	for (const [x, y] of points) {
		x0 = Math.min(x0, x);
		y0 = Math.min(y0, y);
		x1 = Math.max(x1, x);
		y1 = Math.max(y1, y);
	}
	return Number.isFinite(x0) ? [x0, y0, x1, y1] : [0, 0, 0, 0];
}

const object = (
	id: string,
	kind: DrawingKind,
	label: string,
	x: number,
	y: number,
	extra: Partial<DrawingObject> = {}
): DrawingObject => ({
	id,
	sourceId: id,
	kind,
	label,
	x,
	y,
	rotation: 0,
	width: kind === 'busbar' ? 100 : 30,
	height: 30,
	labelOffset: [0, -25],
	ports: [
		{ id: 'in', x: -15, y: 0 },
		{ id: 'out', x: 15, y: 0 }
	],
	layer: 'equipment',
	group: '',
	locked: false,
	hidden: false,
	field: 'name',
	...extra
});
const connection = (
	id: string,
	from: string,
	to: string,
	extra: Partial<DrawingConnection> = {}
): DrawingConnection => ({
	id,
	from,
	to,
	bends: [],
	label: '',
	connected: true,
	layer: 'connections',
	...extra
});

function detailedObjects(
	data: NodeBreakerDrawing
): Pick<DrawingDocument, 'objects' | 'connections'> {
	const levelRows = new Map<string, typeof data.nodes>();
	for (const node of data.nodes) {
		const nodes = levelRows.get(node.voltage_level) ?? [];
		nodes.push(node);
		levelRows.set(node.voltage_level, nodes);
	}
	const objects: DrawingObject[] = [];
	let row = 0;
	for (const nodes of levelRows.values()) {
		for (const [i, n] of nodes.entries())
			objects.push(
				object(n.id, 'node', n.name, 100 + (i % 6) * 180, 100 + (row + Math.floor(i / 6)) * 360, {
					bus: n.bus ?? undefined,
					station: n.station ?? undefined,
					group: n.container ?? n.voltage_level
				})
			);
		row += Math.ceil(nodes.length / 6);
	}
	const byId = new Map(objects.map((o) => [o.id, o]));
	const connections: DrawingConnection[] = [];
	for (const b of data.busbars) {
		const n = byId.get(b.node);
		if (!n) continue;
		objects.push(
			object(b.id, 'busbar', b.name, n.x, n.y - 55, {
				bus: n.bus,
				station: n.station,
				group: n.group
			})
		);
		connections.push(connection(`busbar:${b.id}`, b.id, b.node));
	}
	for (const s of data.switches) {
		const a = byId.get(s.from),
			b = byId.get(s.to);
		if (!a || !b) continue;
		objects.push(
			object(
				s.id,
				s.kind === 'disconnector' ? 'disconnector' : s.kind === 'breaker' ? 'breaker' : 'switch',
				s.name,
				(a.x + b.x) / 2,
				(a.y + b.y) / 2 + 45,
				{ open: s.open, station: a.station, group: a.group }
			)
		);
		connections.push(
			connection(`${s.id}:1`, s.from, s.id, { toPort: 'in' }),
			connection(`${s.id}:2`, s.id, s.to, { fromPort: 'out' })
		);
	}
	for (const [i, c] of data.connections.entries())
		connections.push(connection(`internal:${i}`, c.from, c.to));
	const drawnEquipment = new Set([...data.busbars, ...data.switches].map((e) => e.id));
	const equipment = new Map<string, NodeBreakerDrawing['terminals']>();
	for (const terminal of data.terminals) {
		if (drawnEquipment.has(terminal.equipment)) continue;
		const terminals = equipment.get(terminal.equipment) ?? [];
		terminals.push(terminal);
		equipment.set(terminal.equipment, terminals);
	}
	for (const [id, terminals] of equipment) {
		const locations = terminals.map((t) => byId.get(t.node)).filter((n): n is DrawingObject => !!n);
		if (!locations.length) continue;
		const first = terminals[0];
		const kind: DrawingKind = first.kind.includes('transformer')
			? 'transformer'
			: first.kind.includes('load')
				? 'load'
				: first.kind.includes('generator')
					? 'generator'
					: first.kind.includes('shunt')
						? 'shunt'
						: 'equipment';
		const ports = terminals.map((t, i) => ({
			id: String(t.number),
			x: (i - (terminals.length - 1) / 2) * 20,
			y: -15
		}));
		objects.push(
			object(
				id,
				kind,
				first.name,
				locations.reduce((sum, n) => sum + n.x, 0) / locations.length,
				locations.reduce((highest, n) => Math.max(highest, n.y), -Infinity) + 120,
				{ ports, group: locations[0].group, station: locations[0].station }
			)
		);
		for (const t of terminals)
			connections.push(
				connection(t.id, t.node, id, {
					toPort: String(t.number),
					connected: t.connected,
					label: `Terminal ${t.number}`
				})
			);
	}
	return { objects, connections };
}

export function createDrawing(
	network: Network,
	name: string,
	distribution?: DistGraph
): DrawingDocument {
	const data = network.equipment?.node_breaker;
	let objects: DrawingObject[], connections: DrawingConnection[];
	if (data?.nodes.length) ({ objects, connections } = detailedObjects(data));
	else {
		objects = network.buses.map((b, i) =>
			object(
				`bus:${b.id}`,
				'busbar',
				b.name ?? `Bus ${b.id}`,
				100 + (i % 8) * 160,
				100 + Math.floor(i / 8) * 180,
				{ bus: b.id, group: String(b.substation ?? '') }
			)
		);
		connections = network.branches.map((b) =>
			connection(`branch:${b.id}`, `bus:${b.from}`, `bus:${b.to}`, {
				branch: b.id,
				connected: b.status !== 0
			})
		);
		const busObjects = new Map(objects.map((o) => [o.bus, o]));
		for (const t of network.equipment?.three_winding ?? []) {
			const id = `transformer:${t.uid ?? t.id}`;
			const buses = t.buses
				.map((bus) => busObjects.get(bus))
				.filter((o): o is DrawingObject => !!o);
			objects.push(
				object(
					id,
					'transformer',
					t.name ?? `Transformer ${t.id}`,
					buses.reduce((sum, b) => sum + b.x, 0) / Math.max(1, buses.length),
					Math.max(0, ...buses.map((b) => b.y)) + 75,
					{
						ports: t.buses.map((_, i) => ({
							id: String(i),
							x: (i - 1) * 20,
							y: -15
						}))
					}
				)
			);
			connections = connections.filter(
				(c) => c.branch === undefined || !t.branches.includes(c.branch)
			);
			for (const [i, bus] of t.buses.entries())
				connections.push(
					connection(`${id}:${i}`, `bus:${bus}`, id, {
						toPort: String(i),
						connected: t.in_service
					})
				);
		}
	}
	if (!data?.nodes.length && network.buses.length) {
		const [x0, y0, x1, y1] = drawingExtent(network.buses.map((b): DrawingPoint => [b.lon, b.lat]));
		const dx = x1 - x0,
			dy = y1 - y0;
		const scale = Math.max(800, Math.sqrt(network.buses.length) * 160) / Math.max(dx, dy, 1e-9);
		if ([x0, y0, scale].every(Number.isFinite) && (dx > 0 || dy > 0)) {
			const project = ([x, y]: DrawingPoint): DrawingPoint => [
				100 + (x - x0) * scale,
				100 + (network.coordinate_space === 'diagram' ? y - y0 : dy - y + y0) * scale
			];
			const positioned = new Map(objects.map((o) => [o.id, o]));
			const branchConnections = new Map(connections.map((c) => [c.branch, c]));
			for (const b of network.buses) {
				const o = positioned.get(`bus:${b.id}`)!;
				[o.x, o.y] = project([b.lon, b.lat]);
			}
			for (const branch of network.branches) {
				const c = branchConnections.get(branch.id);
				if (c) c.bends = branch.path.slice(1, -1).map(project);
			}
		}
	}
	if (distribution) {
		const buses = new Map(distribution.buses.map((b, i) => [b.id.toLowerCase(), objects[i]]));
		for (const b of distribution.buses) {
			const o = buses.get(b.id.toLowerCase())!;
			o.label = b.id;
			o.sourceId = `bus:${b.id}`;
			o.ports = b.terminals.map((terminal, i) => ({
				id: terminal,
				x: (i - (b.terminals.length - 1) / 2) * 20,
				y: 0
			}));
			for (const [i, attachment] of (b.attachments ?? []).entries()) {
				const id = `${attachment.kind}:${attachment.id}`;
				const kind: DrawingKind =
					attachment.kind === 'load'
						? 'load'
						: attachment.kind === 'generator' || attachment.kind === 'source'
							? 'generator'
							: attachment.kind === 'shunt'
								? 'shunt'
								: 'equipment';
				objects.push(
					object(id, kind, attachment.id, o.x + i * 60, o.y + 75, {
						ports: attachment.terminals.map((t, j) => ({
							id: t,
							x: (j - (attachment.terminals.length - 1) / 2) * 10,
							y: -15
						}))
					})
				);
			}
		}
		const routes = new Map(connections.map((c) => [c.branch, c.bends]));
		connections = [];
		const transformers = new Set<string>();
		for (const [edgeIndex, edge] of distribution.edges.entries()) {
			const a = buses.get(edge.from.toLowerCase()),
				b = buses.get(edge.to.toLowerCase());
			if (!a || !b) continue;
			if (edge.kind === 'transformer' && edge.windings?.length) {
				if (transformers.has(edge.id)) continue;
				transformers.add(edge.id);
				const id = `transformer:${edge.id}`;
				objects.push(
					object(id, 'transformer', edge.id, (a.x + b.x) / 2, (a.y + b.y) / 2 + 60, {
						ports: edge.windings.map((_, i) => ({
							id: String(i),
							x: (i - (edge.windings!.length - 1) / 2) * 20,
							y: -15
						}))
					})
				);
				for (const [i, winding] of edge.windings.entries()) {
					const bus = buses.get(winding.bus.toLowerCase());
					if (bus)
						connections.push(
							connection(`${id}:${i}`, bus.id, id, {
								toPort: String(i),
								label: `Winding ${i + 1}: ${winding.terminals.join(', ')}`
							})
						);
				}
			} else {
				for (const [from, to] of edge.conductors)
					connections.push(
						connection(
							`${edge.kind}:${edge.id}:${edge.from}:${from}:${edge.to}:${to}`,
							a.id,
							b.id,
							{
								branch: edgeIndex,
								bends: (routes.get(edgeIndex) ?? []).map((p) => [...p] as DrawingPoint),
								fromPort: from,
								toPort: to,
								connected: edge.closed,
								label: `${edge.id}: ${from}–${to}`
							}
						)
					);
			}
		}
		for (const b of distribution.buses)
			for (const attachment of b.attachments ?? []) {
				const bus = buses.get(b.id.toLowerCase())!;
				for (const terminal of attachment.terminals)
					connections.push(
						connection(
							`${attachment.kind}:${attachment.id}:${terminal}`,
							bus.id,
							`${attachment.kind}:${attachment.id}`,
							{
								fromPort: terminal,
								toPort: terminal,
								label: `Terminal ${terminal}`
							}
						)
					);
			}
	}
	return {
		format: 'tellegen-drawing',
		version: 1,
		id: crypto.randomUUID(),
		name,
		network: drawingNetworkKey(network, distribution),
		objects,
		connections,
		layers: [
			{ id: 'equipment', visible: true },
			{ id: 'connections', visible: true },
			{ id: 'annotations', visible: true }
		],
		grid: 10
	};
}
export function drawingPort(object: DrawingObject, id?: string): DrawingPoint {
	const port = object.ports.find((p) => p.id === id);
	const x = port?.x ?? 0,
		y = port?.y ?? 0,
		angle = (object.rotation * Math.PI) / 180;
	return [
		object.x + x * Math.cos(angle) - y * Math.sin(angle),
		object.y + x * Math.sin(angle) + y * Math.cos(angle)
	];
}
export function drawingPaths(
	drawing: DrawingDocument
): { connection: DrawingConnection; points: DrawingPoint[] }[] {
	const objects = new Map(drawing.objects.map((o) => [o.id, o]));
	return drawing.connections.flatMap((c) => {
		const a = objects.get(c.from),
			b = objects.get(c.to);
		return !a || !b
			? []
			: [
					{
						connection: c,
						points: [drawingPort(a, c.fromPort), ...c.bends, drawingPort(b, c.toPort)]
					}
				];
	});
}
export function drawingLabel(object: DrawingObject, solution?: Solution | null): string {
	if (object.bus !== undefined && object.field === 'voltage') {
		const value = solution?.vm?.find((v) => v.bus === object.bus)?.value;
		return value === undefined
			? `${object.label}: voltage unavailable`
			: `${object.label}: ${value.toFixed(object.precision ?? 3)} pu`;
	}
	if (object.bus !== undefined && object.field === 'price') {
		const value = solution?.prices.find((v) => v.bus === object.bus)?.value;
		return value === undefined
			? `${object.label}: price unavailable`
			: `${object.label}: ${value.toFixed(object.precision ?? 2)} $/MWh`;
	}
	return object.label;
}
export function parseDrawing(text: string, network: string): DrawingDocument {
	const value: unknown = JSON.parse(text);
	const record = (v: unknown): v is Record<string, unknown> =>
		!!v && typeof v === 'object' && !Array.isArray(v);
	const string = (v: unknown): v is string => typeof v === 'string' && v.length <= 10_000;
	const id = (v: unknown): v is string => string(v) && v.length > 0;
	const finite = (v: unknown): v is number =>
		typeof v === 'number' && Number.isFinite(v) && Math.abs(v) <= 1e9;
	const optional = (v: unknown, valid: (v: unknown) => boolean) => v === undefined || valid(v);
	const integer = (v: unknown) => finite(v) && Number.isSafeInteger(v) && v >= 0;
	const point = (p: unknown) => Array.isArray(p) && p.length === 2 && p.every(finite);
	const fail = (what: string): never => {
		throw new Error(`Invalid drawing ${what}`);
	};
	if (
		!record(value) ||
		value.format !== 'tellegen-drawing' ||
		value.version !== 1 ||
		value.network !== network ||
		!id(value.id) ||
		!string(value.name)
	)
		fail('or a different network');
	const d = value as Record<string, unknown>;
	if (
		!Array.isArray(d.objects) ||
		d.objects.length > 100_000 ||
		!Array.isArray(d.connections) ||
		d.connections.length > 200_000 ||
		!finite(d.grid) ||
		d.grid <= 0 ||
		!optional(d.station, string) ||
		!optional(d.showFlows, (v) => typeof v === 'boolean') ||
		!optional(d.resultPrecision, (v) => integer(v) && (v as number) <= 8) ||
		!Array.isArray(d.layers) ||
		d.layers.length > 1000
	)
		fail('contents');
	const layers = new Set<string>();
	for (const layer of d.layers as unknown[]) {
		if (
			!record(layer) ||
			!id(layer.id) ||
			layers.has(layer.id) ||
			typeof layer.visible !== 'boolean'
		)
			fail('layer');
		layers.add((layer as { id: string }).id);
	}
	const objects = new Map<string, Set<string>>();
	for (const item of d.objects as unknown[]) {
		if (!record(item)) fail('object');
		const o = item as Record<string, unknown>;
		if (
			!id(o.id) ||
			objects.has(o.id) ||
			!string(o.label) ||
			!string(o.group) ||
			!id(o.layer) ||
			!layers.has(o.layer) ||
			typeof o.locked !== 'boolean' ||
			typeof o.hidden !== 'boolean' ||
			![
				'busbar',
				'node',
				'breaker',
				'disconnector',
				'switch',
				'transformer',
				'load',
				'generator',
				'shunt',
				'equipment',
				'text',
				'frame',
				'image'
			].includes(String(o.kind)) ||
			![o.x, o.y, o.rotation, o.width, o.height].every(finite) ||
			(o.width as number) <= 0 ||
			(o.height as number) <= 0 ||
			!point(o.labelOffset) ||
			!Array.isArray(o.ports) ||
			o.ports.length > 10_000 ||
			!optional(o.sourceId, id) ||
			!optional(o.station, string) ||
			!optional(o.bus, integer) ||
			!optional(o.open, (v) => typeof v === 'boolean') ||
			!optional(o.field, (v) => ['name', 'voltage', 'price'].includes(String(v))) ||
			!optional(o.colour, (v) => typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) ||
			!optional(o.precision, (v) => integer(v) && (v as number) <= 8)
		)
			fail('object');
		const ports = new Set<string>();
		for (const port of o.ports as unknown[]) {
			if (!record(port) || !id(port.id) || ports.has(port.id) || !finite(port.x) || !finite(port.y))
				fail('port');
			ports.add((port as { id: string }).id);
		}
		if (
			o.kind === 'image' &&
			(typeof o.image !== 'string' ||
				!/^data:image\/(png|jpeg|webp);base64,[a-z0-9+/=]+$/i.test(o.image) ||
				o.image.length > 7_000_000)
		)
			fail('reference image');
		if (o.kind !== 'image' && o.image !== undefined) fail('image on another object');
		objects.set(o.id as string, ports);
	}
	const connections = new Set<string>();
	for (const item of d.connections as unknown[]) {
		if (!record(item)) fail('connection');
		const c = item as Record<string, unknown>;
		if (
			!id(c.id) ||
			connections.has(c.id) ||
			!id(c.from) ||
			!objects.has(c.from) ||
			!id(c.to) ||
			!objects.has(c.to) ||
			!string(c.label) ||
			typeof c.connected !== 'boolean' ||
			!id(c.layer) ||
			!layers.has(c.layer) ||
			!Array.isArray(c.bends) ||
			c.bends.length > 10_000 ||
			!c.bends.every(point) ||
			!optional(c.branch, integer) ||
			!optional(c.fromPort, (v) => id(v) && !!objects.get(c.from as string)?.has(v)) ||
			!optional(c.toPort, (v) => id(v) && !!objects.get(c.to as string)?.has(v))
		)
			fail('connection');
		connections.add(c.id as string);
	}
	return value as DrawingDocument;
}

/** Restore manual presentation while source connectivity and operating states remain authoritative. */
export function restoreDrawing(
	document: DrawingDocument,
	reference: DrawingDocument
): DrawingDocument {
	const sourceObjects = new Map(reference.objects.map((o) => [o.id, o]));
	const sourceConnections = new Map(reference.connections.map((c) => [c.id, c]));
	const saved = parseDrawing(JSON.stringify(document), reference.network);
	if (saved.connections.length !== reference.connections.length)
		throw new Error('Drawing connections do not match the network');
	for (const o of saved.objects) {
		const source = sourceObjects.get(o.id);
		if (!source) {
			if (o.sourceId || !['text', 'frame', 'image'].includes(o.kind))
				throw new Error('Drawing equipment does not match the network');
			continue;
		}
		if (o.sourceId !== source.sourceId || o.kind !== source.kind)
			throw new Error('Drawing equipment does not match the network');
		o.bus = source.bus;
		o.open = source.open;
		o.station = source.station;
		sourceObjects.delete(o.id);
	}
	if (sourceObjects.size) throw new Error('Drawing is missing network equipment');
	for (const c of saved.connections) {
		const source = sourceConnections.get(c.id);
		if (
			!source ||
			c.from !== source.from ||
			c.to !== source.to ||
			c.fromPort !== source.fromPort ||
			c.toPort !== source.toPort
		)
			throw new Error('Drawing connectivity does not match the network');
		c.connected = source.connected;
		c.branch = source.branch;
	}
	return saved;
}

/** Lay out connected equipment by graph distance while retaining locked positions. */
export function arrangeDrawing(document: DrawingDocument): void {
	const neighbors = new Map(document.objects.map((o) => [o.id, new Set<string>()]));
	for (const c of document.connections) {
		neighbors.get(c.from)?.add(c.to);
		neighbors.get(c.to)?.add(c.from);
	}
	const objects = new Map(document.objects.map((o) => [o.id, o]));
	const visited = new Set<string>();
	let componentRow = 0;
	for (const root of document.objects) {
		if (visited.has(root.id) || !root.sourceId) continue;
		const queue: [string, number][] = [[root.id, 0]];
		const rows = new Map<number, number>();
		visited.add(root.id);
		let height = 0;
		for (let i = 0; i < queue.length; i++) {
			const [id, level] = queue[i],
				o = objects.get(id)!;
			const row = rows.get(level) ?? 0;
			rows.set(level, row + 1);
			height = Math.max(height, row + 1);
			if (!o.locked) {
				o.x = 100 + level * 180;
				o.y = 100 + (componentRow + row) * 140;
			}
			for (const next of neighbors.get(id) ?? [])
				if (!visited.has(next)) {
					visited.add(next);
					queue.push([next, level + 1]);
				}
		}
		componentRow += height + 1;
	}
}

/** Bounds include rotated symbols, ports, label offsets and every routed connection. */
export function drawingBounds(
	document: DrawingDocument,
	solution: Solution | null = null
): [number, number, number, number] {
	let left = Infinity,
		top = Infinity,
		right = -Infinity,
		bottom = -Infinity;
	const include = (x: number, y: number) => {
		left = Math.min(left, x);
		top = Math.min(top, y);
		right = Math.max(right, x);
		bottom = Math.max(bottom, y);
	};
	for (const o of document.objects) {
		const angle = (o.rotation * Math.PI) / 180,
			cos = Math.cos(angle),
			sin = Math.sin(angle);
		const point = (x: number, y: number) =>
			include(o.x + x * cos - y * sin, o.y + x * sin + y * cos);
		const width = Math.max(40, o.width),
			height = Math.max(40, o.height);
		for (const x of [-width / 2, width / 2]) for (const y of [-height / 2, height / 2]) point(x, y);
		for (const p of o.ports) point(p.x, p.y);
		// A full font-size per character safely covers the variable-width rendered label.
		const halfLabel = (Array.from(drawingLabel(o, solution)).length * 13) / 2;
		for (const x of [-halfLabel, halfLabel])
			for (const y of [-16, 4]) point(o.labelOffset[0] + x, o.labelOffset[1] + y);
	}
	for (const path of drawingPaths(document)) for (const p of path.points) include(p[0], p[1]);
	if (!Number.isFinite(left)) return [0, 0, 800, 500];
	const margin = 100;
	return [
		left - margin,
		top - margin,
		Math.max(100, right - left) + margin * 2,
		Math.max(100, bottom - top) + margin * 2
	];
}

const storageKey = (network: string) => `tellegen.drawings.v1:${network}`;
export function savedDrawings(network: string): DrawingDocument[] {
	const values: unknown = JSON.parse(localStorage.getItem(storageKey(network)) ?? '[]');
	if (!Array.isArray(values)) throw new Error('Invalid saved drawing list');
	return values.map((d) => parseDrawing(JSON.stringify(d), network));
}
export function saveDrawing(drawing: DrawingDocument): void {
	parseDrawing(JSON.stringify(drawing), drawing.network);
	const all = savedDrawings(drawing.network).filter((d) => d.id !== drawing.id);
	localStorage.setItem(storageKey(drawing.network), JSON.stringify([...all, drawing]));
}
export function deleteDrawing(drawing: DrawingDocument): void {
	localStorage.setItem(
		storageKey(drawing.network),
		JSON.stringify(savedDrawings(drawing.network).filter((d) => d.id !== drawing.id))
	);
}

/** Copy the visible layout into a drawing without changing the electrical network. */
export function networkWithDrawingGeometry(
	network: Network,
	geometry: import('./schematic-layout.js').SchematicGeometry
): Network {
	const routes = new Map<number, DrawingPoint[]>();
	for (let i = 0; i < geometry.segmentEdge.length; i++) {
		const id = geometry.segmentEdge[i];
		const route = routes.get(id) ?? [[geometry.sources[i * 3], geometry.sources[i * 3 + 1]]];
		route.push([geometry.targets[i * 3], geometry.targets[i * 3 + 1]]);
		routes.set(id, route);
	}
	return {
		...network,
		coordinate_space: 'diagram',
		buses: network.buses.map((b, i) => ({
			...b,
			lon: geometry.positions[i * 3],
			lat: geometry.positions[i * 3 + 1]
		})),
		branches: network.branches.map((b, i) => ({
			...b,
			path: routes.get(i) ?? b.path
		}))
	};
}
