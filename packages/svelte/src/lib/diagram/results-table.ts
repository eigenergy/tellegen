import { worstBand } from './colour-modes.js';
import type { ResultSnapshot } from './snapshot.js';
import { pairVoltageLabel, type PairVoltage } from './snapshot.js';
import { busState, busUnbalance, loadingState } from './summary.js';
import { phaseLabel } from '../multiconductor.js';

export type TableKind = 'bus' | 'branch';
type Cell = string | number | null;
export type TableRow = Record<string, Cell> & { state: string };

export interface TableColumn {
	key: string;
	label: string;
	unit?: string;
	numeric?: boolean;
	digits?: number;
	title?: string;
}

export interface TableNames {
	buses: { id: string; source?: string }[];
	branches: { id: string; family: string; from: number; to: number; inService: boolean }[];
}

export interface TableModel {
	kind: TableKind;
	count: number;
	columns: TableColumn[];
	row(index: number): TableRow;
}

const BUS_COLUMNS: TableColumn[] = [
	{ key: 'id', label: 'Bus' },
	{ key: 'source', label: 'Source' },
	{ key: 'reference', label: 'Reference' },
	{ key: 'pairs', label: 'Pairs' },
	{ key: 'v0', label: 'V A', unit: 'V', numeric: true },
	{ key: 'v1', label: 'V B', unit: 'V', numeric: true },
	{ key: 'v2', label: 'V C', unit: 'V', numeric: true },
	{
		key: 'band',
		label: 'Band position',
		unit: '±1 = limit',
		numeric: true,
		digits: 3,
		title:
			'Worst pair: −1 at the declared minimum, +1 at the maximum; sorts by distance from mid-band'
	},
	{ key: 'unbalance', label: 'Unbalance', unit: '%', numeric: true, digits: 1 },
	{ key: 'unbalanceBasis', label: 'Basis' },
	{ key: 'state', label: 'Status' }
];

const BRANCH_COLUMNS: TableColumn[] = [
	{ key: 'id', label: 'Branch' },
	{ key: 'family', label: 'Type' },
	{ key: 'from', label: 'From bus' },
	{ key: 'to', label: 'To bus' },
	{
		key: 'pFrom',
		label: 'P from',
		unit: 'kW',
		numeric: true,
		title: 'Positive into the equipment'
	},
	{ key: 'qFrom', label: 'Q from', unit: 'kvar', numeric: true },
	{
		key: 'pTo',
		label: 'P to',
		unit: 'kW',
		numeric: true,
		title: 'Positive into the equipment'
	},
	{ key: 'qTo', label: 'Q to', unit: 'kvar', numeric: true },
	{
		key: 'loss',
		label: 'Losses',
		unit: 'kW',
		numeric: true,
		title: 'Sum of both end P'
	},
	{ key: 'loading', label: 'Loading', unit: '%', numeric: true, digits: 1 },
	{ key: 'state', label: 'Status' }
];

const PHASE_SLOT: Record<string, number> = { A: 0, B: 1, C: 2 };

/** A bus's pairs in the three voltage columns, by the phase each pair starts from (phase A in the
 * first column), so a single-phase bus on phase B lands under B. A pair whose first terminal is
 * not a phase fills the first free column. The first pair for a phase wins. */
function voltageSlots(pairs: PairVoltage[]): (PairVoltage | null)[] {
	const slots: (PairVoltage | null)[] = [null, null, null];
	const rest: PairVoltage[] = [];
	for (const p of pairs) {
		const slot = PHASE_SLOT[phaseLabel(p.pair[0])];
		if (slot === undefined) rest.push(p);
		else if (!slots[slot]) slots[slot] = p;
	}
	for (const p of rest) {
		const free = slots.indexOf(null);
		if (free < 0) break;
		slots[free] = p;
	}
	return slots;
}

/** Name each voltage column after the pair its rows share (`V AN`, `V AB`, `V AG`); a column
 * whose rows mix references keeps the phase alone (`V A`). Never `V1`, which reads as the
 * positive-sequence voltage. */
function busColumns(result: ResultSnapshot): TableColumn[] {
	const seen: (string | null | undefined)[] = [undefined, undefined, undefined];
	for (const bus of result.buses) {
		voltageSlots(bus.pairs).forEach((p, k) => {
			if (!p) return;
			const name = pairVoltageLabel(p).replace('–', '');
			seen[k] = seen[k] === undefined || seen[k] === name ? name : null;
		});
	}
	return BUS_COLUMNS.map((c) => {
		const k = ['v0', 'v1', 'v2'].indexOf(c.key);
		const name = k < 0 ? undefined : seen[k];
		return name
			? {
					...c,
					label: `V ${name}`,
					title: `${name[0]}–${name.slice(1)} voltage magnitude`
				}
			: k < 0
				? c
				: { ...c, title: `Phase ${'ABC'[k]} pair voltage; the Pairs column names its reference` };
	});
}

/** Rows are built on demand and cached in a bounded window, so 100k-row snapshots stay cheap. */
export function tableModel(result: ResultSnapshot, names: TableNames, kind: TableKind): TableModel {
	const cache = new Map<number, TableRow>();
	const build = (i: number): TableRow => {
		if (kind === 'bus') {
			const bus = result.buses[i];
			const u = busUnbalance(bus);
			const slots = voltageSlots(bus.pairs);
			return {
				id: names.buses[i].id,
				source: names.buses[i].source ?? null,
				reference: bus.pairs.length ? [...new Set(bus.pairs.map((p) => p.zone))].join(', ') : null,
				// Source terminal names, so a CSV round-trips with the case data; the table shows
				// `pairLabels` (A–N) on screen.
				pairs: bus.pairs.map((p) => p.pair.join('–')).join(' ') || null,
				pairLabels: bus.pairs.map(pairVoltageLabel).join(' ') || null,
				v0: slots[0]?.volts ?? null,
				v1: slots[1]?.volts ?? null,
				v2: slots[2]?.volts ?? null,
				band: worstBand(bus.pairs),
				unbalance: u?.percent ?? null,
				unbalanceBasis: u?.definition ?? null,
				state: busState(bus)
			};
		}
		const name = names.branches[i];
		const { ends, loading, loss } = result.branches[i];
		const [a, b] = ends;
		return {
			id: name.id,
			family: name.family,
			from: names.buses[name.from]?.id ?? null,
			to: names.buses[name.to]?.id ?? null,
			pFrom: a?.p ?? null,
			qFrom: a?.q ?? null,
			pTo: b?.p ?? null,
			qTo: b?.q ?? null,
			// The equipment's loss, on its first edge only, so the column sums to the total.
			loss: loss?.p ?? null,
			loading,
			state: name.inService ? loadingState(loading) : 'inactive'
		};
	};
	return {
		kind,
		count: kind === 'bus' ? result.buses.length : result.branches.length,
		columns: kind === 'bus' ? busColumns(result) : BRANCH_COLUMNS,
		row(i) {
			let row = cache.get(i);
			if (!row) {
				if (cache.size > 4096) cache.clear();
				row = build(i);
				cache.set(i, row);
			}
			return row;
		}
	};
}

const ISSUE_STATES: Record<TableKind, Set<string>> = {
	bus: new Set(['violation']),
	branch: new Set(['violation', 'warning'])
};
const isMissing = (v: Cell) => v === null || (typeof v === 'number' && Number.isNaN(v));

export interface TableQuery {
	text?: string;
	issuesOnly?: boolean;
	sortKey?: string | null;
	descending?: boolean;
}

/** Filter by text over identities and optionally issues only, then a stable sort with missing
 * values last in both directions. */
export function queryTable(model: TableModel, q: TableQuery = {}): Int32Array {
	const text = (q.text ?? '').trim().toLowerCase();
	const out: number[] = [];
	for (let i = 0; i < model.count; i++) {
		const row = model.row(i);
		if (q.issuesOnly && !ISSUE_STATES[model.kind].has(row.state)) continue;
		if (
			text &&
			!['id', 'source', 'from', 'to', 'family'].some((k) =>
				String(row[k] ?? '')
					.toLowerCase()
					.includes(text)
			)
		)
			continue;
		out.push(i);
	}
	const column = model.columns.find((c) => c.key === q.sortKey);
	if (column) {
		const key = column.key;
		const sign = q.descending ? -1 : 1;
		const values = new Map(out.map((i) => [i, model.row(i)[key]]));
		out.sort((a, b) => {
			const x = values.get(a) ?? null;
			const y = values.get(b) ?? null;
			if (isMissing(x) || isMissing(y))
				return isMissing(x) === isMissing(y) ? a - b : isMissing(x) ? 1 : -1;
			const order = column.numeric
				? key === 'band'
					? Math.abs(x as number) - Math.abs(y as number)
					: (x as number) - (y as number)
				: String(x).localeCompare(String(y));
			return order * sign || a - b;
		});
	}
	return Int32Array.from(out);
}

/** Text a spreadsheet would evaluate as a formula gets a leading apostrophe; numbers stay numeric. */
function csvCell(v: Cell): string {
	if (v === null || v === undefined) return '';
	let s = String(v);
	if (typeof v !== 'number' && /^[=+\-@\t\r]/.test(s)) s = `'${s}`;
	return /[",\n\r']/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** Full-precision CSV of the given rows, units in the header, provenance as `#` comment lines. */
export function tableCsv(
	model: TableModel,
	indices: Iterable<number>,
	provenance: string[] = []
): string {
	const lines = [
		...provenance.map((line) => `# ${line}`),
		model.columns.map((c) => csvCell(c.unit ? `${c.label} (${c.unit})` : c.label)).join(',')
	];
	for (const i of indices) {
		const row = model.row(i);
		lines.push(model.columns.map((c) => csvCell(row[c.key])).join(','));
	}
	return lines.join('\r\n') + '\r\n';
}
