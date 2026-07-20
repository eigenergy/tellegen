/** Client-side helper for saved study packages (`.pio.json`). The engine owns
 * parsing, restoring, and exporting; classification of a dropped file lives in
 * `drop-classify.ts` (the single owner). This module covers the one remaining
 * frontend decision around a package: which commit an export targets. */

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
