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

/** Why `text` is not a parameter value, or `null` when it is one: a
 * number (`1.5`, `-.2`, `3e-2`) and at least `min` when given. The unit
 * editor and the block-diagram editor check what is typed with it.
 * @block Parameter value check */
export function numberError(text: string, min?: number): string | null {
	const t = text.trim();
	if (!t) return 'enter a number';
	if (!NUMBER.test(t) || !Number.isFinite(Number(t))) return `"${t}" is not a number`;
	if (min !== undefined && Number(t) < min) return `must be at least ${min}`;
	return null;
}

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
		const error = numberError(t);
		if (error) errors.push(`${rec.model} ${p.name}: ${error}`);
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
