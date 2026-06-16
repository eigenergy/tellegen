import type { NetworkBranch, NetworkBus } from './api';
import type { Topology } from './wasm';

export interface GeoSidecar {
	sourceNames: string[];
	busCoords: Map<number, [number, number]>;
	branchPaths: Map<string, [number, number][]>;
	warnings: string[];
}

export interface AppliedGeoSidecar {
	view: { buses: NetworkBus[]; branches: NetworkBranch[] };
	sourceLabel: string;
	matchedBuses: number;
	matchedBranches: number;
	warnings: string[];
}

type CsvRecord = Record<string, string>;
type JsonRecord = Record<string, unknown>;

export function isGeoSidecarFile(name: string): boolean {
	const ext = name.split('.').pop()?.toLowerCase();
	return ext === 'csv' || ext === 'json' || ext === 'geojson';
}

export function mergeGeoSidecars(sidecars: GeoSidecar[]): GeoSidecar {
	const merged: GeoSidecar = {
		sourceNames: [],
		busCoords: new Map(),
		branchPaths: new Map(),
		warnings: []
	};
	for (const sidecar of sidecars) {
		merged.sourceNames.push(...sidecar.sourceNames);
		for (const [id, coord] of sidecar.busCoords) merged.busCoords.set(id, coord);
		for (const [key, path] of sidecar.branchPaths) merged.branchPaths.set(key, path);
		merged.warnings.push(...sidecar.warnings);
	}
	return merged;
}

export function parseGeoSidecar(name: string, text: string): GeoSidecar {
	const ext = name.split('.').pop()?.toLowerCase();
	const sidecar: GeoSidecar = {
		sourceNames: [name],
		busCoords: new Map(),
		branchPaths: new Map(),
		warnings: []
	};
	if (ext === 'csv') parseCsvSidecar(sidecar, text);
	else parseJsonSidecar(sidecar, text);
	if (sidecar.busCoords.size === 0 && sidecar.branchPaths.size === 0) {
		throw new Error('no bus coordinates or branch paths found');
	}
	return sidecar;
}

export function applyGeoSidecar(topology: Topology, sidecar: GeoSidecar): AppliedGeoSidecar {
	const buses = [...topology.buses].sort((a, b) => a.id - b.id);
	const missing = buses.filter((bus) => !sidecar.busCoords.has(bus.id)).map((bus) => bus.id);
	if (missing.length > 0) {
		const shown = missing.slice(0, 8).join(', ');
		throw new Error(
			`coordinates matched ${buses.length - missing.length}/${buses.length} buses; missing ${shown}${
				missing.length > 8 ? ', ...' : ''
			}`
		);
	}

	let matchedBranches = 0;
	const busCoords = sidecar.busCoords;
	const branches = topology.branches.map((branch) => {
		const from = busCoords.get(branch.from)!;
		const to = busCoords.get(branch.to)!;
		const path =
			sidecar.branchPaths.get(branchKey(branch.id)) ??
			sidecar.branchPaths.get(edgeKey(branch.from, branch.to)) ??
			sidecar.branchPaths.get(edgeKey(branch.to, branch.from)) ??
			[from, to];
		if (path.length > 2) matchedBranches++;
		return {
			id: branch.id,
			from: branch.from,
			to: branch.to,
			rate_mw: branch.rate_mw,
			status: branch.status,
			path
		};
	});

	return {
		view: {
			buses: buses.map((bus) => {
				const [lon, lat] = busCoords.get(bus.id)!;
				return { id: bus.id, lon, lat, demand_mw: bus.demand_mw, gen_mw: bus.gen_mw };
			}),
			branches
		},
		sourceLabel: sidecar.sourceNames.join(' + '),
		matchedBuses: buses.length,
		matchedBranches,
		warnings: sidecar.warnings
	};
}

function parseCsvSidecar(sidecar: GeoSidecar, text: string) {
	const rows = parseCsv(text);
	if (rows.length === 0) throw new Error('empty CSV');
	for (const row of rows) {
		addPointRecord(sidecar, row);
		addBranchRecord(sidecar, row);
	}
}

function parseJsonSidecar(sidecar: GeoSidecar, text: string) {
	const data = JSON.parse(text) as unknown;
	if (isFeatureCollection(data)) {
		for (const feature of data.features) addFeature(sidecar, feature);
		return;
	}
	for (const record of collectRecords(data)) {
		addPointRecord(sidecar, record);
		addBranchRecord(sidecar, record);
	}
}

function addFeature(sidecar: GeoSidecar, feature: JsonRecord) {
	const props = asRecord(feature.properties) ?? {};
	const geometry = asRecord(feature.geometry);
	if (!geometry) return;
	const type = String(geometry.type ?? '');
	if (type === 'Point') {
		const id = findNumber(props, ['bus_i', 'bus', 'bus_id', 'id', 'number']);
		const coord = firstCoord(geometry.coordinates);
		if (id !== null && coord) sidecar.busCoords.set(id, coord);
		return;
	}
	if (type === 'LineString') {
		const path = coordPath(geometry.coordinates);
		addBranchPath(sidecar, props, path);
		return;
	}
	if (type === 'MultiLineString') {
		const paths = Array.isArray(geometry.coordinates)
			? geometry.coordinates.flatMap((p) => coordPath(p))
			: [];
		addBranchPath(sidecar, props, paths);
	}
}

function addPointRecord(sidecar: GeoSidecar, record: JsonRecord | CsvRecord) {
	const id = findNumber(record, ['bus_i', 'bus', 'bus_id', 'bus number', 'number', 'id']);
	const lat = findNumber(record, ['lat', 'latitude', 'y']);
	const lon = findNumber(record, ['lon', 'lng', 'longitude', 'x']);
	if (id === null || lat === null || lon === null) return;
	if (!validCoord(lon, lat)) return;
	sidecar.busCoords.set(id, [lon, lat]);
}

function addBranchRecord(sidecar: GeoSidecar, record: JsonRecord | CsvRecord) {
	const from = findNumber(record, ['f_bus', 'from', 'from_bus']);
	const to = findNumber(record, ['t_bus', 'to', 'to_bus']);
	const path = pathFromRecord(record);
	if (path.length < 2) return;
	addBranchPath(sidecar, record, path);
	if (from !== null && !sidecar.busCoords.has(from)) sidecar.busCoords.set(from, path[0]);
	if (to !== null && !sidecar.busCoords.has(to)) sidecar.busCoords.set(to, path[path.length - 1]);
}

function addBranchPath(sidecar: GeoSidecar, record: JsonRecord | CsvRecord, path: [number, number][]) {
	if (path.length < 2) return;
	const from = findNumber(record, ['f_bus', 'from', 'from_bus']);
	const to = findNumber(record, ['t_bus', 'to', 'to_bus']);
	const id = findNumber(record, ['branch', 'branch_id', 'branch number', 'cats_id', 'id']);
	if (id !== null) sidecar.branchPaths.set(branchKey(id), path);
	if (from !== null && to !== null) {
		sidecar.branchPaths.set(edgeKey(from, to), path);
		if (!sidecar.busCoords.has(from)) sidecar.busCoords.set(from, path[0]);
		if (!sidecar.busCoords.has(to)) sidecar.busCoords.set(to, path[path.length - 1]);
	}
}

function pathFromRecord(record: JsonRecord | CsvRecord): [number, number][] {
	const rawPath = valueOf(record, ['path', 'geometry', 'coordinates']);
	if (Array.isArray(rawPath)) return coordPath(rawPath);
	const lat1 = findNumber(record, ['lat1', 'from_lat']);
	const lon1 = findNumber(record, ['lon1', 'lng1', 'from_lon', 'from_lng']);
	const lat2 = findNumber(record, ['lat2', 'to_lat']);
	const lon2 = findNumber(record, ['lon2', 'lng2', 'to_lon', 'to_lng']);
	if (lat1 === null || lon1 === null || lat2 === null || lon2 === null) return [];
	if (!validCoord(lon1, lat1) || !validCoord(lon2, lat2)) return [];
	return [
		[lon1, lat1],
		[lon2, lat2]
	];
}

function parseCsv(text: string): CsvRecord[] {
	const rows: string[][] = [];
	let row: string[] = [];
	let cell = '';
	let quoted = false;
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (quoted) {
			if (ch === '"' && text[i + 1] === '"') {
				cell += '"';
				i++;
			} else if (ch === '"') quoted = false;
			else cell += ch;
			continue;
		}
		if (ch === '"') quoted = true;
		else if (ch === ',') {
			row.push(cell);
			cell = '';
		} else if (ch === '\n') {
			row.push(cell);
			rows.push(row);
			row = [];
			cell = '';
		} else if (ch !== '\r') cell += ch;
	}
	if (cell || row.length > 0) {
		row.push(cell);
		rows.push(row);
	}
	const [headers, ...body] = rows.filter((r) => r.some((c) => c.trim() !== ''));
	if (!headers) return [];
	return body.map((cells) => {
		const record: CsvRecord = {};
		headers.forEach((header, i) => {
			record[header.trim()] = cells[i]?.trim() ?? '';
		});
		return record;
	});
}

function collectRecords(data: unknown): JsonRecord[] {
	if (Array.isArray(data)) return data.flatMap(collectRecords);
	const record = asRecord(data);
	if (!record) return [];
	const direct = Object.values(record).flatMap((value) => (Array.isArray(value) ? collectRecords(value) : []));
	return direct.length > 0 ? direct : [record];
}

function findNumber(record: JsonRecord | CsvRecord, names: string[]): number | null {
	const value = valueOf(record, names);
	if (typeof value === 'number' && Number.isFinite(value)) return value;
	if (typeof value !== 'string') return null;
	const parsed = Number(value.trim().replace(/^['"]|['"]$/g, ''));
	return Number.isFinite(parsed) ? parsed : null;
}

function valueOf(record: JsonRecord | CsvRecord, names: string[]): unknown {
	const wanted = new Set(names.map(normalizeKey));
	for (const [key, value] of Object.entries(record)) {
		if (wanted.has(normalizeKey(key))) return value;
	}
	return undefined;
}

function normalizeKey(key: string): string {
	return key.toLowerCase().replace(/[^a-z0-9]/g, '');
}

function coordPath(raw: unknown): [number, number][] {
	if (!Array.isArray(raw)) return [];
	return raw
		.map(firstCoord)
		.filter((coord): coord is [number, number] => coord !== null);
}

function firstCoord(raw: unknown): [number, number] | null {
	if (!Array.isArray(raw) || raw.length < 2) return null;
	const lon = Number(raw[0]);
	const lat = Number(raw[1]);
	return validCoord(lon, lat) ? [lon, lat] : null;
}

function validCoord(lon: number, lat: number): boolean {
	return Number.isFinite(lon) && Number.isFinite(lat) && Math.abs(lon) <= 180 && Math.abs(lat) <= 90;
}

function branchKey(id: number): string {
	return `branch:${id}`;
}

function edgeKey(from: number, to: number): string {
	return `edge:${from}:${to}`;
}

function asRecord(value: unknown): JsonRecord | null {
	return value && typeof value === 'object' && !Array.isArray(value) ? (value as JsonRecord) : null;
}

function isFeatureCollection(value: unknown): value is { features: JsonRecord[] } {
	const record = asRecord(value);
	return Array.isArray(record?.features);
}
