/** The words of a dynamics verdict: an icon and a word first (never colour
 * alone), then rows of metric · value · criterion · ✓/✗/–. Every number
 * comes from the engine's screening of the run's own samples. Pure. */

import type { RmsScreening } from '@tellegen/engine';

export type DynTone = 'ok' | 'warning' | 'failure' | 'pending';

export interface DynHeadline {
	icon: string;
	word: string;
	detail: string | null;
	tone: DynTone;
}

export interface DynRow {
	metric: string;
	value: string;
	criterion: string;
	status: '✓' | '✗' | '–';
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
			word: failure.at != null ? `Run failed at t = ${failure.at.toFixed(2)} s` : 'Run failed',
			detail: failure.message,
			tone: 'failure'
		};
	if (!s) return { icon: '…', word: 'Not run yet', detail: null, tone: 'pending' };
	switch (s.verdict) {
		case 'stable':
			return {
				icon: '✓',
				word: 'Stable',
				detail: s.voltage.recovered ? null : 'with a voltage concern',
				tone: s.voltage.recovered ? 'ok' : 'warning'
			};
		case 'loss_of_synchronism':
			return {
				icon: '✗',
				word: 'Unstable: loss of synchronism',
				detail: null,
				tone: 'failure'
			};
		case 'island_runaway':
			return {
				icon: '✗',
				word: 'Unstable: island frequency runs away',
				detail: null,
				tone: 'failure'
			};
		case 'not_settled':
			return {
				icon: '–',
				word: `Inconclusive: not settled by ${tEnd != null ? `${tEnd} s` : 'the end of the run'}`,
				detail: 'extend the run',
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

/** The swing trend in plain words, after a `; `. */
function trendWords(
	trend: RmsScreening['angle']['trend'],
	verdict: RmsScreening['verdict']
): string {
	switch (trend) {
		case 'decaying':
			return '; swings decaying';
		case 'damped_out':
			return '; swings damped out';
		case 'growing':
			return verdict === 'stable'
				? '; swings growing at the end, extend the run to confirm'
				: '; swings growing';
		case 'sustained':
			return '; swings sustained, neither growing nor decaying';
		case 'undetermined':
			return '; swing trend: too few large swing peaks to tell';
		default:
			return '';
	}
}

/** The rows under the headline. `events` is how many events the run
 * applied: with none, nothing is "after the event". */
export function rows(s: RmsScreening, events = 1): DynRow[] {
	const c = s.criteria;
	const f = s.frequency;
	const a = s.angle;
	const v = s.voltage;
	const slip = a.pole_slips[0];
	const out: DynRow[] = [
		{
			metric: 'Pole slip',
			value: slip
				? `unit ${slip.unit} slips at ${sec(slip.t)}`
				: `largest |δ − δ_COI| ${deg(a.max_coi_angle_deg)}${a.max_coi_angle_unit ? ` (unit ${a.max_coi_angle_unit}, ${sec(a.max_coi_angle_t)})` : ''}`,
			criterion: `no machine past ±${c.pole_slip_deg}° from its centre of inertia`,
			status: a.pole_slips.length ? '✗' : '✓'
		},
		{
			metric: 'First swing',
			value: a.first_swing_unit ? `${deg(a.first_swing_deg)} (unit ${a.first_swing_unit})` : '–',
			criterion:
				'reported; largest excursion of δ − δ_COI from its value before the first event, to its first peak after the last event',
			status: '–'
		},
		{
			metric: 'Max angle separation',
			value: `${deg(a.max_separation_deg)}${a.pair.length === 2 ? ` (${a.pair[0]} – ${a.pair[1]}, ${sec(a.separation_t)})` : ''}${events > 0 && a.initial_separation_deg != null ? `; ${deg(a.initial_separation_deg)} before the event` : ''}${trendWords(a.trend, s.verdict)}`,
			criterion:
				events > 0
					? 'reported; largest δmax − δmin from the first event on, against the steady-state spread before it'
					: 'reported; largest δmax − δmin over the run (no events)',
			status: '–'
		},
		{
			metric: 'Frequency nadir',
			value:
				events > 0
					? `${hz(f.coi_nadir_hz)} at ${sec(f.nadir_t)} (COI)`
					: `${hz(f.coi_nadir_hz)} (COI); no events`,
			criterion: `reported; lowest bus ${hz(f.lowest_bus_hz)}${f.lowest_bus != null ? ` at ${f.lowest_bus}` : ''}, lowest unit ${hz(f.lowest_unit_hz)}${f.lowest_unit ? ` (${f.lowest_unit})` : ''}`,
			status: '–'
		},
		{
			metric: 'RoCoF',
			value: `${f.max_rocof_hz_s.toFixed(3)} Hz/s`,
			criterion: `largest change of COI frequency over ${c.rocof_window_s} s`,
			status: '–'
		},
		{
			metric: 'Settling',
			value: f.settled && a.settled ? `settled at ${hz(f.final_hz)}` : 'not settled',
			criterion: `over the last ${c.settle_window_s} s, COI frequency stays within ${c.settle_frequency_hz} Hz of its final value and separation within ${c.settle_angle_deg}° of its final value`,
			status: f.settled && a.settled ? '✓' : '✗'
		},
		{
			metric: 'Voltage recovery',
			value:
				v.recovered_s == null
					? `not recovered; lowest ${v.min_pu.toFixed(3)} pu`
					: events === 0
						? `no events; lowest ${v.min_pu.toFixed(3)} pu${v.min_bus != null ? ` at ${v.min_bus}` : ''}`
						: `${sec(v.recovered_s)} after the last event; lowest ${v.min_pu.toFixed(3)} pu${v.min_bus != null ? ` at ${v.min_bus}` : ''}`,
			criterion: `every bus ≥ ${c.voltage_recovery_pu} pu within ${c.voltage_recovery_s} s; final ${v.final_min_pu.toFixed(3)}–${v.final_max_pu.toFixed(3)} pu`,
			status: v.recovered ? '✓' : '✗'
		}
	];
	if (s.islands.length > 1)
		out.push({
			metric: 'Islands',
			value: s.islands.map((i) => `${i.buses} buses at ${hz(i.final_hz ?? null)}`).join('; '),
			criterion: `each on its own COI; runaway past ${c.runaway_pu} pu`,
			status: s.islands.some((i) => i.runaway) ? '✗' : '✓'
		});
	return out;
}

/** The one sentence a screen reader hears once per run. */
export function announcement(s: RmsScreening | null, tEnd?: number, events = 1): string {
	const h = headline(s, null, tEnd);
	if (!s) return h.word;
	if (events === 0)
		return `${h.word}. No events applied; lowest COI frequency ${hz(s.frequency.coi_nadir_hz)}; lowest voltage ${s.voltage.min_pu.toFixed(2)} pu.`;
	return `${h.word}. COI frequency nadir ${hz(s.frequency.coi_nadir_hz)} at ${sec(s.frequency.nadir_t)}; lowest voltage ${s.voltage.min_pu.toFixed(2)} pu.`;
}

/** The one line under every verdict: what it does not prove. */
export const SCREENING_CAVEAT =
	"Screening by tellegen's criteria; no protection, UFLS or motor loads. Not a planning assessment.";

/** The fixed disclaimer of the screening. */
export const SCREENING_DISCLAIMER =
	'Screening criteria chosen by tellegen, not a TPL-001 or WECC planning assessment. No protection, UFLS, or motor loads are modelled (see Known limitations).';
