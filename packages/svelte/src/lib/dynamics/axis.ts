/** Axis ranges and ticks of the trace charts. Pure. */

/** The smallest y span a chart shows for `unit`: a flat trace (a case at
 * rest) gets a readable axis around its value instead of ticks that all
 * print the same number. */
export function minSpanFor(unit: string): number {
	if (/Hz/.test(unit)) return 0.02;
	if (/pu/.test(unit)) return 0.02;
	if (/°/.test(unit)) return 2;
	if (/MW|Mvar/.test(unit)) return 2;
	return 0;
}

/** The y range for data spanning `[lo, hi]`: at least `minSpan` wide,
 * centred on the data, with a 6 % margin. */
export function yDomain(lo: number, hi: number, minSpan = 0): [number, number] {
	if (!Number.isFinite(lo) || !Number.isFinite(hi)) return [0, 1];
	let span = Math.max(hi - lo, Math.abs(hi) * 1e-6, 1e-6);
	if (span < minSpan) {
		const mid = (lo + hi) / 2;
		lo = mid - minSpan / 2;
		hi = mid + minSpan / 2;
		span = minSpan;
	}
	return [lo - 0.06 * span, hi + 0.06 * span];
}

/** "Nice" ticks (1, 2, 2.5, 5 × 10ⁿ) inside `[lo, hi]`, at most 50. */
export function ticks(lo: number, hi: number, n: number): number[] {
	const raw = (hi - lo) / n;
	const mag = 10 ** Math.floor(Math.log10(raw));
	const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw;
	const out: number[] = [];
	if (!(step > 0) || !Number.isFinite(lo) || !Number.isFinite(hi)) return out;
	const first = Math.ceil(lo / step);
	for (let k = 0; k < 50; k++) {
		const v = (first + k) * step;
		if (v > hi + 1e-12) break;
		out.push(+v.toPrecision(12));
	}
	return out;
}

/** A tick label with as many decimals as the tick step needs, and no more:
 * a 0.005 step prints 60.005, not 60.0050. */
export function tickLabel(v: number, step: number): string {
	let d = 0;
	const s = Math.abs(step) || 1;
	while (d < 5 && Math.abs(s * 10 ** d - Math.round(s * 10 ** d)) > 1e-9 * 10 ** d) d++;
	return v.toFixed(d);
}
