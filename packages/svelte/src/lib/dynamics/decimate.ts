/** M4 decimation for line charts: per pixel column the first, lowest,
 * highest and last sample, so the drawn line keeps every extremum (a fault
 * dip or a frequency nadir is never smoothed away). Pure. */

/** Indices of `y` (sampled at `x`, ascending) to draw across `width`
 * columns of the range `[x0, x1]`. NaN samples are skipped. */
export function m4(
	x: ArrayLike<number>,
	y: ArrayLike<number>,
	width: number,
	x0: number,
	x1: number
): number[] {
	const n = Math.min(x.length, y.length);
	const cols = Math.max(1, Math.floor(width));
	const span = x1 - x0;
	if (n === 0 || !(span > 0)) return [];
	const out: number[] = [];
	let col = -1;
	let first = -1,
		lo = -1,
		hi = -1,
		last = -1;
	const flush = () => {
		if (first < 0) return;
		const idx = [first, lo, hi, last].sort((a, b) => a - b);
		for (let k = 0; k < 4; k++) if (k === 0 || idx[k] !== idx[k - 1]) out.push(idx[k]);
	};
	for (let i = 0; i < n; i++) {
		const xi = x[i];
		if (xi < x0 || xi > x1) continue;
		const v = y[i];
		if (!Number.isFinite(v)) continue;
		const c = Math.min(cols - 1, Math.floor(((xi - x0) / span) * cols));
		if (c !== col) {
			flush();
			col = c;
			first = lo = hi = last = i;
			continue;
		}
		last = i;
		if (v < y[lo]) lo = i;
		if (v > y[hi]) hi = i;
	}
	flush();
	return out;
}
