/** Opt-in browser benchmarks for the generated-grid hand-off to the viewer (#261).
 *
 * Skipped unless TELLEGEN_SYNTH_BENCH=1. Build the wasm (`npm run wasm`) and
 * the app (`npm run build:web`) first; the config starts the preview and
 * fixture servers.
 *
 *   TELLEGEN_SYNTH_BENCH=1 npx --prefix apps/web playwright test \
 *     --config apps/web/playwright.config.ts synth-handoff-bench
 *
 * - `engine hand-off` times the public engine path on its own (fixture page):
 *   generation, `ingest()` to the page, `createStudy()`, first solution.
 * - `viewer` drops the same generated module into the built app and times
 *   the drop to the first solve card, with each engine round trip.
 *
 * Playwright's default headless shell renders WebGL in software
 * (SwiftShader); its GPU process then blocks compositor commits for seconds
 * on the map, which dominates the viewer time. TELLEGEN_SYNTH_BENCH_GPU=1
 * runs Chromium's new headless mode with hardware GL instead.
 *
 * Optional: TELLEGEN_SYNTH_BENCH_BUSES (default 3000), TELLEGEN_SYNTH_BENCH_RUNS
 * (default 5). The first sample is cold (worker start, wasm compile, places
 * table) and reported apart from the warm median. */

import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const enabled = process.env.TELLEGEN_SYNTH_BENCH === '1';
const gpu = process.env.TELLEGEN_SYNTH_BENCH_GPU === '1';
const buses = Number(process.env.TELLEGEN_SYNTH_BENCH_BUSES ?? 3000);
const runs = Number(process.env.TELLEGEN_SYNTH_BENCH_RUNS ?? 5);
const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);
const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));

type Sample = { ms: Record<string, number>; buses: number; module_json_chars: number };

const median = (xs: number[]) => {
	const s = [...xs].sort((a, b) => a - b);
	const m = Math.floor(s.length / 2);
	return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

function table(cold: Record<string, number>, warm: Record<string, number>[]) {
	const rows = Object.keys(cold).map(
		(name) =>
			`| ${name} | ${cold[name].toFixed(0)} | ${median(warm.map((w) => w[name])).toFixed(0)} |`
	);
	return [`| stage | cold ms | warm median ms (n=${warm.length}) |`, '|---|---:|---:|', ...rows];
}

// Browser launch options must be set at the top level of the file.
if (gpu)
	test.use({
		channel: 'chromium',
		launchOptions: {
			args: [
				'--enable-gpu',
				'--ignore-gpu-blocklist',
				...(process.platform === 'darwin' ? ['--use-angle=metal'] : [])
			]
		}
	});

test.describe('generated-grid hand-off browser benchmark', () => {
	test.skip(!enabled, 'opt-in: set TELLEGEN_SYNTH_BENCH=1');
	const mode = gpu ? 'hardware GL' : 'headless shell';

	test(`engine hand-off, ${buses}-bus generated grid`, async ({ page }) => {
		test.setTimeout(30 * 60_000);
		const pageErrors: string[] = [];
		page.on('pageerror', (error) => pageErrors.push(String(error)));
		await page.goto(`http://127.0.0.1:${port}/synth-handoff.html`);
		await expect(page.locator('#status')).toHaveText('ready', { timeout: 60_000 });
		const sample = () =>
			page.evaluate(
				(n) =>
					(
						window as unknown as { synthHandoff: { run(n: number): Promise<Sample> } }
					).synthHandoff.run(n),
				buses
			);
		const cold = await sample();
		const warm: Sample[] = [];
		for (let i = 0; i < runs; i++) warm.push(await sample());
		console.log(
			[
				`engine hand-off in Chromium (${mode}): ${cold.buses} buses, module_json ${(cold.module_json_chars / 1e6).toFixed(2)} M chars`,
				...table(
					cold.ms,
					warm.map((w) => w.ms)
				)
			].join('\n')
		);
		expect(pageErrors).toEqual([]);
	});

	test(`viewer, drop of a ${buses}-bus generated module to the first solve`, async ({
		page
	}, testInfo) => {
		test.setTimeout(30 * 60_000);
		const out = testInfo.outputPath();
		mkdirSync(out, { recursive: true });
		const modulePath = `${out}/generated.pio.json`;
		execFileSync(process.execPath, [
			`${repoRoot}packages/engine/scripts/bench-synth-handoff.mjs`,
			'--buses',
			String(buses),
			'--runs',
			'1',
			'--write-module',
			modulePath
		]);
		const buffer = readFileSync(modulePath);

		// Record every engine worker round trip: op, page time sent and back,
		// and the worker's own time for it.
		await page.addInitScript(() => {
			// When the page sees the file, after Playwright's upload.
			document.addEventListener(
				'change',
				(event) => {
					if ((event.target as HTMLInputElement | null)?.type === 'file')
						(window as unknown as { fileChange: number }).fileChange = performance.now();
				},
				true
			);
			const calls: { op: string; sent: number; back?: number; worker?: number }[] = [];
			(window as unknown as { engineCalls: typeof calls }).engineCalls = calls;
			// Request ids are per worker host.
			const pending = new WeakMap<Worker, Map<number, (typeof calls)[number]>>();
			const post = Worker.prototype.postMessage;
			Worker.prototype.postMessage = function (
				this: Worker,
				message: unknown,
				options?: Transferable[] | StructuredSerializeOptions
			) {
				const msg = message as { id?: unknown; op?: unknown };
				if (typeof msg?.id === 'number' && typeof msg.op === 'string') {
					let byId = pending.get(this);
					if (!byId) {
						const ids = new Map<number, (typeof calls)[number]>();
						pending.set(this, (byId = ids));
						this.addEventListener('message', (event: MessageEvent) => {
							const call = ids.get(event.data?.id);
							if (!call) return;
							ids.delete(event.data.id);
							call.back = performance.now();
							call.worker = event.data?.worker_ms;
						});
					}
					const call = { op: msg.op, sent: performance.now() };
					byId.set(msg.id, call);
					calls.push(call);
				}
				return post.call(this, message, options as StructuredSerializeOptions);
			};
		});
		await page.route('**/api/cases', (route) => void route.fulfill({ json: [] }));
		await page.goto('/');
		await expect(page.getByText('no default cases loaded')).toBeVisible();

		const start = await page.evaluate(() => performance.now());
		await page
			.locator('input[type="file"]')
			.setInputFiles([{ name: 'generated.pio.json', mimeType: 'application/json', buffer }]);
		// Polled every frame, so the end time is the first frame showing the result.
		const end = (await (
			await page.waitForFunction(
				() =>
					[...document.querySelectorAll('.solvecard dt')].some((dt) => dt.textContent === 'Time')
						? performance.now()
						: 0,
				null,
				{ polling: 'raf', timeout: 5 * 60_000 }
			)
		).jsonValue()) as number;
		const { fileChange, calls } = await page.evaluate(() => {
			const w = window as unknown as {
				fileChange: number;
				engineCalls: { op: string; sent: number; back?: number; worker?: number }[];
			};
			return { fileChange: w.fileChange, calls: w.engineCalls };
		});
		const at = (t?: number) => (t === undefined ? '-' : (t - start).toFixed(0));
		console.log(
			[
				`viewer in Chromium (${mode}): drop to first solve card ${(end - start).toFixed(0)} ms, ` +
					`of which file upload ${(fileChange - start).toFixed(0)} ms (times below count from the start of the upload)`,
				`solve card: ${(await page.locator('.solvecard .verdict').innerText()).trim()}`,
				'| engine op | sent ms | back ms | worker ms |',
				'|---|---:|---:|---:|',
				...calls
					.filter((c) => c.sent >= start)
					.map(
						(c) => `| ${c.op} | ${at(c.sent)} | ${at(c.back)} | ${c.worker?.toFixed(0) ?? '-'} |`
					)
			].join('\n')
		);
	});
});
