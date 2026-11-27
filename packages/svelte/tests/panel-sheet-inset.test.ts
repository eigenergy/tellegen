import { describe, expect, it } from 'vitest';
import { sheetInset, type SheetInsetInput } from '../src/lib/panels.svelte';

const base: SheetInsetInput = {
	active: true,
	stripShown: false,
	stripBottom: 88,
	stripHeight: 36,
	handleHeight: 22,
	open: false,
	headerAbove: false
};

describe('sheetInset', () => {
	it('is 0 with the strip hidden and no panel open', () => {
		expect(sheetInset(base)).toBe(0);
	});

	it('counts the header above the sheet when the strip is hidden', () => {
		expect(sheetInset({ ...base, open: true, stripBottom: 96, headerAbove: true })).toBe(96 + 36);
	});

	it('counts only the sheet when its header is inside it', () => {
		expect(sheetInset({ ...base, open: true, stripBottom: 96 })).toBe(96);
	});

	it('counts the strip and its handle when the strip is shown', () => {
		expect(sheetInset({ ...base, stripShown: true, open: true, stripBottom: 96 })).toBe(
			96 + 36 + 22
		);
		expect(sheetInset({ ...base, stripShown: true, stripBottom: 0 })).toBe(36 + 22);
	});

	it('is 0 outside the phone layout', () => {
		expect(sheetInset({ ...base, active: false, open: true, headerAbove: true })).toBe(0);
	});
});
