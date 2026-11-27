import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import TellegenProvider from '../src/lib/TellegenProvider.svelte';

describe('TellegenProvider server render', () => {
	it('leaves data-theme off without a theme prop, so the first-paint CSS applies', () => {
		const { body } = render(TellegenProvider, { props: { loadDefaultCases: false } });
		expect(body).toMatch(/class="tellegen"/);
		expect(body).not.toContain('data-theme');
	});

	it('renders the theme a host passes', () => {
		const { body } = render(TellegenProvider, {
			props: { loadDefaultCases: false, theme: 'dark' }
		});
		expect(body).toContain('data-theme="dark"');
	});
});
