import type { Network, NetworkShunt } from '@tellegen/engine';
import { attachmentGlyph, branchGlyph, type Glyph } from './schematic-equipment.js';
import type { TxGraph, TxSchematicParts } from './transmission.js';
import type { SchematicGeometry } from './schematic-layout.js';

export type EquipmentKind =
	| 'capacitor'
	| 'reactor'
	| 'shunt'
	| 'pst'
	| 'transformer'
	| 'hvdc'
	| 'three_winding'
	| 'svc'
	| 'storage'
	| 'switch';
export interface EquipmentItem {
	key: string;
	kind: EquipmentKind;
	title: string;
	state: string;
	bus?: number;
	branch?: number;
	facts: string[];
	explanation: string;
}
const number = (n: number) => Number(n.toPrecision(6)).toString();

/** Treatment is tied to the selected formulation, separately from equipment identity/state. */
export function equipmentTreatment(kind: EquipmentKind, formulation: string): string {
	if (kind === 'hvdc')
		return formulation === 'acopf' || formulation === 'socwr'
			? 'Active links are unsupported in this formulation.'
			: 'Fixed terminal injections; DC voltages, currents and controls are not solved in this formulation.';
	if (kind === 'svc')
		return formulation === 'dcopf' || formulation === 'dcpf'
			? 'DC approximation: static VAR compensator reactive controls are omitted and reported in diagnostics.'
			: 'Active static VAR compensator controls are unsupported in AC calculations.';
	if (kind === 'storage') return 'Active devices are unsupported; the solve must reject them.';
	if (kind === 'switch')
		return 'Open switches isolate their terminals; closed switches require topology reduction.';
	if (kind === 'three_winding')
		return 'Modeled by a three-branch star equivalent with an internal analysis bus.';
	if (kind === 'capacitor' || kind === 'reactor' || kind === 'shunt')
		return formulation === 'dcopf' || formulation === 'dcpf'
			? 'DC approximation: conductance enters active balance; reactive susceptance is omitted.'
			: 'Fixed admittance; active/reactive contribution depends on voltage squared. Use the separate automatic regulator study to switch declared sections.';
	return 'Fixed tap ratio and phase setting; use the separate automatic regulator study to adjust enabled source controls.';
}

/** Infer only a pure reactive bank. Mixed conductive or mixed-step devices stay generic. */
export function shuntKind(s: NetworkShunt): 'capacitor' | 'reactor' | 'shunt' {
	if (s.g_mw !== 0 || s.blocks.some((b) => b.g_mw !== 0)) return 'shunt';
	const values = [s.b_mvar, ...s.blocks.filter((b) => b.steps > 0).map((b) => b.b_mvar)];
	if (values.some((v) => v > 0) && values.every((v) => v >= 0)) return 'capacitor';
	if (values.some((v) => v < 0) && values.every((v) => v <= 0)) return 'reactor';
	return 'shunt';
}

export function equipmentItems(network: Network): EquipmentItem[] {
	const e = network.equipment;
	if (!e) return [];
	const branches = new Map(network.branches.map((branch) => [branch.id, branch]));
	return [
		...(e.three_winding ?? []).map((t): EquipmentItem => ({
			key: `three-winding:${t.id}`,
			kind: 'three_winding',
			title: t.name ?? `Three-winding transformer ${t.id}`,
			state: t.in_service ? 'In service · star equivalent' : 'Out of service',
			bus: t.buses[0],
			facts: [
				`Winding buses: ${t.buses.join(', ')}`,
				...(t.star_bus === null
					? []
					: [`Analysis-only star bus ${t.star_bus}; winding branches ${t.branches.join(', ')}`])
			],
			explanation:
				'Three windings belong to one physical transformer. The calculation uses an internal star equivalent; its star bus is not a physical substation bus.'
		})),
		...(e.devices ?? []).map((d): EquipmentItem => ({
			key: d.key,
			kind: d.kind,
			title: `${d.kind === 'svc' ? 'Static VAR compensator' : d.kind === 'storage' ? 'Storage' : 'Switch'} ${d.key.split(':')[1]}`,
			state:
				d.kind === 'switch'
					? d.in_service
						? 'Closed · topology reduction required'
						: 'Open'
					: d.in_service
						? d.kind === 'svc'
							? 'In service · formulation-dependent treatment'
							: 'In service · unsupported by this solver'
						: 'Out of service',
			bus: d.buses[0],
			facts: [`Terminal buses: ${d.buses.join(', ')}`, `Calculation treatment: ${d.treatment}`],
			explanation:
				d.kind === 'svc'
					? 'A controllable reactive-power device. DC calculations omit its reactive controls and report that approximation; AC calculations reject active SVCs.'
					: d.kind === 'storage'
						? 'Storage can charge and discharge. Its energy balance and operating limits are not represented by these solvers; active devices are rejected.'
						: 'A switch connects or isolates its terminals. Closed switches require an explicit topology reduction before calculation.'
		})),
		...e.transformers.map((t): EquipmentItem => {
			const branch = branches.get(t.branch);
			return {
				key: `transformer:${t.branch}`,
				kind: t.phase_shifter ? 'pst' : 'transformer',
				title: `${t.phase_shifter ? 'PST' : 'Transformer'} · branch ${t.branch}`,
				state: branch
					? branch.status
						? 'In service'
						: 'Out of service'
					: 'Outside displayed network',
				branch: branch?.id,
				facts: [
					...(branch ? [`Bus ${branch.from} ↔ bus ${branch.to}`] : []),
					`Phase setting ${number(t.phase_shift_deg)}° · tap ratio ${number(t.tap_ratio)}`,
					...(t.tap_changers ?? []).map(
						(tap) =>
							`${tap.kind === 'phase' ? 'Phase' : 'Ratio'} tap changer · winding ${tap.winding} · assigned position ${tap.tap_position ?? 'unknown'} · ${tap.steps.length} recorded steps · source regulation ${tap.regulating ? 'enabled' : 'disabled'}`
					),
					`Source control: ${t.control_mode?.replaceAll('_', ' ') ?? 'not specified'}${t.control_enabled === null ? '' : t.control_enabled ? ' (enabled in source)' : ' (disabled in source)'}`,
					...(t.setting_min !== null && t.setting_max !== null
						? [
								`Setting range ${number(t.setting_min)} to ${number(t.setting_max)} ${t.phase_shifter ? 'degrees' : 'ratio'}`
							]
						: []),
					...(t.controlled_bus !== null ? [`Regulated bus ${t.controlled_bus}`] : [])
				],
				explanation: t.phase_shifter
					? 'A phase-shifting transformer changes phase angle to influence active-power sharing. This calculation uses the stated setting; the separate automatic regulator study can adjust a declared discrete phase control.'
					: 'A transformer changes voltage ratio and may introduce a winding phase shift. These calculations use fixed tap and phase settings. A phase shift alone does not identify a PST.'
			};
		}),
		...e.shunts.map((s): EquipmentItem => {
			const kind = shuntKind(s);
			return {
				key: `shunt:${s.id}`,
				kind,
				bus: s.bus,
				title: `${kind === 'capacitor' ? 'Capacitor bank' : kind === 'reactor' ? 'Shunt reactor' : 'Shunt admittance'} ${s.id}`,
				state: s.in_service ? 'In service' : 'Out of service',
				facts: [
					`Bus ${s.bus}`,
					`At 1 pu: B = ${number(s.b_mvar)} Mvar · G = ${number(s.g_mw)} MW`,
					...(s.section_count === null ? [] : [`${s.section_count} sections connected`]),
					...(s.blocks.length
						? [
								`Step blocks: ${s.blocks.map((b) => `${b.steps} × ${number(b.b_mvar)} Mvar`).join('; ')}`
							]
						: []),
					`Source switching: ${s.control_mode ?? 'not specified'}`
				],
				explanation:
					'Positive B supplies reactive power; negative B absorbs it. In AC calculations the contribution varies with voltage squared. DC power flow omits reactive power. Bank settings are fixed in the main calculation. The separate automatic regulator study can switch declared sections to reach a voltage band.'
			};
		}),
		...e.hvdc.map((h): EquipmentItem => ({
			key: `hvdc:${h.id}`,
			kind: 'hvdc',
			bus: h.from,
			title: `HVDC link ${h.id}`,
			state: h.in_service ? 'In service · fixed-transfer input' : 'Out of service',
			facts: [
				`Bus ${h.from} ↔ bus ${h.to}`,
				`Input withdrawal at bus ${h.from}: ${number(h.pf_mw)} MW`,
				`Input injection at bus ${h.to}: ${number(h.pt_mw)} MW`,
				`Net input loss: ${number(h.pf_mw - h.pt_mw)} MW`,
				`Converters: ${h.from_converter?.toUpperCase() ?? 'unspecified'} / ${h.to_converter?.toUpperCase() ?? 'unspecified'}`,
				...(h.nominal_kv ? [`Nominal DC voltage ${number(h.nominal_kv)} kV`] : [])
			],
			explanation:
				'DC PF, DC OPF and AC PF use fixed terminal injections; AC OPF and SOCWR reject active links. An excluded terminal can omit the whole transfer: check solve diagnostics. These are input powers, not solved DC voltages or currents. DC power flow is an AC-network approximation, not an HVDC-network simulation.'
		}))
	];
}

/** Compact diagram symbols; exact standard variants remain subject to symbol-library review. */
export function equipmentGlyph(kind: EquipmentKind, closed = false): Glyph {
	if (kind === 'switch') return branchGlyph('switch', closed)!;
	if (kind === 'svc')
		return formulation === 'dcopf' || formulation === 'dcpf'
			? 'DC approximation: static VAR compensator reactive controls are omitted and reported in diagnostics.'
			: 'Active static VAR compensator controls are unsupported in AC calculations.';
	if (kind === 'storage')
		return [
			[
				[0, 0],
				[0.4, 0]
			],
			[
				[0.4, -0.7],
				[1.8, -0.7],
				[1.8, 0.7],
				[0.4, 0.7],
				[0.4, -0.7]
			],
			...(kind === 'storage'
				? ([
						[
							[0.8, -0.4],
							[0.8, 0.4]
						],
						[
							[1.3, -0.2],
							[1.3, 0.2]
						]
					] as Glyph)
				: ([
						[
							[0.6, 0.4],
							[1.6, -0.4]
						]
					] as Glyph))
		];
	if (kind === 'three_winding') {
		const winding = (x: number, y: number): [number, number][] =>
			Array.from({ length: 25 }, (_, i) => [
				x + 0.7 * Math.cos((i * Math.PI) / 12),
				y + 0.7 * Math.sin((i * Math.PI) / 12)
			]);
		return [winding(-0.5, -0.3), winding(0.5, -0.3), winding(0, 0.55)];
	}
	if (kind === 'transformer') return branchGlyph('transformer', true)!;
	if (kind === 'pst')
		return [
			...branchGlyph('transformer', true)!,
			[
				[-0.9, 1],
				[0.9, -1]
			],
			[
				[0.3, -1],
				[0.9, -1],
				[0.9, -0.4]
			]
		];
	if (kind === 'shunt') return attachmentGlyph('shunt');
	if (kind === 'capacitor')
		return [
			[
				[0, 0],
				[0.9, 0]
			],
			[
				[0.9, -0.7],
				[0.9, 0.7]
			],
			[
				[1.3, -0.7],
				[1.3, 0.7]
			],
			[
				[1.3, 0],
				[2, 0]
			]
		];
	if (kind === 'reactor') {
		const coil: [number, number][] = [];
		for (let i = 0; i <= 32; i++)
			coil.push([0.5 + (1.2 * i) / 32, -0.45 * Math.sin((Math.PI * i) / 8)]);
		return [
			[
				[0, 0],
				[0.5, 0]
			],
			coil,
			[
				[1.7, 0],
				[2, 0]
			]
		];
	}
	// Two-port converter: AC mark at left, DC bars at right, with a separating diagonal.
	return [
		[
			[-1, -0.9],
			[1, -0.9],
			[1, 0.9],
			[-1, 0.9],
			[-1, -0.9]
		],
		[
			[-1, 0.9],
			[1, -0.9]
		],
		[
			[-0.8, -0.35],
			[-0.6, -0.55],
			[-0.4, -0.15],
			[-0.2, -0.35]
		],
		[
			[0.15, 0.3],
			[0.75, 0.3]
		],
		[
			[0.15, 0.55],
			[0.75, 0.55]
		]
	];
}
export interface EquipmentDrawing {
	masks: { position: [number, number]; radius: number }[];
	strokes: {
		from: [number, number];
		to: [number, number];
		key: string;
		inactive: boolean;
	}[];
	labels: { position: [number, number]; text: string; key: string }[];
}

/** Auxiliary equipment never enters AC topology, edge indices or loading statistics. */
export function equipmentDrawing(
	network: Network,
	graph: TxGraph,
	geometry: SchematicGeometry,
	parts: TxSchematicParts | null,
	anchors: Float32Array,
	unit: number,
	detail: boolean
): EquipmentDrawing {
	const out: EquipmentDrawing = { strokes: [], labels: [], masks: [] };
	const equipment = network.equipment;
	if (!equipment) return out;
	const point = (id: number): [number, number] | null => {
		const i = graph.index.get(id);
		if (i === undefined) return null;
		const p: [number, number] = [geometry.positions[i * 3], geometry.positions[i * 3 + 1]];
		return p.every(Number.isFinite) ? p : null;
	};
	const stroke = (a: [number, number], b: [number, number], key: string, inactive: boolean) =>
		out.strokes.push({ from: a, to: b, key, inactive });
	const symbol = (
		kind: EquipmentKind,
		x: number,
		y: number,
		c: number,
		s: number,
		key: string,
		inactive: boolean
	) => {
		for (const line of equipmentGlyph(kind, kind === 'switch' && !inactive))
			for (let i = 1; i < line.length; i++) {
				const p = line[i - 1],
					q = line[i];
				stroke(
					[x + unit * (p[0] * c - p[1] * s), y + unit * (p[0] * s + p[1] * c)],
					[x + unit * (q[0] * c - q[1] * s), y + unit * (q[0] * s + q[1] * c)],
					key,
					inactive
				);
			}
	};
	if (detail) {
		const slots = new Map<number, number>();
		for (const shunt of equipment.shunts) {
			const p = point(shunt.bus);
			if (!p) continue;
			const i = graph.index.get(shunt.bus)!;
			const slot = slots.get(shunt.bus) ?? 0;
			slots.set(shunt.bus, slot + 1);
			const root: [number, number] = parts ? [parts.busbars[i * 4], parts.busbars[i * 4 + 1]] : p;
			// Above the bus, leaving generator/load attachments below it.
			const x = root[0] + slot * unit * 3,
				y = root[1];
			const key = `shunt:${shunt.id}`;
			stroke(root, [x, y], key, !shunt.in_service);
			symbol(shuntKind(shunt), x, y, 0, -1, key, !shunt.in_service);
			out.labels.push({
				position: [x, y - unit * 3],
				text: `${shunt.in_service ? '' : 'OFF · '}S${shunt.id}`,
				key
			});
		}
		for (const t of equipment.three_winding ?? []) {
			const endpoints = t.buses.map(point);
			if (endpoints.some((p) => p === null)) continue;
			const p = t.star_bus === null ? null : point(t.star_bus);
			const centre: [number, number] = p ?? [
				endpoints.reduce((n, p) => n + p![0], 0) / 3,
				endpoints.reduce((n, p) => n + p![1], 0) / 3
			];
			const key = `three-winding:${t.id}`;
			if (t.star_bus === null)
				for (const end of endpoints) stroke(end!, centre, key, !t.in_service);
			out.masks.push({ position: centre, radius: unit * 1.6 });
			symbol('three_winding', centre[0], centre[1], 1, 0, key, !t.in_service);
			out.labels.push({
				position: [centre[0], centre[1] - unit * 2.5],
				text: `${t.in_service ? '' : 'OFF · '}3W ${t.id}`,
				key
			});
		}
		for (const d of equipment.devices ?? []) {
			const p = point(d.buses[0]);
			if (!p) continue;
			if (d.kind === 'switch' && d.buses.length === 2) {
				const q = point(d.buses[1]);
				if (!q) continue;
				const x = (p[0] + q[0]) / 2,
					y = (p[1] + q[1]) / 2;
				stroke(p, q, d.key, !d.in_service);
				out.masks.push({ position: [x, y], radius: unit * 1.6 });
				symbol('switch', x, y, 1, 0, d.key, !d.in_service);
				out.labels.push({
					position: [x, y - unit * 2],
					text: `${d.in_service ? 'CLOSED' : 'OPEN'} · ${d.key}`,
					key: d.key
				});
			} else {
				const slot = slots.get(d.buses[0]) ?? 0;
				slots.set(d.buses[0], slot + 1);
				const x = p[0] + slot * unit * 3;
				stroke(p, [x, p[1]], d.key, !d.in_service);
				symbol(d.kind, x, p[1], 0, -1, d.key, !d.in_service);
				out.labels.push({
					position: [x, p[1] - unit * 3],
					text: `${d.in_service ? '' : 'OFF · '}${d.key.toUpperCase()}`,
					key: d.key
				});
			}
		}
		for (const t of equipment.transformers.filter((t) => t.phase_shifter)) {
			const row = graph.branchIndex.get(t.branch);
			if (row === undefined || !Number.isFinite(anchors[row * 4])) continue;
			const [x, y] = anchors.subarray(row * 4, row * 4 + 2);
			out.labels.push({
				position: [x, y + unit * 2.6],
				text: `PST ${number(t.phase_shift_deg)}°`,
				key: `transformer:${t.branch}`
			});
		}
	}
	const pairs = new Map<string, number>();
	for (const h of equipment.hvdc) {
		const a = point(h.from),
			b = point(h.to);
		if (!a || !b) continue;
		const key = `hvdc:${h.id}`;
		const pair = [h.from, h.to].sort((a, b) => a - b).join(':');
		const slot = pairs.get(pair) ?? 0;
		pairs.set(pair, slot + 1);
		const dx = b[0] - a[0],
			dy = b[1] - a[1],
			length = Math.hypot(dx, dy);
		const c = length ? dx / length : 1,
			s = length ? dy / length : 0;
		const offset = (5 + slot * 4) * unit;
		const af: [number, number] = [a[0] - s * offset, a[1] + c * offset];
		const bt: [number, number] = [b[0] - s * offset, b[1] + c * offset];
		if (length < 4 * unit) {
			af[0] -= c * 3 * unit;
			af[1] -= s * 3 * unit;
			bt[0] += c * 3 * unit;
			bt[1] += s * 3 * unit;
		}
		stroke(a, [af[0] - c * unit, af[1] - s * unit], key, !h.in_service);
		stroke([bt[0] + c * unit, bt[1] + s * unit], b, key, !h.in_service);
		out.masks.push({ position: af, radius: unit * 1.1 }, { position: bt, radius: unit * 1.1 });
		// Put the converter boxes at the ends of the DC connection. The line stops
		// at each box, so there is no AC conductor drawn through a converter.
		symbol('hvdc', af[0], af[1], c, s, key, !h.in_service);
		symbol('hvdc', bt[0], bt[1], -c, -s, key, !h.in_service);
		stroke(
			[af[0] + c * unit, af[1] + s * unit],
			[bt[0] - c * unit, bt[1] - s * unit],
			key,
			!h.in_service
		);
		out.labels.push({
			position: [(af[0] + bt[0]) / 2, (af[1] + bt[1]) / 2 - unit * 1.5],
			text: `${h.in_service ? '' : 'OFF · '}DC ${h.id} · input`,
			key
		});
	}
	return out;
}
