/** Series a display derives from a run: totals, rotor angles relative to
 * the centre of inertia, envelopes. Pure; inputs are the run's own samples
 * (views into its buffer), outputs fresh arrays. */

export interface SeriesSource {
	time: Float64Array;
	keys: readonly string[];
	series(key: string): Float64Array | undefined;
}

/** Σ of every channel with `prefix` (e.g. `pl:`), sample by sample. */
export function sumPrefix(run: SeriesSource, prefix: string, scale = 1): Float64Array {
	const out = new Float64Array(run.time.length);
	for (const k of run.keys) {
		if (!k.startsWith(prefix)) continue;
		const s = run.series(k)!;
		for (let i = 0; i < out.length; i++) out[i] += s[i] * scale;
	}
	return out;
}

/** `δ − δ_COI` in degrees for one unit. */
export function coiRelativeDeg(run: SeriesSource, unit: string): Float64Array | undefined {
	const d = run.series(`delta:${unit}`);
	const c = run.series('coi:delta');
	if (!d || !c) return undefined;
	const out = new Float64Array(d.length);
	const k = 180 / Math.PI;
	for (let i = 0; i < d.length; i++) out[i] = (d[i] - c[i]) * k;
	return out;
}

/** Per sample, the lowest and highest of every channel with `prefix`
 * (NaN skipped). */
export function envelope(
	run: SeriesSource,
	prefix: string
): { lo: Float64Array; hi: Float64Array; argLo: Int32Array } {
	const n = run.time.length;
	const lo = new Float64Array(n).fill(Infinity);
	const hi = new Float64Array(n).fill(-Infinity);
	const argLo = new Int32Array(n).fill(-1);
	run.keys.forEach((k, j) => {
		if (!k.startsWith(prefix)) return;
		const s = run.series(k)!;
		for (let i = 0; i < n; i++) {
			const v = s[i];
			if (!Number.isFinite(v)) continue;
			if (v < lo[i]) {
				lo[i] = v;
				argLo[i] = j;
			}
			if (v > hi[i]) hi[i] = v;
		}
	});
	return { lo, hi, argLo };
}

/** The `count` channels with `prefix` whose minimum is lowest. */
export function lowest(run: SeriesSource, prefix: string, count: number): string[] {
	const mins: [string, number][] = [];
	for (const k of run.keys) {
		if (!k.startsWith(prefix)) continue;
		const s = run.series(k)!;
		let m = Infinity;
		for (let i = 0; i < s.length; i++) if (s[i] < m) m = s[i];
		if (Number.isFinite(m)) mins.push([k, m]);
	}
	return mins
		.sort((a, b) => a[1] - b[1])
		.slice(0, count)
		.map(([k]) => k);
}

/** The units whose COI-relative angle swings furthest from its start. */
export function largestSwings(
	run: SeriesSource,
	units: readonly string[],
	count: number
): string[] {
	const out: [string, number][] = [];
	for (const u of units) {
		const r = coiRelativeDeg(run, u);
		if (!r || r.length === 0) continue;
		let m = 0;
		for (let i = 0; i < r.length; i++) m = Math.max(m, Math.abs(r[i] - r[0]));
		out.push([u, m]);
	}
	return out
		.sort((a, b) => b[1] - a[1])
		.slice(0, count)
		.map(([u]) => u);
}

/** Index of the sample at or before `t`, and the fraction to the next. */
export function sampleAt(time: ArrayLike<number>, t: number): { i: number; f: number } {
	const n = time.length;
	if (n === 0) return { i: 0, f: 0 };
	if (t <= time[0]) return { i: 0, f: 0 };
	if (t >= time[n - 1]) return { i: n - 1, f: 0 };
	let lo = 0,
		hi = n - 1;
	while (hi - lo > 1) {
		const mid = (lo + hi) >> 1;
		if (time[mid] <= t) lo = mid;
		else hi = mid;
	}
	const span = time[hi] - time[lo];
	return { i: lo, f: span > 0 ? (t - time[lo]) / span : 0 };
}

/** A series' value at `t`, linear between samples. */
export function valueAt(time: ArrayLike<number>, s: ArrayLike<number>, t: number): number {
	const { i, f } = sampleAt(time, t);
	if (f === 0 || i + 1 >= s.length) return s[i];
	return s[i] + (s[i + 1] - s[i]) * f;
}
