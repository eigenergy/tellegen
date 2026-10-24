import type { GalvanicZone } from './galvanic-zones.js';

/** Voltage classes for drawing, by a zone's level in volts. Wide bands, so a single-phase winding's
 * own rating (7.2 kV of a 12.47 kV system, 19.1 kV SWER) lands where its system does. */
export type VoltageClass = 'hv' | 'sub' | 'mv' | 'lv';

export const VOLTAGE_CLASSES: { cls: VoltageClass; name: string; from: number }[] = [
	{ cls: 'hv', name: 'transmission', from: 100_000 },
	{ cls: 'sub', name: 'subtransmission', from: 25_000 },
	{ cls: 'mv', name: 'MV', from: 1_000 },
	{ cls: 'lv', name: 'LV', from: 0 }
];

/** The class of a zone: by its level when known; otherwise the zone holding a source counts as MV
 * and the others as LV, as for line weights. */
export function voltageClass(zone: GalvanicZone, volts: number | null): VoltageClass {
	if (volts === null) return zone.hasSource ? 'mv' : 'lv';
	return VOLTAGE_CLASSES.find((c) => volts >= c.from)!.cls;
}

/** A level as engineers name it: "11 kV", "400 V", "19.1 kV". */
export function levelLabel(volts: number): string {
	const kv = volts / 1000;
	if (kv < 1) return `${Math.round(volts)} V`;
	return `${Number(kv.toPrecision(4))} kV`;
}
