import type { NetworkBranch, NetworkBus } from './api.js';
import type { Topology } from '@tellegen/engine';

export interface GeoFile {
	sourceNames: string[];
	busCoords: Map<number, [number, number]>;
	branchPaths: Map<string, [number, number][]>;
	warnings: string[];
}

export interface AppliedGeoFile {
	view: { buses: NetworkBus[]; branches: NetworkBranch[] };
	sourceLabel: string;
	matchedBuses: number;
	matchedBranches: number;
	warnings: string[];
}

type CsvRecord = Record<string, string>;
type JsonRecord = Record<string, unknown>;

export function isGeoFile(name: string): boolean {
	const ext = name.split('.').pop()?.toLowerCase();
	return ext === 'csv' || ext === 'json' || ext === 'geojson';
}

export function mergeGeoFiles(geoFiles: GeoFile[]): GeoFile {
	const merged: GeoFile = {
		sourceNames: [],
		busCoords: new Map(),
		branchPaths: new Map(),
		warnings: []
	};
	for (const geoFile of geoFiles) {
		merged.sourceNames.push(...geoFile.sourceNames);
		for (const [id, coord] of geoFile.busCoords) merged.busCoords.set(id, coord);
		for (const [key, path] of geoFile.branchPaths) merged.branchPaths.set(key, path);
		merged.warnings.push(...geoFile.warnings);
	}
	return merged;
}

export function parseGeoFile(name: string, text: string): GeoFile {
	const ext = name.split('.').pop()?.toLowerCase();
	const geoFile: GeoFile = {
		sourceNames: [name],
		busCoords: new Map(),
		branchPaths: new Map(),
		warnings: []
	};
	if (ext === 'csv') parseCsvGeoFile(geoFile, text);
	else parseJsonGeoFile(geoFile, text);
	if (geoFile.busCoords.size === 0 && geoFile.branchPaths.size === 0) {
		throw new Error('no bus coordinates or branch paths found');
	}
	return geoFile;
}

export function applyGeoFile(topology: Topology, geoFile: GeoFile): AppliedGeoFile {
	const buses = [...topology.buses].sort((a, b) => a.id - b.id);
	const missing = buses.filter((bus) => !geoFile.busCoords.has(bus.id)).map((bus) => bus.id);
	if (missing.length > 0) {
		const shown = missing.slice(0, 8).join(', ');
		throw new Error(
			`coordinates matched ${buses.length - missing.length}/${buses.length} buses; missing ${shown}${
				missing.length > 8 ? ', ...' : ''
			}`
		);
	}

	let matchedBranches = 0;
	const busCoords = geoFile.busCoords;
	const branches = topology.branches.map((branch) => {
		const from = busCoords.get(branch.from)!;
		const to = busCoords.get(branch.to)!;
		const path = geoFile.branchPaths.get(branchKey(branch.id)) ??
			geoFile.branchPaths.get(edgeKey(branch.from, branch.to)) ??
			geoFile.branchPaths.get(edgeKey(branch.to, branch.from)) ?? [from, to];
		if (path.length > 2) matchedBranches++;
		return {
			id: branch.id,
			uid: branch.uid,
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
				return { id: bus.id, uid: bus.uid, lon, lat, demand_mw: bus.demand_mw, gen_mw: bus.gen_mw };
			}),
			branches
		},
		sourceLabel: geoFile.sourceNames.join(' + '),
		matchedBuses: buses.length,
		matchedBranches,
		warnings: geoFile.warnings
	};
}

function parseCsvGeoFile(geoFile: GeoFile, text: string) {
	const rows = parseCsv(text);
	if (rows.length === 0) throw new Error('empty CSV');
	for (const row of rows) {
		addPointRecord(geoFile, row);
		addBranchRecord(geoFile, row);
	}
}

function parseJsonGeoFile(geoFile: GeoFile, text: string) {
	const data = JSON.parse(text) as unknown;
	if (isFeatureCollection(data)) {
		for (const feature of data.features) addFeature(geoFile, feature);
		return;
	}
	for (const record of collectRecords(data)) {
		addPointRecord(geoFile, record);
		addBranchRecord(geoFile, record);
	}
}

function addFeature(geoFile: GeoFile, feature: JsonRecord) {
	const props = asRecord(feature.properties) ?? {};
	const geometry = asRecord(feature.geometry);
	if (!geometry) return;
	const type = String(geometry.type ?? '');
	if (type === 'Point') {
		const id = findNumber(props, ['bus_i', 'bus', 'bus_id', 'id', 'number']);
		const coord = firstCoord(geometry.coordinates);
		if (id !== null && coord) geoFile.busCoords.set(id, coord);
		return;
	}
	if (type === 'LineString') {
		const path = coordPath(geometry.coordinates);
		addBranchPath(geoFile, props, path);
		return;
	}
	if (type === 'MultiLineString') {
		const paths = Array.isArray(geometry.coordinates)
			? geometry.coordinates.map((p) => coordPath(p)).filter((p) => p.length >= 2)
			: [];
		addBranchEndpoints(geoFile, props, paths);
		warnOnce(
			geoFile,
			'GeoJSON MultiLineString branch paths are skipped; straight segments will be drawn'
		);
	}
}

function addPointRecord(geoFile: GeoFile, record: JsonRecord | CsvRecord) {
	const id = findNumber(record, ['bus_i', 'bus', 'bus_id', 'bus number', 'number', 'id']);
	const lat = findNumber(record, ['lat', 'latitude', 'y']);
	const lon = findNumber(record, ['lon', 'lng', 'longitude', 'x']);
	if (id === null || lat === null || lon === null) return;
	if (!validCoord(lon, lat)) return;
	geoFile.busCoords.set(id, [lon, lat]);
}

function addBranchRecord(geoFile: GeoFile, record: JsonRecord | CsvRecord) {
	const from = findNumber(record, ['f_bus', 'from', 'from_bus']);
	const to = findNumber(record, ['t_bus', 'to', 'to_bus']);
	const path = pathFromRecord(record);
	if (path.length < 2) return;
	addBranchPath(geoFile, record, path);
	if (from !== null && !geoFile.busCoords.has(from)) geoFile.busCoords.set(from, path[0]);
	if (to !== null && !geoFile.busCoords.has(to)) geoFile.busCoords.set(to, path[path.length - 1]);
}

function addBranchPath(geoFile: GeoFile, record: JsonRecord | CsvRecord, path: [number, number][]) {
	if (path.length < 2) return;
	const from = findNumber(record, ['f_bus', 'from', 'from_bus']);
	const to = findNumber(record, ['t_bus', 'to', 'to_bus']);
	const id = findNumber(record, ['branch', 'branch_id', 'branch number', 'cats_id', 'id']);
	if (id !== null) geoFile.branchPaths.set(branchKey(id), path);
	if (from !== null && to !== null) {
		geoFile.branchPaths.set(edgeKey(from, to), path);
		if (!geoFile.busCoords.has(from)) geoFile.busCoords.set(from, path[0]);
		if (!geoFile.busCoords.has(to)) geoFile.busCoords.set(to, path[path.length - 1]);
	}
}

function addBranchEndpoints(
	geoFile: GeoFile,
	record: JsonRecord | CsvRecord,
	paths: [number, number][][]
) {
	if (paths.length === 0) return;
	const from = findNumber(record, ['f_bus', 'from', 'from_bus']);
	const to = findNumber(record, ['t_bus', 'to', 'to_bus']);
	if (from === null || to === null) return;
	const first = paths[0][0];
	const lastPath = paths[paths.length - 1];
	const last = lastPath[lastPath.length - 1];
	if (!geoFile.busCoords.has(from)) geoFile.busCoords.set(from, first);
	if (!geoFile.busCoords.has(to)) geoFile.busCoords.set(to, last);
}

function warnOnce(geoFile: GeoFile, warning: string) {
	if (!geoFile.warnings.includes(warning)) geoFile.warnings.push(warning);
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
	const direct = Object.values(record).flatMap((value) =>
		Array.isArray(value) ? collectRecords(value) : []
	);
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
	return raw.map(firstCoord).filter((coord): coord is [number, number] => coord !== null);
}

function firstCoord(raw: unknown): [number, number] | null {
	if (!Array.isArray(raw) || raw.length < 2) return null;
	const lon = Number(raw[0]);
	const lat = Number(raw[1]);
	return validCoord(lon, lat) ? [lon, lat] : null;
}

function validCoord(lon: number, lat: number): boolean {
	return (
		Number.isFinite(lon) && Number.isFinite(lat) && Math.abs(lon) <= 180 && Math.abs(lat) <= 90
	);
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
