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
		else if (!NUMBER.test(t) || !Number.isFinite(Number(t))) errors.push(`${rec.model} ${p.name}: "${t}" is not a number`);
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
