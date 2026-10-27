import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import LocalCaseDetails from '../src/lib/components/LocalCaseDetails.svelte';
import type { LocalCase } from '../src/lib/state.svelte.js';

const state = vi.hoisted(() => ({
	app: { activeLocal: null as unknown },
	exportUnavailable: null as string | null
}));

vi.mock('../src/lib/context.svelte.js', () => ({
	getAppState: () => state.app,
	getController: () => ({ caseExportUnavailableReason: () => state.exportUnavailable })
}));

function localCase(overrides: Record<string, unknown> = {}) {
	return {
		label: 'Portugal',
		fileName: 'portugal.pio.json',
		summary: {
			n_bus: 120,
			n_branch: 180,
			n_gen: 40,
			load_mw: 7000,
			gen_mw: 9000,
			base_mva: 100,
			coords_method: 'generated'
		},
		studyInputJson: '{}',
		view: { buses: [], branches: [] },
		coordsKind: 'synthetic',
		displayMode: 'geographic',
		diagram: null,
		formulation: 'dcopf',
		solution: {},
		...overrides
	};
}

beforeEach(() => {
	state.app.activeLocal = localCase();
	state.exportUnavailable = null;
});

describe('local case details', () => {
	it('shows data and actions without host copy', () => {
		const { body } = render(LocalCaseDetails);
		expect(body).toMatch(/<h2[^>]*>Portugal<\/h2>/);
		expect(body).toMatch(/Coordinates<\/dt> <dd[^>]*>Generated<\/dd>/);
		expect(body).toMatch(/File<\/dt> <dd[^>]*>portugal\.pio\.json<\/dd>/);
		for (const label of ['Save case', 'Save solution', 'Export as', 'Save geography', 'Remove']) {
			expect(body).toContain(label);
		}
		for (const prose of ['browser', 'upload', 'wasm', 'via ', '(.json)', '…']) {
			expect(body).not.toContain(prose);
		}
	});

	it('names the geographic file the coordinates came from', () => {
		state.app.activeLocal = localCase({ coordsKind: 'geofile', geoSource: 'buses.csv' });
		expect(render(LocalCaseDetails).body).toMatch(
			/Coordinates<\/dt> <dd[^>]*>From buses\.csv<\/dd>/
		);
	});

	it('disables saving and exporting with the controller reason', () => {
		state.exportUnavailable = 'Saving and exporting AC OPF cases is not supported yet.';
		const { body } = render(LocalCaseDetails);
		expect(body).toMatch(/<button[^>]*disabled[^>]*>Save case<\/button>/);
		expect(body).toMatch(/<button[^>]*disabled[^>]*>\s*Export as\s*<\/button>/);
		expect(body).toContain('Saving and exporting AC OPF cases is not supported yet.');
	});

	it('renders a host note with the case', () => {
		const note = createRawSnippet((lc: () => LocalCase) => ({
			render: () => `<p>Generated from ${lc().label}</p>`
		}));
		const { body } = render(LocalCaseDetails, { props: { note } });
		expect(body).toContain('<p>Generated from Portugal</p>');
	});
});
