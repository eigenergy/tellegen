/** Pure helpers of the live workbench, the unit editor and their charts,
 * apart from the components so they are tested without a browser.
 * @block Live workbench logic */

/** The simulated time the live view's pacing loop aims for after `wall`
 * seconds of wall time at `speed`. Paced, it follows the clock but runs at
 * most 2 s ahead of the session time `t`; unpaced (`Infinity`) it stays at
 * `t`, so a switch back to a pace starts from the time reached (a product
 * `0 · Infinity` would otherwise leave it `NaN` and stall the loop).
 * @block Pacing target */
export function paceTarget(expected: number, wall: number, speed: number, t: number): number {
	if (!Number.isFinite(speed) || !Number.isFinite(expected)) return t;
	return Math.min(expected + wall * speed, t + 2);
}

/** Round axis ticks between `lo` and `hi`, about `count` of them. The loop
 * is bounded and stops where the values no longer differ, so a range at the
 * limit of the floating-point resolution cannot hang it.
 * @block Axis ticks */
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

/** `[a, b]` widened about its centre to at least {@link minZoomSpan}.
 * @block Zoom limit */
export function clampZoom(a: number, b: number): [number, number] {
	const min = minZoomSpan(a, b);
	if (b - a >= min) return [a, b];
	const c = (a + b) / 2;
	return [c - min / 2, c + min / 2];
}

const NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

export interface DraftRecord {
	line: number;
	model: string;
	parameters: { name: string; text: string }[];
}

/** The parameter edits of the unit editor's drafts (`line:index` → typed
 * text): changed values become edits; blank or non-numeric text is an
 * error naming the parameter; a draft whose parameter no longer exists is
 * dropped.
 * @block Parameter drafts */
export function parseDrafts(
	draft: Record<string, string>,
	records: readonly DraftRecord[]
): { edits: { line: number; index: number; value: number }[]; errors: string[] } {
	const edits: { line: number; index: number; value: number }[] = [];
	const errors: string[] = [];
	for (const [k, raw] of Object.entries(draft)) {
		const [line, index] = k.split(':').map(Number);
		const rec = records.find((r) => r.line === line);
		const p = rec?.parameters[index];
		if (!rec || !p) continue;
		const t = raw.trim();
		if (t === p.text) continue;
		if (!t) errors.push(`${rec.model} ${p.name}: enter a number`);
		else if (!NUMBER.test(t)) errors.push(`${rec.model} ${p.name}: "${t}" is not a number`);
		else edits.push({ line, index, value: Number(t) });
	}
	return { edits, errors };
}

/** What the drafts were typed against: the records' lines, models and
 * parameter texts. Drafts are cleared when it changes.
 * @block Draft signature */
export function recordsSignature(records: readonly DraftRecord[]): string {
	return records.map((r) => `${r.line} ${r.model} ${r.parameters.map((p) => p.text).join(' ')}`).join('\n');
}
