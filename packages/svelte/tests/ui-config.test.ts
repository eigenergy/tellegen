import { describe, expect, it } from 'vitest';
import { resolveTellegenUiConfig } from '../src/lib/context.svelte.js';
import { formulationLabel } from '../src/lib/format.js';
describe('host configuration', () => {
	it('has no demo backend, organization, routes, or example by default', () => {
		expect(resolveTellegenUiConfig()).toMatchObject({
			apiBase: null,
			loadDefaultCases: false,
			orgLabel: '',
			orgHref: '',
			showFooter: false,
			menuLinks: [],
			dynamicsExample: false,
			distributionResultsPanel: null
		});
	});
	it('keeps explicit host choices', () => {
		const options = {
			apiBase: '/cases',
			loadDefaultCases: true,
			menuLinks: [{ label: 'Home', href: '/home' }],
			distributionResultsPanel: 'results',
			dynamicsExample: true
		};
		expect(resolveTellegenUiConfig(options)).toMatchObject(options);
	});
	it('names feasibility from solve metadata without changing calculation identifiers', () => {
		expect(formulationLabel('dcopf', 'feasibility')).toBe('DC feasibility');
		expect(formulationLabel('dcopf', 'cost')).toBe('DC OPF');
		expect(formulationLabel('dcopf')).toBe('DC OPF');
	});
});
