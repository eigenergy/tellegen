import { defineConfig, devices } from '@playwright/test';

const previewPort = Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173);
if (!Number.isInteger(previewPort) || previewPort < 1 || previewPort > 65535) {
	throw new Error('TELLEGEN_PREVIEW_PORT must be an integer from 1 to 65535');
}
const previewUrl = `http://127.0.0.1:${previewPort}`;
const mcPreviewPort = Number(process.env.TELLEGEN_MC_PREVIEW_PORT ?? previewPort + 1);
if (
	!Number.isInteger(mcPreviewPort) ||
	mcPreviewPort < 1 ||
	mcPreviewPort > 65535 ||
	mcPreviewPort === previewPort
) {
	throw new Error('TELLEGEN_MC_PREVIEW_PORT must be a distinct port from 1 to 65535');
}

export default defineConfig({
	testDir: './tests',
	timeout: 60_000,
	expect: { timeout: 15_000 },
	use: {
		baseURL: previewUrl,
		trace: 'retain-on-failure',
		viewport: { width: 1280, height: 800 }
	},
	webServer: [
		{
			command: `npm run preview -- --host 127.0.0.1 --port ${previewPort} --strictPort`,
			url: previewUrl,
			reuseExistingServer: !process.env.CI,
			timeout: 120_000
		},
		{
			command: `npx vite --config tests/mc-pf-vite.config.ts --host 127.0.0.1 --port ${mcPreviewPort} --strictPort`,
			url: `http://127.0.0.1:${mcPreviewPort}`,
			reuseExistingServer: !process.env.CI,
			timeout: 120_000
		}
	],
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		}
	]
});
