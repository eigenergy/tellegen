/** The words of a dynamics verdict: an icon and a word first (never colour
 * alone), then labelled numbers, pass or fail checks and a list of details.
 * Every number comes from the engine's screening of the run's own samples. Pure. */

import type { RmsScreening } from '@tellegen/engine';
import type { Stat } from '../diagram/verdict.js';

export type DynTone = 'ok' | 'warning' | 'failure' | 'pending';

export interface DynHeadline {
	icon: string;
	word: string;
	detail: string | null;
	tone: DynTone;
}

const hz = (v: number | null | undefined, d = 3) =>
	v == null || !Number.isFinite(v) ? '–' : `${v.toFixed(d)} Hz`;
const deg = (v: number) => `${v.toFixed(1)}°`;
const sec = (v: number) => `${v.toFixed(2)} s`;

/** The headline of a screened run, or of a run that failed at `failedAt`. */
export function headline(
	s: RmsScreening | null,
	failure?: { message: string; at: number | null } | null,
	tEnd?: number
): DynHeadline {
	if (failure)
		return {
			icon: '⨯',
			word: 'Failed',
			detail: failure.at != null ? `at ${failure.at.toFixed(2)} s` : null,
			tone: 'failure'
		};
	if (!s) return { icon: '…', word: 'Not run', detail: null, tone: 'pending' };
	switch (s.verdict) {
		case 'stable':
			return {
				icon: '✓',
				word: 'Stable',
				detail: s.voltage.recovered ? null : 'Voltage concern',
				tone: s.voltage.recovered ? 'ok' : 'warning'
			};
		case 'loss_of_synchronism':
			return {
				icon: '✗',
				word: 'Unstable',
				detail: 'Loss of synchronism',
				tone: 'failure'
			};
		case 'island_runaway':
			return {
				icon: '✗',
				word: 'Unstable',
				detail: 'Island runaway',
				tone: 'failure'
			};
		case 'not_settled':
			return {
				icon: '–',
				word: 'Not settled',
				detail: tEnd != null ? `by ${tEnd} s` : null,
				tone: 'warning'
			};
	}
}

/** The time at which a failed run stopped, read from the engine's message
 * (`… at t = 3.4200 s: …`). */
export function failureTime(message: string): number | null {
	const m = /at t = ([0-9.]+) s/.exec(message);
	return m ? Number(m[1]) : null;
}

/** A unit as the engine names it (`bus#id`), in plain words. */
export const unitName = (key: string) => `Unit ${key.replace('#', ' ')}`;

const TREND: Record<string, string> = {
	decaying: 'Decaying',
	damped_out: 'Damped out',
	growing: 'Growing',
	sustained: 'Sustained',
	undetermined: 'Unclear'
};

/** The key results of a screened run as labelled numbers. Red marks the one
 * that failed its check. */
export function stats(s: RmsScreening): Stat[] {
	const f = s.frequency;
	const a = s.angle;
	const v = s.voltage;
	const settled = f.settled && a.settled;
	const out: Stat[] = [
		{ label: 'Lowest frequency', value: hz(f.coi_nadir_hz) },
		{
			label: 'Lowest voltage',
			value: `${v.min_pu.toFixed(3)} pu`,
			tone: v.recovered ? undefined : 'failure'
		},
		{
			label: 'Angle spread',
			value: deg(a.max_separation_deg),
			tone: a.pole_slips.length ? 'failure' : undefined
		},
		{
			label: 'Final frequency',
			value: settled ? hz(f.final_hz) : 'Not settled',
			tone: settled ? undefined : 'warning'
		}
	];
	if (s.islands.length > 1)
		out.push({
			label: 'Islands',
			value: s.islands.length.toLocaleString(),
			tone: s.islands.some((i) => i.runaway) ? 'failure' : undefined
		});
	return out;
}

export interface DynCheck {
	label: string;
	ok: boolean;
}

/** The pass or fail checks behind the verdict, one word each. */
export function checks(s: RmsScreening): DynCheck[] {
	const out: DynCheck[] = [
		{ label: 'In step', ok: s.angle.pole_slips.length === 0 },
		{ label: 'Settled', ok: s.frequency.settled && s.angle.settled },
		{ label: 'Voltage recovered', ok: s.voltage.recovered }
	];
	if (s.islands.length > 1)
		out.push({ label: 'Islands stable', ok: !s.islands.some((i) => i.runaway) });
	return out;
}

export interface DynDetail {
	label: string;
	value: string;
}

/** Where and when: one value per row, for the Details disclosure. `events` is
 * how many events the run applied; with none, nothing is "before the event". */
export function details(s: RmsScreening, events = 1): DynDetail[] {
	const f = s.frequency;
	const a = s.angle;
	const v = s.voltage;
	const out: DynDetail[] = [];
	const add = (label: string, value: string | null | undefined) => {
		if (value) out.push({ label, value });
	};
	const slip = a.pole_slips[0];
	if (slip) {
		add('Pole slip', unitName(slip.unit));
		add('Pole slip time', sec(slip.t));
	}
	if (events > 0) add('Lowest frequency time', sec(f.nadir_t));
	if (f.lowest_bus != null) {
		add('Lowest bus frequency', hz(f.lowest_bus_hz));
		add('Lowest frequency bus', `Bus ${f.lowest_bus}`);
	}
	add('Fastest frequency change', `${f.max_rocof_hz_s.toFixed(3)} Hz/s`);
	if (v.min_bus != null) add('Lowest voltage bus', `Bus ${v.min_bus}`);
	if (events > 0 && v.recovered_s != null) add('Voltage recovery time', sec(v.recovered_s));
	add('Final voltage range', `${v.final_min_pu.toFixed(3)} to ${v.final_max_pu.toFixed(3)} pu`);
	if (a.pair.length === 2)
		add('Widest angle pair', `Units ${a.pair.map((k) => k.replace('#', ' ')).join(' and ')}`);
	if (events > 0 && a.initial_separation_deg != null)
		add('Angle spread before event', deg(a.initial_separation_deg));
	if (a.trend) add('Swings', TREND[a.trend]);
	if (a.first_swing_unit) add('First swing', deg(a.first_swing_deg));
	if (s.islands.length > 1)
		s.islands.forEach((i, n) => {
			add(`Island ${n + 1} buses`, i.buses.toLocaleString());
			add(`Island ${n + 1} frequency`, hz(i.final_hz ?? null));
		});
	return out;
}

/** What a screen reader hears once per run. */
export function announcement(s: RmsScreening | null, tEnd?: number): string {
	const h = headline(s, null, tEnd);
	if (!s) return h.word;
	return `${h.word}. Lowest frequency ${hz(s.frequency.coi_nadir_hz)}. Lowest voltage ${s.voltage.min_pu.toFixed(3)} pu.`;
}
