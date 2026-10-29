import type { PickingInfo } from '@deck.gl/core';
import type { DistGraph } from '@tellegen/engine';
import {
	ATTACHMENT_NAMES,
	bandPosition,
	conductorSet,
	formatPower,
	loadingState,
	terminalLabel,
	type DisplayTree,
	type EquipmentItem,
	type Feeders,
	type EquipmentSymbols,
	type GalvanicZone,
	type GalvanicZones,
	type Landmark,
	type ResultSnapshot,
	type ZoneState
} from '../../diagram/index.js';

/** What the diagram's words are made from: tooltips, announcements and the cards' rows. */
export interface TextContext {
	graph: DistGraph;
	zones: GalvanicZones;
	zoneStates: ZoneState[];
	/** Whether a zone is drawn as a network of its own (an LV network). */
	outlined: (zone: GalvanicZone) => boolean;
	transformerNames: Map<number, string>;
	live: { edges: ArrayLike<number> };
	equipment: EquipmentSymbols | null;
	snapshot: ResultSnapshot | null;
	powers: Map<string, { p: number; q: number }> | null;
	valuesOn: boolean;
	edgeMv: Uint8Array;
	zoneMv: boolean[];
	/** Per bus, for source positions: 1 drawn next to a connected bus, 2 left at the origin. */
	inferred: Uint8Array | null;
	feeders: Feeders;
	tree: DisplayTree;
	/** Short bus names, as drawn. */
	busNames: string[];
	/** Close enough for terminal and transformer labels beside the symbols. */
	labelsOn: boolean;
}

const list = (ids: string[]) =>
	ids.slice(0, 4).join(', ') + (ids.length > 4 ? ` +${ids.length - 4}` : '');
const counted = (n: number, name: string) => (n > 1 ? `${n} ${name}s` : name);
const volts = (x: number, digits: number) =>
	x >= 1000 ? `${(x / 1000).toFixed(digits)} kV` : `${x.toFixed(1)} V`;
const attachmentKinds = (graph: DistGraph, i: number) =>
	new Set(
		Object.values(graph.buses[i].terminal_attachments ?? {}).flatMap((l) => l.map((a) => a.kind))
	);

export class SchematicText {
	constructor(private readonly c: TextContext) {}

	/** What supplies a network at bus `s`: its feeding transformer, or the source bus itself. */
	private supplyName(s: number): string {
		const { tree, graph, transformerNames, busNames } = this.c;
		const k = tree.parentEdge[s];
		return k >= 0 ? (transformerNames.get(k) ?? graph.edges[k].id) : busNames[s];
	}

	/** "feeder 2 of TX 1234", "feeder 1 of SOURCEBUS": the same words on MV and LV. */
	feederName(f: number): string {
		const feeder = this.c.feeders.feeders[f];
		return `feeder ${feeder.number} of ${this.supplyName(feeder.supply)}`;
	}

	/** Where a bus sits: on a feeder, or as the bus a network is supplied at. */
	networkNote(i: number): string {
		const f = this.c.feeders.busFeeder[i];
		if (f >= 0) return ` · ${this.feederName(f)}`;
		return this.c.tree.parentEdge[i] >= 0
			? ` · supply bus of the network fed by ${this.supplyName(i)}`
			: ' · source bus';
	}

	/** A bus's worst pair voltage against its band, for announcements and the stack card. */
	voltageNote(i: number): string | null {
		const pairs = this.c.snapshot?.buses[i]?.pairs ?? [];
		if (!pairs.length) return null;
		const worst = pairs.reduce((a, b) =>
			Math.abs(bandPosition(b) ?? 0) > Math.abs(bandPosition(a) ?? 0) ? b : a
		);
		const t = bandPosition(worst);
		const band =
			worst.min !== null && worst.max !== null
				? `, ${t !== null && Math.abs(t) > 1 ? 'outside' : 'within'} ${volts(worst.min, 3)} to ${volts(worst.max, 3)}`
				: '';
		return `Worst voltage ${worst.pair.join('–')} ${volts(worst.volts, 3)}${band}`;
	}

	bus(i: number): string {
		const how = this.c.inferred?.[i] ?? 0;
		const id = this.c.graph.buses[i].id + this.networkNote(i);
		return how === 1
			? `${id} · no position in the case data; drawn next to a connected bus`
			: how === 2
				? `${id} · no position in the case data`
				: id;
	}

	/** Whether a branch is drawn on a route shared with others. */
	isShared(k: number): boolean {
		return (this.c.equipment?.routeOf[k] ?? -1) >= 0;
	}

	branch(k: number): string {
		const { graph, live, equipment } = this.c;
		const e = graph.edges[k];
		const { phases, neutral } = conductorSet(graph, k);
		const wires = `${phases} phase${phases === 1 ? '' : 's'}${neutral ? ' + neutral' : ''}`;
		const shared = equipment?.sharedRoute[k] ?? 1;
		const route =
			shared > 1
				? ` · drawn on a route shared with ${shared - 1} other branch${shared > 2 ? 'es' : ''}`
				: '';
		const f = this.c.feeders.edgeFeeder[k];
		const feeder = f >= 0 ? ` · ${this.feederName(f)}` : '';
		return `${e.kind} ${e.id} · ${wires}${e.closed ? '' : ' · open'}${live.edges[k] ? '' : ' · not energised'}${feeder}${route}`;
	}

	item(d: EquipmentItem): string {
		const { graph } = this.c;
		const name = d.kind in ATTACHMENT_NAMES ? ATTACHMENT_NAMES[d.kind as 'load'] : d.kind;
		if (d.kind === 'ground')
			return `solid earth: terminal${d.ids.length > 1 ? 's' : ''} ${d.ids.join(', ')} of ${graph.buses[d.bus].id} perfectly grounded`;
		const terminals = d.terminals ? ` · terminals ${terminalLabel(d.terminals, d.connection)}` : '';
		const label = d.label ? ` · ${d.label}` : '';
		return d.bus >= 0
			? `${counted(d.ids.length, name)} ${list(d.ids)} at ${graph.buses[d.bus].id}${terminals}${this.networkNote(d.bus)}`
			: `${name} ${list(d.ids)}${label}${graph.edges[d.edge].closed ? '' : ' (open)'}`;
	}

	/** An attachment's text beside its symbol: its count when several, and its solved P and Q when
	 * values show. Generators, IBRs and sources read as output; loads and shunts as consumption. */
	itemValue(d: EquipmentItem): string {
		const { valuesOn, powers, labelsOn } = this.c;
		const parts: string[] = [];
		if (labelsOn && d.terminals) parts.push(terminalLabel(d.terminals, d.connection));
		if (d.ids.length > 1) parts.push(`×${d.ids.length}`);
		if (valuesOn && powers) {
			let p = 0;
			let q = 0;
			let found = false;
			for (const id of d.ids) {
				const row = powers.get(id);
				if (!row) continue;
				p += row.p;
				q += row.q;
				found = true;
			}
			const output = d.kind === 'generator' || d.kind === 'ibr' || d.kind === 'source';
			// Below 1 VA (an earthing shunt, say) the value is noise, not information.
			if (found && Math.hypot(p, q) >= 1)
				parts.push(output ? formatPower(-p, -q) : formatPower(p, q));
		}
		return parts.join(' ');
	}

	landmark(l: Landmark): string {
		const name = {
			source: 'source',
			generator: 'generator',
			ibr: 'inverter-based resource',
			transformer: 'transformer'
		}[l.kind];
		const where = l.bus >= 0 ? ` at ${this.c.graph.buses[l.bus].id}` : '';
		return `${counted(l.ids.length, name)} ${list(l.ids)}${where}`;
	}

	zone(z: number): string {
		const { zones, zoneStates, graph } = this.c;
		const zone = zones.zones[z];
		const t = zone.transformers[0];
		const verdict = {
			violation: 'a limit is violated',
			warning: 'a branch is near its rating',
			normal: 'within limits',
			missing: 'no result'
		}[zoneStates[z]];
		const fed =
			t === undefined ? 'Network' : `Network fed by ${graph.edges[t].kind} ${graph.edges[t].id}`;
		return `${fed}: ${zone.buses.length} buses, ${verdict}`;
	}

	/** Rows of a shared route's card. */
	routeRows(members: number[]) {
		const { graph, snapshot, edgeMv } = this.c;
		return members.map((k) => {
			const e = graph.edges[k];
			const { phases, neutral } = conductorSet(graph, k);
			const loading = snapshot?.branches[k]?.loading ?? null;
			const state = loadingState(loading);
			return {
				k,
				id: e.id,
				kind: e.kind,
				level: edgeMv[k] ? 'MV' : 'LV',
				wires: `${phases}${neutral ? '+N' : ''}`,
				loading: loading === null ? null : `${loading.toFixed(0)} %`,
				state: state === 'missing' ? 'normal' : state
			};
		});
	}

	/** Rows of a bus stack's card. */
	stackRows(members: number[]) {
		const { graph, zones, zoneMv } = this.c;
		return members.map((i) => {
			const note = this.voltageNote(i);
			return {
				i,
				id: graph.buses[i].id,
				level: zoneMv[zones.busZone[i]] ? 'MV' : 'LV',
				attached: [...attachmentKinds(graph, i)].join(', '),
				voltage: note ? note.replace(/^Worst voltage /, '').replace(/, (within|outside).*/, '') : ''
			};
		});
	}
}

/** Picks and how the canvas reads them, for the tooltip. */
export interface TooltipPicks {
	/** Whether this pick belongs to the stack whose card is open (the card names its buses). */
	inOpenStack: (info: PickingInfo) => boolean;
	/** Edge of a drawn branch segment. */
	segmentEdge: (index: number) => number;
	/** Whether symbols show, so shared routes are named by their card. */
	symbolsOn: boolean;
}

/** The tooltip for a pick, or null where a card or nothing says it better. */
export function tooltip(
	info: PickingInfo,
	text: SchematicText,
	picks: TooltipPicks
): string | null {
	const id = info.layer?.id;
	if (info.index < 0 || !id) return null;
	// A stack's ring opens its card, which says the rest; inside an open stack the card names the
	// buses, and a tooltip would cover them. A fanned route's card names its branches.
	if (id === 'stack-rings' || picks.inOpenStack(info) || id === 'fan-branches') return null;
	if (id === 'route-badges')
		return `${(info.object as { n: number }).n} branches share this route. Hover to separate them; click to keep them open.`;
	if (id === 'landmark-halos') return text.landmark(info.object as Landmark);
	if (id.startsWith('zones'))
		return `${text.zone((info.object as { z: number }).z)}. Click to fit.`;
	if (id.startsWith('buses-')) return text.bus(info.object as number);
	if (id.startsWith('branches-')) {
		const k = picks.segmentEdge(info.index);
		// A shared route's card names its branches; its hidden true route stays silent.
		return picks.symbolsOn && text.isShared(k) ? null : text.branch(k);
	}
	if (id === 'equipment-hits') return text.item(info.object as EquipmentItem);
	return null;
}
