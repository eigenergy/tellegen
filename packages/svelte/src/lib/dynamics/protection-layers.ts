/** The protection of the active case on the map: a relay marker at each
 * protected line end (green in service, amber picked up, red tripped), the
 * distance zones of the selected relay as coloured stretches along the
 * lines (Z1 green, Z2 amber, Z3 orange; every relay's Z1 faintly when none
 * is selected), faults inside lines, and the breakers the relays opened up
 * to the time shown. Pure geometry plus light deck.gl layers. */

import type { Layer } from '@deck.gl/core';
import type { RelayOperation } from '@tellegen/engine';
import type { LayerCtors } from './layers.js';
import type { ProtectionState, RelayView } from './protection.svelte.js';

type Rgba = [number, number, number, number];
type Pt = [number, number];

const ZONE: Record<string, Rgba> = {
	Z1: [46, 160, 90, 230],
	Z2: [230, 168, 23, 220],
	Z3: [232, 112, 40, 200]
};
const RED: Rgba = [227, 73, 72, 255];

/** The part of a polyline between fractions `a` and `b` of its length. */
export function subPath(path: Pt[], a: number, b: number): Pt[] {
	if (path.length < 2) return path;
	const seg = path.slice(1).map((p, i) => Math.hypot(p[0] - path[i][0], p[1] - path[i][1]));
	const total = seg.reduce((s, v) => s + v, 0) || 1;
	const at = (u: number): Pt => {
		let d = Math.max(0, Math.min(1, u)) * total;
		for (let i = 0; i < seg.length; i++) {
			if (d <= seg[i] || i === seg.length - 1) {
				const f = seg[i] > 0 ? Math.min(1, d / seg[i]) : 0;
				return [
					path[i][0] + f * (path[i + 1][0] - path[i][0]),
					path[i][1] + f * (path[i + 1][1] - path[i][1])
				];
			}
			d -= seg[i];
		}
		return path[path.length - 1];
	};
	const out: Pt[] = [at(a)];
	let acc = 0;
	for (let i = 0; i < seg.length; i++) {
		acc += seg[i];
		const u = acc / total;
		if (u > a && u < b) out.push(path[i + 1]);
	}
	out.push(at(b));
	return out;
}

/** Each relay's state at time `t` from a run's operations (in time order). */
export function relayStates(ops: RelayOperation[], t: number): Map<string, 'picked' | 'tripped'> {
	const out = new Map<string, 'picked' | 'tripped'>();
	const picked = new Map<string, number>();
	for (const o of ops) {
		if (o.time > t) break;
		if (o.kind === 'trip') out.set(o.relay, 'tripped');
		else if (o.kind === 'pickup') picked.set(o.relay, (picked.get(o.relay) ?? 0) + 1);
		else if (o.kind === 'reset') picked.set(o.relay, Math.max(0, (picked.get(o.relay) ?? 0) - 1));
		else if (o.kind === 'reclose') out.delete(o.relay);
	}
	for (const [r, n] of picked) if (n > 0 && !out.has(r)) out.set(r, 'picked');
	return out;
}

/** What the protection layers draw, in the coordinates of the branch paths. */
export interface ProtectionGeometry {
	markers: { id: string; at: Pt; state: string }[];
	spans: { path: Pt[]; color: Rgba; width: number; zone: string }[];
	faults: { at: Pt; active: boolean }[];
	/** The opened end's stretch of a line (red). */
	open: { path: Pt[] }[];
	/** Whole lines with both ends open (greyed, de-energized). */
	dead: { path: Pt[] }[];
	/** Open breakers. */
	breakers: { at: Pt }[];
	/** Trips of the last 0.6 s: a ring that grows and fades (`age` 0…1). */
	flashes: { at: Pt; age: number }[];
	/** Tripped busbars and units (red rings at their bus). */
	tripped: { at: Pt; kind: 'busbar' | 'unit' }[];
}

const FLASH_S = 0.6;

export function protectionGeometry(
	p: ProtectionState,
	view: RelayView,
	selected: string | null,
	branchesOf: { from: number; to: number; path: Pt[] }[],
	busAt: (bus: number) => Pt | undefined
): ProtectionGeometry {
	const branchPath = (from: number, to: number): { path: Pt[]; flip: boolean } | null => {
		const b = branchesOf.find(
			(x) => (x.from === from && x.to === to) || (x.from === to && x.to === from)
		);
		return b ? { path: b.path, flip: b.from !== from } : null;
	};
	const t = view.t;
	const ops = view.ops;
	const states = relayStates(ops, t);

	// Relay markers a little inside each protected line end.
	const markers = p.scheme.relays.flatMap((r) => {
		if (r.location.kind !== 'line') return [];
		const { from, to, at } = r.location;
		const bp = branchPath(from, to);
		if (!bp) return [];
		const atFrom = (at === from) !== bp.flip;
		const u = atFrom ? 0.12 : 0.88;
		const state = states.get(r.id) ?? (r.enabled === false ? 'off' : 'idle');
		return [{ id: r.id, at: subPath(bp.path, u, u)[0], state }];
	});

	// Zones of the selected relay (every relay's Z1 faintly when none is).
	const spans = p.zones.flatMap((z) => {
		if (selected ? z.relay !== selected : z.zone !== 'Z1') return [];
		const bp = branchPath(z.from, z.to);
		if (!bp) return [];
		const [a, b] = bp.flip ? [1 - z.end, 1 - z.start] : [z.start, z.end];
		const color = ZONE[z.zone] ?? ([120, 120, 120, 200] as Rgba);
		return [
			{
				path: subPath(bp.path, a, b),
				color: selected ? color : ([color[0], color[1], color[2], 90] as Rgba),
				width: selected ? (z.zone === 'Z1' ? 9 : z.zone === 'Z2' ? 6 : 4) : 3,
				zone: z.zone
			}
		];
	});

	// Faults inside lines.
	const events = view.events;
	const faults = events.flatMap((e) => {
		if (e.kind !== 'line_fault') return [];
		const bp = branchPath(e.from, e.to);
		if (!bp) return [];
		const u = bp.flip ? 1 - e.location : e.location;
		const clear = events.find(
			(x) => x.kind === 'clear_line_fault' && x.from === e.from && x.to === e.to && x.time >= e.time
		);
		const active = t >= e.time && (!clear || t < clear.time);
		return [{ at: subPath(bp.path, u, u)[0], active }];
	});

	// Breakers open at time t (a reclose closes them again).
	const isOpen = (o: RelayOperation) =>
		o.kind === 'breaker_open' &&
		o.time <= t &&
		!ops.some((x) => x.relay === o.relay && x.kind === 'reclose' && x.time > o.time && x.time <= t);
	const open: ProtectionGeometry['open'] = [];
	const dead: ProtectionGeometry['dead'] = [];
	const breakers: ProtectionGeometry['breakers'] = [];
	const tripped: ProtectionGeometry['tripped'] = [];
	const openEnds = new Map<string, { path: Pt[]; ends: Set<boolean> }>();
	const relays = new Map(p.scheme.relays.map((r) => [r.id, r]));
	for (const o of ops) {
		if (!isOpen(o)) continue;
		const r = relays.get(o.relay);
		if (!r) continue;
		const loc = r.location;
		if (loc.kind === 'line' || loc.kind === 'transformer') {
			const bp = branchPath(loc.from, loc.to);
			if (!bp) continue;
			// A line relay opens its own end; a transformer relay both.
			const ends = loc.kind === 'line' ? [(loc.at === loc.from) !== bp.flip] : [true, false];
			for (const atFrom of ends) {
				open.push({ path: subPath(bp.path, atFrom ? 0 : 0.85, atFrom ? 0.15 : 1) });
				const u = atFrom ? 0.06 : 0.94;
				breakers.push({ at: subPath(bp.path, u, u)[0] });
			}
			const key = `${Math.min(loc.from, loc.to)}-${Math.max(loc.from, loc.to)}`;
			const entry = openEnds.get(key) ?? { path: bp.path, ends: new Set<boolean>() };
			for (const e of ends) entry.ends.add(e);
			openEnds.set(key, entry);
		} else {
			const at = busAt(loc.bus);
			if (at) tripped.push({ at, kind: loc.kind === 'busbar' ? 'busbar' : 'unit' });
		}
	}
	for (const { path, ends } of openEnds.values()) if (ends.size === 2) dead.push({ path });

	// Trip flashes.
	const markerAt = new Map(markers.map((m) => [m.id, m.at]));
	const flashes = Number.isFinite(t)
		? ops.flatMap((o) => {
				if (o.kind !== 'trip' || o.time > t || t - o.time > FLASH_S) return [];
				const r = relays.get(o.relay);
				if (!r) return [];
				const at =
					markerAt.get(r.id) ??
					('bus' in r.location ? busAt(r.location.bus) : busAt(r.location.from));
				return at ? [{ at, age: (t - o.time) / FLASH_S }] : [];
			})
		: [];
	return { markers, spans, faults, open, dead, breakers, flashes, tripped };
}

export const relayColor = (s: string): Rgba =>
	s === 'tripped'
		? RED
		: s === 'picked'
			? [230, 168, 23, 255]
			: s === 'off'
				? [150, 150, 140, 200]
				: [46, 160, 90, 255];

/** The layer id prefix of the relay markers (tooltips, picking). */
export const RELAY_LAYER = 'prot-relays';

export function protectionLayers(
	p: ProtectionState,
	view: RelayView,
	selected: string | null,
	buses: { id: number; lon: number; lat: number }[],
	branches: { from: number; to: number; path: [number, number][] }[],
	ctors: LayerCtors,
	onrelay?: (id: string) => void
): Layer[] {
	const pos = new Map(buses.map((b) => [b.id, [b.lon, b.lat] as Pt]));
	const geo = protectionGeometry(
		p,
		view,
		selected,
		branches.map((b) => ({ from: b.from, to: b.to, path: b.path as Pt[] })),
		(bus) => pos.get(bus)
	);
	const { markers, spans, faults, open, dead, breakers, flashes, tripped } = geo;
	const trigger = [view.kind, view.t, view.ops.length, selected, p];
	return [
		new ctors.PathLayer<{ path: Pt[] }>({
			id: 'prot-dead',
			data: dead,
			getPath: (d) => d.path,
			getColor: [120, 120, 112, 170],
			getWidth: 7,
			widthUnits: 'pixels',
			capRounded: true,
			updateTriggers: { getPath: trigger }
		}),
		new ctors.PathLayer<{ path: Pt[]; color: Rgba; width: number }>({
			id: 'prot-zones',
			data: spans,
			getPath: (d) => d.path,
			getColor: (d) => d.color,
			getWidth: (d) => d.width,
			widthUnits: 'pixels',
			capRounded: true,
			updateTriggers: { getPath: trigger, getColor: trigger, getWidth: trigger }
		}),
		new ctors.PathLayer<{ path: Pt[] }>({
			id: 'prot-open',
			data: open,
			getPath: (d) => d.path,
			getColor: RED,
			getWidth: 5,
			widthUnits: 'pixels',
			updateTriggers: { getPath: trigger }
		}),
		new ctors.ScatterplotLayer<{ id: string; at: Pt; state: string }>({
			id: RELAY_LAYER,
			data: markers,
			pickable: true,
			onClick: (info) => {
				if (!info.object || !onrelay) return false;
				onrelay(info.object.id);
				return true;
			},
			getPosition: (d) => d.at,
			getRadius: (d) => (d.id === selected ? 7 : d.state === 'tripped' ? 6 : 4),
			radiusUnits: 'pixels',
			getFillColor: (d) => relayColor(d.state),
			stroked: true,
			getLineColor: (d) => (d.id === selected ? [30, 30, 30, 255] : [255, 255, 255, 230]),
			getLineWidth: (d) => (d.id === selected ? 2 : 1),
			lineWidthUnits: 'pixels',
			billboard: true,
			updateTriggers: {
				getFillColor: trigger,
				getRadius: trigger,
				getLineColor: trigger,
				getLineWidth: trigger
			}
		}),
		new ctors.ScatterplotLayer<{ at: Pt; age: number }>({
			id: 'prot-flash',
			data: flashes,
			getPosition: (d) => d.at,
			getRadius: (d) => 10 + 46 * d.age,
			radiusUnits: 'pixels',
			filled: true,
			getFillColor: (d) => [227, 73, 72, Math.round(110 * (1 - d.age))],
			stroked: true,
			getLineColor: (d) => [227, 73, 72, Math.round(255 * (1 - d.age))],
			getLineWidth: 3,
			lineWidthUnits: 'pixels',
			billboard: true,
			updateTriggers: { getRadius: trigger, getFillColor: trigger, getLineColor: trigger }
		}),
		new ctors.ScatterplotLayer<{ at: Pt; kind: string }>({
			id: 'prot-tripped',
			data: tripped,
			getPosition: (d) => d.at,
			getRadius: (d) => (d.kind === 'busbar' ? 20 : 26),
			radiusUnits: 'pixels',
			filled: false,
			stroked: true,
			getLineColor: RED,
			getLineWidth: (d) => (d.kind === 'busbar' ? 4 : 3),
			lineWidthUnits: 'pixels',
			billboard: true,
			updateTriggers: { getRadius: trigger, getLineWidth: trigger }
		}),
		new ctors.ScatterplotLayer<{ at: Pt }>({
			id: 'prot-breakers',
			data: breakers,
			getPosition: (d) => d.at,
			getRadius: 7,
			radiusUnits: 'pixels',
			getFillColor: [255, 255, 255, 255],
			stroked: true,
			getLineColor: RED,
			getLineWidth: 3,
			lineWidthUnits: 'pixels',
			billboard: true,
			updateTriggers: { getPosition: trigger }
		}),
		new ctors.ScatterplotLayer<{ at: Pt; active: boolean }>({
			id: 'prot-faults',
			data: faults,
			getPosition: (d) => d.at,
			getRadius: (d) => (d.active ? 14 : 8),
			radiusUnits: 'pixels',
			getFillColor: (d) => (d.active ? [227, 73, 72, 140] : [0, 0, 0, 0]),
			stroked: true,
			getLineColor: RED,
			getLineWidth: 2,
			lineWidthUnits: 'pixels',
			billboard: true,
			updateTriggers: { getRadius: trigger, getFillColor: trigger }
		})
	];
}
