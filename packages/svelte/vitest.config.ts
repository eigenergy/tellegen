import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	plugins: [svelte()],
	test: {
		projects: [
			{
				extends: true,
				test: {
					name: 'unit',
					include: ['tests/**/*.test.ts'],
					exclude: ['tests/**/*.svelte.test.ts']
				}
			},
			{
				// Runes modules compiled for the browser, so their effects run.
				extends: true,
				resolve: { conditions: ['browser'] },
				test: {
					name: 'runes',
					include: ['tests/**/*.svelte.test.ts'],
					environment: './tests/client-environment.ts'
				}
			}
		]
	}
});
