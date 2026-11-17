import { RATE_LIMITED } from '../api.js';

/** Short notices shared by controller operations. */
export const NOTICES = {
	caseChanged: 'Case changed; attach again',
	caseChangedWhileLoading: 'Case changed; attach again',
	selectMatchingCase: 'Wrong case',
	returnToLiveCase: 'Close the study first',
	batchBusy: 'Still reading files',
	collectionOpen: 'Collection still open',
	rateLimited: RATE_LIMITED
} as const;

export function caseLabel(name: string | null | undefined, fileName: string): string {
	const stem = fileName.replace(/\.[^.]+$/, '');
	const given = name?.trim();
	if (!given || given === 'case') return stem;
	// RAW and RAWX headers hold a free-text line cut at its field width, not a name.
	if (/\.rawx?$/i.test(fileName)) return stem;
	if (given.length > 32 || given.split(/\s+/).length > 4) return stem;
	return given;
}
