import { defineConfig, devices } from '@playwright/test';

// Opt-in qualification against the actual server and staged, pinned operator
// data. HTTPS preserves the production upgrade-insecure-requests policy in WebKit.
const baseURL = process.env.TELLEGEN_TEXAS7K_URL;
if (!baseURL || new URL(baseURL).protocol !== 'https:') {
	throw new Error('Set TELLEGEN_TEXAS7K_URL to the staged Tellegen HTTPS server');
}

export default defineConfig({
	testDir: './tests',
	testMatch: 'texas7k-distribution.spec.ts',
	outputDir: './test-results/texas7k-qualification',
	workers: 1,
	timeout: 180_000,
	expect: { timeout: 15_000 },
	use: {
		baseURL,
		ignoreHTTPSErrors: process.env.TELLEGEN_TEST_SELF_SIGNED_TLS === '1',
		trace: 'off',
		screenshot: 'only-on-failure'
	},
	projects: [
		{
			name: 'chromium-desktop',
			use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } }
		},
		{
			name: 'webkit-desktop',
			use: { ...devices['Desktop Safari'], viewport: { width: 1280, height: 800 } }
		},
		{ name: 'chromium-mobile-emulation', use: { ...devices['Pixel 7'] } }
	]
});
