/** Client-side helpers for saved study packages (`.pio.json`). The engine owns
 * parsing, restoring, and exporting; these cover the two frontend decisions around
 * them: whether a dropped file is a study package, and which commit an export targets. */

/** The schema URL prefix every `.pio.json` package envelope carries. */
export const PIO_PACKAGE_SCHEMA_PREFIX = 'https://powerio.dev/schema/pio-package';

/** Whether `text` is a saved study package, by its envelope `schema` field. Sniffs
 * before invoking the engine so a plain JSON case is left to the normal case path. A
 * non-JSON or non-package input returns false rather than throwing. */
export function isStudyPackageText(text: string): boolean {
	let schema: unknown;
	try {
		schema = (JSON.parse(text) as { schema?: unknown }).schema;
	} catch {
		return false;
	}
	return typeof schema === 'string' && schema.startsWith(PIO_PACKAGE_SCHEMA_PREFIX);
}

/** The commit index for a saved package's current committed state: its last study
 * commit, or 0 when it carries none (export then writes the base network). Tolerant of
 * malformed input, returning 0 rather than throwing. */
export function studyCommitIndex(packageJson: string): number {
	try {
		const commits = (JSON.parse(packageJson) as { study?: { commits?: unknown[] } }).study?.commits
			?.length;
		return Math.max(0, (commits ?? 0) - 1);
	} catch {
		return 0;
	}
}
