/** Axis helpers shared by the charts: round tick values and zoom limits.
 * Pure.
 * @block Chart axes */

/** Round axis ticks between `lo` and `hi`, about `count` of them. The loop
 * is bounded and stops where the values no longer differ, so a range at the
 * limit of the floating-point resolution cannot hang it. */
export function niceTicks(lo: number, hi: number, count: number): number[] {
	const span = hi - lo;
	if (!Number.isFinite(span) || !(span > 0)) return Number.isFinite(lo) ? [lo] : [];
	const n = Math.max(1, Math.floor(count));
	const mag = 10 ** Math.floor(Math.log10(span / n));
	// (The tolerance keeps a range such as 0.9 to 1.1, whose difference is
	// 0.2000000000000001, at the step it reads as.)
	const step = [1, 2, 2.5, 5, 10].map((f) => f * mag).find((s) => span / s <= n * (1 + 1e-9)) ?? 10 * mag;
	const first = Math.ceil(lo / step);
	const out: number[] = [];
	for (let i = 0; i <= n + 1; i++) {
		const v = +((first + i) * step).toPrecision(12);
		if (v > hi + step * 1e-9) break;
		if (out.length && v <= out[out.length - 1]) break;
		out.push(v);
	}
	return out;
}

/** The narrowest x range a chart zooms to, in its axis units: a millionth
 * of the values' magnitude (at least of 1), well above the resolution of
 * the tick labels. */
export function minZoomSpan(a: number, b: number): number {
	return 1e-6 * Math.max(1, Math.abs(a), Math.abs(b));
}

/** `[a, b]` widened about its centre to at least {@link minZoomSpan}. */
export function clampZoom(a: number, b: number): [number, number] {
	const min = minZoomSpan(a, b);
	if (b - a >= min) return [a, b];
	const c = (a + b) / 2;
	return [c - min / 2, c + min / 2];
}
