/** Opt-in browser benchmarks for the generated-grid hand-off to the viewer (#261).
 *
 * Skipped unless TELLEGEN_SYNTH_BENCH=1. Build the wasm (`npm run wasm`)
 * first; the config starts the fixture server.
 *
 *   TELLEGEN_SYNTH_BENCH=1 npx --prefix apps/web playwright test \
 *     --config apps/web/playwright.config.ts synth-handoff-bench
 *
 * - `engine hand-off` times the public engine path on its own (fixture page):
 *   generation, `ingest()` to the page, `createStudy()`, first solution.
 * - `viewer` mounts tellegen's shell on a fixture page and opens the
 *   generated grid the way MapToGrid's hand-off does: the ingested payload
 *   straight to `Controller.openBalancedModule` with `formulation: 'dcopf'`
 *   (no `ingest_json_drop`), timed from the payload in hand to the first
 *   frame showing the DC OPF solve card, with each engine round trip.
 *
 * Playwright's default headless shell renders WebGL in software
 * (SwiftShader). TELLEGEN_SYNTH_BENCH_GPU=1 runs Chromium's new headless
 * mode with hardware GL instead, to separate rendering from engine time.
 *
 * Optional: TELLEGEN_SYNTH_BENCH_BUSES (default 3000), TELLEGEN_SYNTH_BENCH_RUNS
 * (default 5), TELLEGEN_SYNTH_BENCH_CHINA=1 (the viewer test generates
 * MapToGrid's request for China at its default density instead). The first sample is cold (worker start, wasm compile, places
 * table) and reported apart from the warm median. */

import { expect, test } from '@playwright/test';
import type { HandoffTiming } from './fixtures/mc-pf-browser/generated-handoff';

const enabled = process.env.TELLEGEN_SYNTH_BENCH === '1';
const gpu = process.env.TELLEGEN_SYNTH_BENCH_GPU === '1';
const china = process.env.TELLEGEN_SYNTH_BENCH_CHINA === '1';
const buses = Number(process.env.TELLEGEN_SYNTH_BENCH_BUSES ?? 3000);
const runs = Number(process.env.TELLEGEN_SYNTH_BENCH_RUNS ?? 5);
const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);

type Sample = { ms: Record<string, number>; buses: number; module_json_chars: number };
type EngineCall = { op: string; sent: number; back?: number; worker?: number };

declare global {
	interface Window {
		engineCalls: EngineCall[];
	}
}

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

	test(`viewer, ${china ? 'China default-density' : `${buses}-bus`} generated grid to the first DC OPF card`, async ({
		page
	}) => {
		test.setTimeout(30 * 60_000);
		const pageErrors: string[] = [];
		page.on('pageerror', (error) => pageErrors.push(String(error)));
		await page.addInitScript(recordEngineCalls);
		await page.goto(`http://127.0.0.1:${port}/generated-handoff.html`);
		await expect(page.locator('body[data-ready="true"]')).toBeAttached({ timeout: 60_000 });

		const grid = await page.evaluate(
			(n) => window.generatedHandoff.generate(n),
			china ? null : buses
		);
		const open = async () => {
			const since = await page.evaluate(() => performance.now());
			const timing = await page.evaluate(() => window.generatedHandoff.open());
			const calls = await page.evaluate(
				(t) => window.engineCalls.filter((c) => c.sent >= t),
				since
			);
			return { timing, calls, since };
		};
		const cold = await open();
		const warm: Awaited<ReturnType<typeof open>>[] = [];
		for (let i = 0; i < runs; i++) warm.push(await open());

		const stages = ({ timing }: { timing: HandoffTiming }) => ({
			openBalancedModule: timing.open_ms,
			'DC OPF in state': timing.solution_ms,
			'solve card painted': timing.card_ms
		});
		const at = (since: number, t?: number) => (t === undefined ? '-' : (t - since).toFixed(0));
		console.log(
			[
				`viewer hand-off in Chromium (${mode}): ${grid.buses} buses, ` +
					`module_json ${(grid.module_json_chars / 1e6).toFixed(2)} M chars; ` +
					`generate ${grid.generate_ms.toFixed(0)} ms, ingest to page ${grid.ingest_ms.toFixed(0)} ms ` +
					`(not counted below: times run from the payload in hand)`,
				`solve card: ${cold.timing.verdict}`,
				...table(stages(cold), warm.map(stages)),
				'',
				'cold sample engine calls:',
				'| engine op | sent ms | back ms | worker ms |',
				'|---|---:|---:|---:|',
				...cold.calls.map(
					(c) =>
						`| ${c.op} | ${at(cold.since, c.sent)} | ${at(cold.since, c.back)} | ${c.worker?.toFixed(0) ?? '-'} |`
				)
			].join('\n')
		);

		expect(pageErrors).toEqual([]);
		for (const sample of [cold, ...warm]) {
			expect(sample.timing.formulation).toBe('dcopf');
			expect(sample.timing.verdict).toMatch(/DC OPF/);
			// The real hand-off passes the ingested payload: no JSON drop, one Study.
			const ops = sample.calls.map((c) => c.op);
			expect(ops).not.toContain('ingest_json_drop');
			expect(ops).toContain('study_new');
			for (const call of sample.calls) expect(call, call.op).toHaveProperty('back');
		}
	});
});

/** Record every engine worker round trip: op, page time sent and back, and the
 * worker's own time for it. Runs in the page before any of its scripts. */
function recordEngineCalls() {
	const calls: EngineCall[] = [];
	window.engineCalls = calls;
	// Request ids are per worker host.
	const pending = new WeakMap<Worker, Map<number, EngineCall>>();
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
				const ids = new Map<number, EngineCall>();
				pending.set(this, (byId = ids));
				this.addEventListener('message', (event: MessageEvent) => {
					const call = ids.get(event.data?.id);
					if (!call) return;
					ids.delete(event.data.id);
					call.back = performance.now();
					call.worker = event.data?.worker_ms;
				});
			}
			const call: EngineCall = { op: msg.op, sent: performance.now() };
			byId.set(msg.id, call);
			calls.push(call);
		}
		return post.call(this, message, options as StructuredSerializeOptions);
	};
}
