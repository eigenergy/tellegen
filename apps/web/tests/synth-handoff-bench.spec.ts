/** The generated-grid hand-off to the viewer (#261): one test that always
 * runs, and opt-in browser benchmarks.
 *
 * The benchmarks are skipped unless TELLEGEN_SYNTH_BENCH=1. Build the wasm (`npm run wasm`)
 * first; the config starts the fixture server. How to run it and the recorded
 * results are in `validation/evidence/synth/handoff/README.md`.
 *
 *   TELLEGEN_SYNTH_BENCH=1 npx --prefix apps/web playwright test \
 *     --config apps/web/playwright.config.ts synth-handoff-bench
 *
 * - `engine hand-off` times the public engine path on its own (fixture page),
 *   from the grid being ready on the synthetic worker to the first DC OPF
 *   answer parsed on the page, split into each build, copy and parse.
 * - `viewer` mounts tellegen's shell on a fixture page and opens the
 *   generated grid the way MapToGrid's hand-off does: the ingested payload
 *   straight to `Controller.openBalancedModule` with `formulation: 'dcopf'`
 *   (no `ingest_json_drop`), timed from the grid being ready to the first
 *   frame showing the DC OPF solve card. Every sample is a reloaded page, so
 *   each is the first grid that viewer opens.
 *
 * The split comes from the engine workers: each answer carries when the
 * worker took the request, how long the engine ran, and when the answer was
 * posted, on the clock the page reads (`performance.timeOrigin +
 * performance.now()`).
 *
 * Playwright's default headless shell renders WebGL in software
 * (SwiftShader). TELLEGEN_SYNTH_BENCH_GPU=1 runs Chromium's new headless
 * mode with hardware GL instead, to separate rendering from engine time.
 *
 * Optional: TELLEGEN_SYNTH_BENCH_PATH=serial (the hand-off as it was before
 * #261: the payload, then the Study; the default `ahead` starts the Study
 * before the payload is prepared), TELLEGEN_SYNTH_BENCH_BUSES (default
 * 3000), TELLEGEN_SYNTH_BENCH_RUNS (default 5), TELLEGEN_SYNTH_BENCH_CHINA=1 (the viewer test generates
 * MapToGrid's request for China at its default density instead). The first
 * sample is cold (browser start, worker start, wasm compile, places table)
 * and reported apart from the median of the rest. */

import { expect, test, type Page } from '@playwright/test';
import type { HandoffTiming } from './fixtures/mc-pf-browser/generated-handoff';

const enabled = process.env.TELLEGEN_SYNTH_BENCH === '1';
const gpu = process.env.TELLEGEN_SYNTH_BENCH_GPU === '1';
const china = process.env.TELLEGEN_SYNTH_BENCH_CHINA === '1';
const buses = Number(process.env.TELLEGEN_SYNTH_BENCH_BUSES ?? 3000);
const runs = Number(process.env.TELLEGEN_SYNTH_BENCH_RUNS ?? 5);
/** `ahead` (the default) starts the Study before the payload is prepared;
 * `serial` is the hand-off as it was before #261. */
const path: 'serial' | 'ahead' =
	process.env.TELLEGEN_SYNTH_BENCH_PATH === 'serial' ? 'serial' : 'ahead';
const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);

/** One engine worker round trip. Every time is in milliseconds since the Unix
 * epoch; `received` and `posted` are the worker's, `sent` and `back` the page's. */
type EngineCall = {
	op: string;
	sent: number;
	received?: number;
	worker_ms?: number;
	posted?: number;
	back?: number;
};
type EngineSample = { at: Record<string, number>; buses: number; module_json_chars: number };
type Stages = Record<string, number>;

declare global {
	interface Window {
		engineCalls: EngineCall[];
		longTaskMs(since: number): number;
	}
}

const median = (xs: number[]) => {
	const s = [...xs].sort((a, b) => a - b);
	const m = Math.floor(s.length / 2);
	return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

function table(cold: Stages, rest: Stages[]) {
	const rows = Object.keys(cold).map((name) => {
		const xs = rest.map((w) => w[name]);
		return `| ${name} | ${cold[name].toFixed(0)} | ${median(xs).toFixed(0)} | ${Math.min(...xs).toFixed(0)} to ${Math.max(...xs).toFixed(0)} |`;
	});
	return [
		`| stage | cold ms | median ms (n=${rest.length}) | min to max ms |`,
		'|---|---:|---:|---:|',
		...rows
	];
}

/** The parts of one worker round trip: the request's copy to the worker, the
 * wait for the worker's wasm (a worker's first request only), the engine's
 * own time, and the answer's copy back to the page. */
function trip(call: EngineCall) {
	const { sent, received, worker_ms, posted, back } = call;
	if (received == null || worker_ms == null || posted == null || back == null)
		throw new Error(`${call.op} did not report its times`);
	return {
		toWorker: received - sent,
		engineWait: Math.max(0, posted - worker_ms - received),
		engine: worker_ms,
		toPage: back - posted
	};
}

function the(calls: EngineCall[], op: string): EngineCall {
	const found = calls.find((c) => c.op === op);
	if (!found) throw new Error(`no ${op} call in ${calls.map((c) => c.op).join(', ')}`);
	return found;
}

async function browserLine(page: Page) {
	const version = page.context().browser()?.version() ?? 'unknown';
	const cores = await page.evaluate(() => navigator.hardwareConcurrency);
	return `Chromium ${version}, ${process.platform} ${process.arch}, ${cores} logical cores`;
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

test('a generated grid opens in the viewer on the Study built ahead of it', async ({ page }) => {
	const pageErrors: string[] = [];
	page.on('pageerror', (error) => pageErrors.push(String(error)));
	await page.addInitScript(recordEngineCalls);
	await page.goto(`http://127.0.0.1:${port}/generated-handoff.html`);
	await expect(page.locator('body[data-ready="true"]')).toBeAttached();
	await page.evaluate(() => window.generatedHandoff.generate(200));
	const timing = await page.evaluate(() => window.generatedHandoff.open('ahead'));
	expect(timing.buses).toBe(200);
	expect(timing.calculation).toBe('dcopf');
	expect(timing.status).toMatch(/optimal|solved/i);
	const calls = await page.evaluate(
		(t) => window.engineCalls.filter((c) => c.sent >= t),
		timing.ready
	);
	const ops = calls.map((c) => c.op);
	// The Study starts from the module text before the payload is asked for,
	// and the viewer solves on that Study: it builds no second one.
	expect(ops.slice(0, 3)).toEqual([
		'synth_session_module',
		'study_new',
		'synth_session_ingest_view'
	]);
	expect(ops.filter((op) => op === 'study_new')).toHaveLength(1);
	expect(ops).not.toContain('synth_session_ingest');
	// The answer carries the worker's times, which the benchmarks below split by.
	expect(trip(the(calls, 'study_new')).engine).toBeGreaterThan(0);
	expect(pageErrors).toEqual([]);
});

test.describe('generated-grid hand-off browser benchmark', () => {
	test.skip(!enabled, 'opt-in: set TELLEGEN_SYNTH_BENCH=1');
	const mode = gpu ? 'hardware GL' : 'headless shell';

	test(`engine hand-off, ${buses}-bus generated grid`, async ({ page }) => {
		test.setTimeout(30 * 60_000);
		const pageErrors: string[] = [];
		page.on('pageerror', (error) => pageErrors.push(String(error)));
		await page.addInitScript(recordEngineCalls);
		await page.goto(`http://127.0.0.1:${port}/synth-handoff.html`);
		await expect(page.locator('#status')).toHaveText('ready', { timeout: 60_000 });
		const sample = async () => {
			const result: EngineSample = await page.evaluate(
				([n, how]) =>
					(
						window as unknown as {
							synthHandoff: { run(n: number, how: string): Promise<EngineSample> };
						}
					).synthHandoff.run(n, how),
				[buses, path] as const
			);
			const calls = await page.evaluate(
				(t) => window.engineCalls.filter((c) => c.sent >= t),
				result.at.generated
			);
			return { ...result, stages: engineStages(result.at, calls) };
		};
		const cold = await sample();
		const warm: Awaited<ReturnType<typeof sample>>[] = [];
		for (let i = 0; i < runs; i++) warm.push(await sample());
		console.log(
			[
				`engine hand-off, ${path} (${mode}; ${await browserLine(page)}): ${cold.buses} buses, ` +
					`module_json ${(cold.module_json_chars / 1e6).toFixed(2)} M chars`,
				...table(
					cold.stages,
					warm.map((w) => w.stages)
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

		// Every sample loads the page and generates again, so each one is a
		// first grid: the viewer's map and study worker start with the page, as
		// they do when MapToGrid hands its grid over.
		const open = async () => {
			await page.goto(`http://127.0.0.1:${port}/generated-handoff.html`);
			await expect(page.locator('body[data-ready="true"]')).toBeAttached({ timeout: 60_000 });
			const grid = await page.evaluate(
				(n) => window.generatedHandoff.generate(n),
				china ? null : buses
			);
			const timing = await page.evaluate((how) => window.generatedHandoff.open(how), path);
			const calls = await page.evaluate(
				(t) => window.engineCalls.filter((c) => c.sent >= t),
				timing.ready
			);
			const busy = await page.evaluate((t) => window.longTaskMs(t), timing.ready);
			return { grid, timing, calls, stages: viewerStages(timing, calls, busy) };
		};
		const first = await open();
		const rest: Awaited<ReturnType<typeof open>>[] = [];
		for (let i = 0; i < runs; i++) rest.push(await open());

		const { timing } = first;
		const at = (t?: number) => (t === undefined ? '-' : (t - timing.ready).toFixed(0));
		console.log(
			[
				`viewer hand-off, ${path} (${mode}; ${await browserLine(page)}): ${timing.buses} buses, ` +
					`module_json ${(timing.module_json_chars / 1e6).toFixed(2)} M chars; ` +
					`generate ${first.grid.generate_ms.toFixed(0)} ms cold, ` +
					`${median(rest.map((r) => r.grid.generate_ms)).toFixed(0)} ms median. ` +
					`Every sample is a newly loaded page; "cold" is the first after the browser started.`,
				...table(
					first.stages,
					rest.map((r) => r.stages)
				),
				'',
				'cold sample engine calls, ms after the grid was ready:',
				'| engine op | sent | worker received | worker ms | worker posted | page received |',
				'|---|---:|---:|---:|---:|---:|',
				...first.calls.map(
					(c) =>
						`| ${c.op} | ${at(c.sent)} | ${at(c.received)} | ${c.worker_ms?.toFixed(0) ?? '-'} | ${at(c.posted)} | ${at(c.back)} |`
				)
			].join('\n')
		);

		expect(pageErrors).toEqual([]);
		for (const sample of [first, ...rest]) {
			expect(sample.timing.formulation).toBe('dcopf');
			expect(sample.timing.calculation).toBe('dcopf');
			expect(sample.timing.status).toMatch(/optimal|solved/i);
			// The real hand-off passes the ingested payload: no JSON drop, one Study.
			const ops = sample.calls.map((c) => c.op);
			expect(ops).not.toContain('ingest_json_drop');
			expect(ops.filter((op) => op === 'study_new')).toHaveLength(1);
			for (const call of sample.calls) expect(call, call.op).toHaveProperty('back');
		}
	});
});

/** The engine hand-off of one sample, from the grid being ready on the
 * synthetic worker to the first DC OPF answer parsed on the page. */
function engineStages(at: Record<string, number>, calls: EngineCall[]): Stages {
	// The `ahead` path asks for the module text, then for the payload without it.
	const ingestCall =
		calls.find((c) => c.op === 'synth_session_ingest_view') ?? the(calls, 'synth_session_ingest');
	const ingest = trip(ingestCall);
	const buildCall = the(calls, 'study_new');
	const build = trip(buildCall);
	const moduleCall = calls.find((c) => c.op === 'synth_session_module');
	const moduleText = moduleCall ? trip(moduleCall) : null;
	const fetchCall = the(calls, 'study_solution');
	return {
		'module serialized on the synth worker': moduleText?.engine ?? 0,
		'copy of the module text, synth worker to page': moduleText?.toPage ?? 0,
		'payload built and serialized on the synth worker': ingest.engine,
		'copy of the payload, synth worker to page': ingest.toPage,
		'payload parsed on the page': at.ingested - ingestCall.back!,
		'copy of the module text, page to study worker': build.toWorker,
		'wait for the study worker to load wasm': build.engineWait,
		'Study built: module parsed, DC OPF solved': build.engine,
		'DC OPF result fetched and parsed': at.solved - fetchCall.sent,
		'grid ready to payload on the page': at.ingested - at.generated,
		'grid ready to Study built': buildCall.posted! - at.generated,
		'hand-off: grid ready to first DC OPF result': at.solved - at.generated
	};
}

/** The viewer hand-off of one sample, from the grid being ready on the
 * synthetic worker to the frame that shows the solved DC OPF card. The page
 * rows overlap the worker rows: the page draws while the study worker builds. */
function viewerStages(timing: HandoffTiming, calls: EngineCall[], busy: number): Stages {
	const build = trip(the(calls, 'study_new'));
	const fetches = calls.filter((c) => c.op === 'study_solution');
	return {
		'grid brought to the page (payload built, copied, parsed)': timing.start - timing.ready,
		'grid ready to Study built': the(calls, 'study_new').posted! - timing.ready,
		'openBalancedModule (case built on the page)': timing.opened - timing.start,
		'copy, page to study worker': build.toWorker,
		'wait for the study worker to load wasm': build.engineWait,
		'Study built: module parsed, DC OPF solved': build.engine,
		'Study answer waits for the page': build.toPage,
		'DC OPF result fetches, including waits for the page': fetches.reduce(
			(sum, c) => sum + (c.back! - c.sent),
			0
		),
		'page main thread in long tasks (overlaps the rows above)': busy,
		'hand-off: grid ready to DC OPF result in state': timing.solved - timing.ready,
		'hand-off: grid ready to solve card painted': timing.card - timing.ready
	};
}

/** Record every engine worker round trip: op, the page's time sent and back,
 * and the worker's own times for it, plus the page's long tasks. Runs in the
 * page before any of its scripts. */
function recordEngineCalls() {
	const now = () => performance.timeOrigin + performance.now();
	const calls: EngineCall[] = [];
	window.engineCalls = calls;
	const longTasks: { start: number; ms: number }[] = [];
	new PerformanceObserver((list) => {
		for (const entry of list.getEntries())
			longTasks.push({ start: performance.timeOrigin + entry.startTime, ms: entry.duration });
	}).observe({ type: 'longtask', buffered: true });
	window.longTaskMs = (since) =>
		longTasks.filter((t) => t.start + t.ms > since).reduce((sum, t) => sum + t.ms, 0);
	// Request ids are per worker host.
	const pending = new WeakMap<Worker, Map<number, EngineCall>>();
	const NativeWorker = window.Worker;
	// Listen from construction, before the engine host sets `onmessage`: the
	// answer is then stamped before the host's handler and the promise
	// callbacks that parse it run.
	window.Worker = class extends NativeWorker {
		constructor(url: string | URL, options?: WorkerOptions) {
			super(url, options);
			const ids = new Map<number, EngineCall>();
			pending.set(this, ids);
			this.addEventListener('message', (event: MessageEvent) => {
				const call = ids.get(event.data?.id);
				if (!call) return;
				ids.delete(event.data.id);
				call.back = now();
				call.worker_ms = event.data?.worker_ms;
				call.received = event.data?.received_at;
				call.posted = event.data?.posted_at;
			});
		}
	};
	const post = NativeWorker.prototype.postMessage;
	NativeWorker.prototype.postMessage = function (
		this: Worker,
		message: unknown,
		options?: Transferable[] | StructuredSerializeOptions
	) {
		const msg = message as { id?: unknown; op?: unknown };
		const ids = pending.get(this);
		if (ids && typeof msg?.id === 'number' && typeof msg.op === 'string') {
			const call: EngineCall = { op: msg.op, sent: now() };
			ids.set(msg.id, call);
			calls.push(call);
		}
		return post.call(this, message, options as StructuredSerializeOptions);
	};
}
