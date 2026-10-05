/** Opt-in browser benchmark for retained multiconductor PF sessions (#132).
 *
 * Skipped unless TELLEGEN_MC_BENCH=1. Modules are never committed: generate
 * them with the native harness, which also writes the matching edit sequence,
 *
 *   cargo run -p benchmarks --profile release-py --bin mc-pf-session-bench -- \
 *     --preset feeder-10k --write-module target/mc-pf-bench/modules/feeder-10k.pio.json
 *
 * then pass absolute paths (served through Vite's `/@fs/`):
 *
 *   TELLEGEN_MC_BENCH=1 \
 *   TELLEGEN_MC_BENCH_MODULES=$PWD/target/mc-pf-bench/modules/feeder-10k.pio.json \
 *   npx --prefix apps/web playwright test --config apps/web/playwright.config.ts mc-pf-bench
 *
 * Build the wasm first (`npm run wasm`). The config also starts the SvelteKit
 * preview server, so build the app or have a server on TELLEGEN_PREVIEW_PORT;
 * this benchmark only uses the multiconductor fixture server.
 *
 * Optional: TELLEGEN_MC_BENCH_EDITS (default 20), TELLEGEN_MC_BENCH_LABEL
 * (artifact label prefix), TELLEGEN_MC_BENCH_OUT (default target/mc-pf-bench).
 * Results go to `<out>/browser-<label>.json`. */

import { expect, test, type CDPSession, type Page } from '@playwright/test';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { basename, isAbsolute, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const enabled = process.env.TELLEGEN_MC_BENCH === '1';
const modules = (process.env.TELLEGEN_MC_BENCH_MODULES ?? '')
	.split(',')
	.map((path) => path.trim())
	.filter(Boolean);
const editCount = Number(process.env.TELLEGEN_MC_BENCH_EDITS ?? 20);
const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));
const outDir = resolve(repoRoot, process.env.TELLEGEN_MC_BENCH_OUT ?? 'target/mc-pf-bench');
const port = Number(
	process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
);

type Bench = Record<string, (...args: never[]) => unknown>;

function stats(values: number[]) {
	if (!values.length) return null;
	const sorted = [...values].sort((a, b) => a - b);
	const rank = (q: number) =>
		sorted[Math.min(sorted.length, Math.max(1, Math.ceil(q * sorted.length))) - 1];
	const mid = Math.floor(sorted.length / 2);
	return {
		n: sorted.length,
		min: sorted[0],
		median: sorted.length % 2 ? sorted[mid] : 0.5 * (sorted[mid - 1] + sorted[mid]),
		p95: rank(0.95),
		max: sorted[sorted.length - 1],
		mean: sorted.reduce((sum, value) => sum + value, 0) / sorted.length
	};
}

function step<T>(page: Page, name: string, ...args: unknown[]): Promise<T> {
	return page.evaluate(
		([name, args]) => {
			const bench = (window as unknown as { mcBench: Bench }).mcBench;
			return (bench[name] as (...values: unknown[]) => unknown)(...args);
		},
		[name, args] as const
	) as Promise<T>;
}

/** Main-thread JS heap from CDP `Performance.getMetrics`, before and after a forced GC. */
async function mainHeap(cdp: CDPSession) {
	const sample = async () => {
		const { metrics } = await cdp.send('Performance.getMetrics');
		const value = (name: string) => metrics.find((metric) => metric.name === name)?.value ?? null;
		return { used: value('JSHeapUsedSize'), total: value('JSHeapTotalSize') };
	};
	const before = await sample();
	await cdp.send('HeapProfiler.collectGarbage');
	const after = await sample();
	return {
		js_heap_used_bytes: before.used,
		js_heap_total_bytes: before.total,
		js_heap_used_after_gc_bytes: after.used
	};
}

/** Worker heaps through Chromium's worker targets (best effort). */
function workerHeaps(cdp: CDPSession) {
	const targets = new Map<string, string>();
	type HeapReply = { id?: number; result?: { usedSize: number; totalSize: number } };
	const replies = new Map<number, (message: HeapReply) => void>();
	let seq = 0;
	cdp.on('Target.attachedToTarget', (event) => {
		if (event.targetInfo.type === 'worker') targets.set(event.sessionId, event.targetInfo.url);
	});
	cdp.on('Target.detachedFromTarget', (event) => targets.delete(event.sessionId));
	cdp.on('Target.receivedMessageFromTarget', (event) => {
		const message = JSON.parse(event.message) as HeapReply;
		if (message.id !== undefined) replies.get(message.id)?.(message);
	});
	let attached: Promise<string | null> | null = null;
	const attach = () =>
		(attached ??= cdp
			.send('Target.setAutoAttach', {
				autoAttach: true,
				waitForDebuggerOnStart: false,
				flatten: false
			})
			.then(() => null)
			.catch((error: unknown) => String(error)));
	return async () => {
		const error = await attach();
		if (error) return { error };
		const heaps: Record<string, unknown> = {};
		for (const [sessionId, url] of targets) {
			const id = ++seq;
			const reply = new Promise<{ result?: { usedSize: number; totalSize: number } }>((done) => {
				replies.set(id, done);
				setTimeout(() => done({}), 5_000);
			});
			try {
				await cdp.send('Target.sendMessageToTarget', {
					sessionId,
					message: JSON.stringify({ id, method: 'Runtime.getHeapUsage' })
				});
				const message = await reply;
				heaps[basename(new URL(url).pathname)] = message.result
					? { used_bytes: message.result.usedSize, total_bytes: message.result.totalSize }
					: { error: 'no reply' };
			} catch (caught) {
				heaps[basename(new URL(url).pathname)] = { error: String(caught) };
			} finally {
				replies.delete(id);
			}
		}
		return heaps;
	};
}

test.describe('multiconductor session browser benchmark', () => {
	test.skip(!enabled, 'opt-in: set TELLEGEN_MC_BENCH=1 and TELLEGEN_MC_BENCH_MODULES');

	for (const modulePath of enabled ? modules : ['(disabled)']) {
		test(`retained session latency and memory: ${basename(modulePath)}`, async ({
			page,
			browser
		}) => {
			test.setTimeout(60 * 60_000);
			expect(isAbsolute(modulePath), 'module paths must be absolute').toBe(true);
			expect(existsSync(modulePath), `${modulePath} exists`).toBe(true);
			const pageErrors: string[] = [];
			page.on('pageerror', (error) => pageErrors.push(String(error)));

			const cdp = await page.context().newCDPSession(page);
			await cdp.send('Performance.enable');
			const sampleWorkers = workerHeaps(cdp);
			await page.goto(`http://127.0.0.1:${port}/bench.html`);
			await expect(page.locator('#status')).toHaveText('ready', { timeout: 60_000 });

			const stem = basename(modulePath).replace(/\.pio\.json$|\.json$/, '');
			const editsPath = modulePath.replace(/\.pio\.json$|\.json$/, '.edits.json');
			const memory: Record<string, unknown> = {};
			const load = await step(
				page,
				'load',
				`/@fs${modulePath}`,
				existsSync(editsPath) ? `/@fs${editsPath}` : null,
				editCount
			);
			memory.main_after_load = await mainHeap(cdp);

			// End to end through @tellegen/engine, as the Svelte controller runs it.
			const create = await step<Record<string, unknown>>(page, 'e2eCreate');
			memory.main_after_e2e_create = await mainHeap(cdp);
			memory.workers_after_e2e_create = await sampleWorkers();
			type EditRow = Record<string, number | null> & {
				profile: Record<string, number>;
			};
			const e2eEdits: EditRow[] = [];
			for (let index = 0; index < editCount; index++) {
				e2eEdits.push(await step(page, 'e2eEdit', index));
			}
			const feederWide = await step(page, 'e2eFeederWide');
			memory.main_after_e2e_edits = await mainHeap(cdp);
			memory.workers_after_e2e_edits = await sampleWorkers();
			const e2eMaterialize = await step(page, 'e2eMaterialize');
			await step(page, 'e2eFree');

			// Breakdown through a dedicated worker calling wasm McPfSession directly.
			const init = await step(page, 'breakdownInit');
			const breakdownCreate = await step<Record<string, unknown>>(page, 'breakdownCreate');
			memory.main_after_breakdown_create = await mainHeap(cdp);
			memory.workers_after_breakdown_create = await sampleWorkers();
			const breakdownEdits: Array<{
				replace: Record<string, number>;
				total_ms: number;
				iterations: number;
				profile: Record<string, number>;
			}> = [];
			for (let index = 0; index < editCount; index++) {
				breakdownEdits.push(await step(page, 'breakdownEdit', index));
			}
			memory.main_after_breakdown_edits = await mainHeap(cdp);
			memory.workers_after_breakdown_edits = await sampleWorkers();
			const onDemand = await step<Record<string, unknown>>(page, 'breakdownOnDemand');
			const breakdownMaterialize = await step<Record<string, unknown>>(
				page,
				'breakdownMaterialize'
			);
			await step(page, 'breakdownFree');

			const pick = <T>(rows: T[], value: (row: T) => number | null | undefined) =>
				stats(rows.map(value).filter((x): x is number => typeof x === 'number'));
			const label = process.env.TELLEGEN_MC_BENCH_LABEL
				? `${process.env.TELLEGEN_MC_BENCH_LABEL}-${stem}`
				: stem;
			const report = {
				schema: 'tellegen-mc-pf-browser-bench',
				version: 2,
				label,
				module: modulePath,
				browser: `chromium ${browser.version()}`,
				user_agent: await page.evaluate(() => navigator.userAgent),
				edits: editCount,
				controller_debounce_ms_excluded: await page.evaluate(
					() =>
						(window as unknown as { mcBench: { controllerDebounceMs: number } }).mcBench
							.controllerDebounceMs
				),
				notes: [
					'ui_visible_ms = await replaceLoadPowers(edits) (one round trip returning the summary) + one requestAnimationFrame, the controller flush path; the 180 ms debounce is excluded',
					'engine_ms is the engine worker time for the edit request (numerical solve plus summary JSON); transfer_ms is the rest of the round trip; parse_ms is the main-thread JSON.parse of the summary',
					'profile is the Rust phase split of the solve (load evaluation, KCL and matvec, retained LU solves, summary) timed with performance.now inside wasm',
					'detail_ms is the separate request an open results panel makes for one bus and 20 equipment ports; ui_visible_with_detail_ms adds it',
					'engine_heap_* come from the counting allocator in the shared engine worker; the edit peak is relative to the live heap before the edit',
					'legacy_result is the complete McPfResult JSON the pre-#132 session returned on every edit, built on demand once for comparison',
					'transfer_ms = main-thread receive time minus worker post time, both performance.timeOrigin-aligned; it includes structured-clone serialization and deserialization of the result string, and the alignment error of the two clocks (about a millisecond; small negative values mean below resolution)',
					'messaging_ms = main-thread round trip minus the worker-clock processing time: the offset-free request plus reply postMessage cost',
					'chars are JS string lengths; the payloads are ASCII JSON, so chars equal UTF-8 bytes',
					'wasm memory is the dedicated breakdown worker instance; the engine worker memory is not reachable from the page'
				],
				load,
				e2e: {
					create,
					edits: e2eEdits,
					ui_visible_ms: pick(e2eEdits, (row) => row.ui_visible_ms),
					ui_visible_with_detail_ms: pick(e2eEdits, (row) => row.ui_visible_with_detail_ms),
					replace_load_powers_ms: pick(e2eEdits, (row) => row.replace_load_powers_ms),
					engine_ms: pick(e2eEdits, (row) => row.engine_ms),
					transfer_ms: pick(e2eEdits, (row) => row.transfer_ms),
					parse_ms: pick(e2eEdits, (row) => row.parse_ms),
					payload_chars: pick(e2eEdits, (row) => row.payload_chars),
					detail_ms: pick(e2eEdits, (row) => row.detail_ms),
					detail_chars: pick(e2eEdits, (row) => row.detail_chars),
					iterations: pick(e2eEdits, (row) => row.iterations),
					profile_load_evaluation_ms: pick(e2eEdits, (row) => row.profile.load_evaluation_ms),
					profile_kcl_and_matvec_ms: pick(e2eEdits, (row) => row.profile.kcl_and_matvec_ms),
					profile_linear_solve_ms: pick(e2eEdits, (row) => row.profile.linear_solve_ms),
					profile_summary_ms: pick(e2eEdits, (row) => row.profile.summary_ms),
					profile_total_ms: pick(e2eEdits, (row) => row.profile.total_ms),
					engine_heap_edit_peak_bytes: pick(e2eEdits, (row) => row.engine_heap_edit_peak_bytes),
					engine_heap_live_change_bytes: pick(e2eEdits, (row) => row.engine_heap_live_change_bytes),
					engine_linear_memory_bytes_first_last: [
						e2eEdits[0]?.engine_linear_memory_bytes ?? null,
						e2eEdits.at(-1)?.engine_linear_memory_bytes ?? null
					],
					feeder_wide: feederWide,
					materialize: e2eMaterialize
				},
				breakdown: {
					init,
					create: breakdownCreate,
					edits: breakdownEdits,
					wasm_replace_ms: pick(breakdownEdits, (row) => row.replace.wasm_ms),
					summary_chars: pick(breakdownEdits, (row) => row.replace.chars),
					summary_transfer_ms: pick(breakdownEdits, (row) => row.replace.transfer_ms),
					summary_messaging_ms: pick(breakdownEdits, (row) => row.replace.messaging_ms),
					summary_parse_ms: pick(breakdownEdits, (row) => row.replace.parse_ms),
					edits_request_transfer_ms: pick(breakdownEdits, (row) => row.replace.request_transfer_ms),
					total_ms: pick(breakdownEdits, (row) => row.total_ms),
					profile_linear_solve_ms: pick(breakdownEdits, (row) => row.profile.linear_solve_ms),
					profile_total_ms: pick(breakdownEdits, (row) => row.profile.total_ms),
					wasm_memory_bytes_after_edits: breakdownEdits.at(-1)?.replace.wasm_memory_bytes ?? null,
					on_demand: onDemand,
					materialize: breakdownMaterialize
				},
				memory,
				page_errors: pageErrors
			};
			mkdirSync(outDir, { recursive: true });
			const outPath = resolve(outDir, `browser-${label}.json`);
			writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
			console.log(`wrote ${outPath}`);
			expect(pageErrors).toEqual([]);
			expect((create as { converged: boolean }).converged).toBe(true);
		});
	}
});
