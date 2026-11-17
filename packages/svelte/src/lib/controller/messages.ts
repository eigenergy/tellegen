import { RATE_LIMITED } from '../api.js';

export const NOTICES = {
	caseChanged: 'Case changed; attach again',
	caseChangedWhileLoading: 'Case changed; attach again',
	selectMatchingCase: 'Wrong case',
	returnToLiveCase: 'Close the study first',
	batchBusy: 'Still reading files',
	collectionOpen: 'Collection still open',
	rateLimited: RATE_LIMITED
} as const;

/** A case that needs the server when the host configured none. */
const noApiNotice = (name: string) => `${name} needs the tellegen server`;

const HIDDEN_DEFAULT_CASES_KEY = 'tellegen.hiddenDefaultCases.v1';
// Open on South Carolina by default: it has the most interesting price action.
const DEFAULT_CASE_ID = 'case500';

/** Commit and snapshot responses can order object keys differently (including nested loading
 * metadata). Compare the exact JSON values, retaining array order and numerical equality. */
function sameSolution(a: Solution, b: Solution): boolean {
	const ordered = (_key: string, value: unknown) => {
		if (value === null || typeof value !== 'object' || Array.isArray(value)) return value;
		const record = value as Record<string, unknown>;
		return Object.fromEntries(
			Object.keys(record)
				.sort()
				.map((key) => [key, record[key]])
		);
	};
	return JSON.stringify(a, ordered) === JSON.stringify(b, ordered);
}

/** A parsed geographic sidecar awaiting application: the canonical `.geo.json`
 * layer document from the engine's tolerant reader, its source file name, and
 * the reader's notes on records it could not use. */

/** Whether a local case renders or will render a bus with this id. */
function localHasBus(c: LocalCase, busId: number): boolean {
	const buses = c.view?.buses ?? c.diagram?.view.buses ?? c.topology?.buses ?? [];
	return buses.some((b) => b.id === busId);
}

type SavedCaseCapture = {
	input: string;
	solution?: string;
	view?: import('@tellegen/engine').StudyView;
};

export function caseLabel(name: string | null | undefined, fileName: string): string {
	const stem = fileName.replace(/\.[^.]+$/, '');
	const given = name?.trim();
	if (!given || given === 'case') return stem;
	// RAW and RAWX headers hold a free-text line cut at its field width, not a name.
	if (/\.rawx?$/i.test(fileName)) return stem;
	if (given.length > 32 || given.split(/\s+/).length > 4) return stem;
	return given;
}
