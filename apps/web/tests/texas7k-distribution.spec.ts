import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { expect, test } from './fixtures/page-errors.js';
import { callTool, installWebMcpHarness } from './fixtures/planning-case.js';

// Run against the actual server serving staged data and the production build.
test.skip(process.env.TELLEGEN_TEXAS7K !== '1', 'Requires the staged Texas7k distribution pilot');
test.use({ trace: 'off' });
test('Texas7k hosted pilot solves, edits and reopens with its recorded options', async ({
	page,
	request,
	browser
}, testInfo) => {
	test.setTimeout(180_000);
	page.setDefaultTimeout(30_000);
	const id = 'texas7k-p1uhs0_1247';
	const catalogue = await (await request.get('/api/cases')).json();
	const entry = catalogue.find((item: { id: string }) => item.id === id);
	expect(entry).toMatchObject({
		model: 'multiconductor',
		n_bus: 22076,
		distribution: {
			related_case_id: 'case7000',
			pf_options: {
				voltage_envelope: false,
				max_iterations: 500,
				tolerance: 1e-7,
				absolute_kcl_tolerance: 1e-5
			}
		}
	});
	await installWebMcpHarness(page);
	const browserEngine = browser.browserType().name();
	const cdp = browserEngine === 'chromium' ? await browser.newBrowserCDPSession() : null;
	let peakBrowserRssBytes = 0;
	const memorySamples: Array<{ phase: string; rss_bytes: number }> = [];
	const sampleMemory = async (phase: string) => {
		if (!cdp || process.platform === 'win32') return;
		const { processInfo } = await cdp.send('SystemInfo.getProcessInfo');
		const { stdout } = await promisify(execFile)('ps', [
			'-o',
			'rss=',
			'-p',
			processInfo.map((p) => p.id).join(',')
		]);
		const rss =
			stdout
				.trim()
				.split(/\s+/)
				.reduce((sum, v) => sum + Number(v), 0) * 1024;
		memorySamples.push({ phase, rss_bytes: rss });
		peakBrowserRssBytes = Math.max(peakBrowserRssBytes, rss);
	};
	await sampleMemory('before_load');
	await page.addInitScript(() => {
		const durations: number[] = [];
		Object.defineProperty(window, '__texasLongTasks', { value: durations });
		if (PerformanceObserver.supportedEntryTypes.includes('longtask')) {
			new PerformanceObserver((list) =>
				durations.push(...list.getEntries().map((e) => e.duration))
			).observe({ type: 'longtask', buffered: true });
		}
	});
	const started = Date.now();
	await page.goto('/');
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');
	// WebMCP registers before the asynchronous catalogue load completes.
	await expect
		.poll(async () => {
			const listed = await callTool(page, 'list_cases', {});
			return (
				listed.ok &&
				Array.isArray(listed.data.cases) &&
				listed.data.cases.some((item: { case_id: string }) => item.case_id === id)
			);
		})
		.toBe(true);

	// Case-level navigation also works when the transmission dataset is staged.
	if (catalogue.some((item: { id: string }) => item.id === 'case7000')) {
		expect(await callTool(page, 'select_case', { case_id: 'case7000' })).toMatchObject({
			ok: true
		});
		await page
			.getByRole('button', { name: 'Explore associated distribution', exact: true })
			.click();
	} else {
		expect(await callTool(page, 'select_case', { case_id: id })).toMatchObject({ ok: true });
	}
	await expect(page.getByRole('button', { name: 'Run power flow', exact: true })).toBeEnabled();
	await expect(page.getByText('Geographic', { exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Dataset and attribution' })).toHaveAttribute(
		'href',
		entry.distribution.source_url
	);
	await sampleMemory('loaded');
	console.info('Texas7k: loaded');
	const loadMs = Date.now() - started;
	const solveStart = Date.now();
	await page.getByRole('button', { name: 'Run power flow', exact: true }).click();
	await expect(page.getByRole('table', { name: 'Terminal results', exact: true })).toBeVisible({
		timeout: 90_000
	});
	await sampleMemory('solved');
	console.info('Texas7k: solved');
	const solveMs = Date.now() - solveStart;
	const inspected = await callTool(page, 'inspect_case', {});
	if (!inspected.ok) throw new Error(inspected.error.message);
	const webmcp = await callTool(page, 'solve_multiconductor_pf', {
		case_id: id,
		expected_revision: inspected.data.revision
	});
	expect(webmcp).toMatchObject({ ok: true, data: { converged: true, terminal_count: 55972 } });
	if (!(await page.getByRole('region', { name: 'Study workspace' }).isVisible()))
		await page.getByRole('button', { name: 'Studies', exact: true }).click();
	const downloadWait = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const download = await downloadWait;
	const exported = await download.path();
	if (!exported) throw new Error('Missing study export');
	await sampleMemory('exported');
	console.info('Texas7k: exported');
	const exportText = await readFile(exported, 'utf8');
	expect(Buffer.byteLength(exportText)).toBeLessThan(128 * 1024 * 1024);
	const snapshot = JSON.parse(exportText);
	expect(snapshot.options).toMatchObject(entry.distribution.pf_options);
	expect(snapshot.result.converged).toBe(true);
	expect(snapshot.result.iterations).toBe(23);
	const resultHash = createHash('sha256').update(JSON.stringify(snapshot.result)).digest('hex');
	const reference = JSON.parse(
		await readFile(
			new URL(
				'../../../docs/evidence/studies/texas7k-reconversion/shipping-wasm-validation-2026-11-03.json',
				import.meta.url
			),
			'utf8'
		)
	);
	expect(resultHash).toBe(reference.reduced.result_sha256);
	const originalOptions = snapshot.options;
	console.info('Texas7k: importing');
	await page.locator('label.file-button input[type="file"]').setInputFiles(exported);
	await expect
		.poll(
			async () => {
				const value = await callTool(page, 'inspect_case', {});
				return value.ok ? value.data.case_id : '';
			},
			{ timeout: 90_000 }
		)
		.toMatch(/^dist-/);
	await expect(
		page.getByRole('region', { name: 'Study workspace' }).getByText('Converged', { exact: true })
	).toBeVisible();
	console.info('Texas7k: imported');
	// A fresh run after import must preserve the hosted options.
	await page
		.getByRole('region', { name: 'Study workspace' })
		.getByRole('button', { name: 'Run power flow' })
		.click();
	await expect(
		page.getByRole('region', { name: 'Study workspace' }).getByText('Converged', { exact: true })
	).toBeVisible({ timeout: 90_000 });
	console.info('Texas7k: resolved');
	await expect(
		page
			.getByRole('region', { name: 'Study workspace' })
			.getByRole('button', { name: 'Run power flow' })
	).toBeEnabled({ timeout: 90_000 });
	const secondDownload = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Export', exact: true }).click();
	const secondPath = await (await secondDownload).path();
	await sampleMemory('reopened_and_resolved');
	const reopened = JSON.parse(await readFile(secondPath!, 'utf8'));
	expect(reopened.options).toEqual(originalOptions);
	expect(createHash('sha256').update(JSON.stringify(reopened.result)).digest('hex')).toBe(
		resultHash
	);
	console.info('Texas7k: reexported');
	const current = await callTool(page, 'inspect_case', {});
	if (!current.ok) throw new Error(current.error.message);
	expect(
		await callTool(page, 'focus_network', {
			case_id: current.data.case_id,
			target: { kind: 'bus', element_id: 'p1ulv10000' }
		})
	).toMatchObject({ ok: true });
	console.info('Texas7k: focused');
	const power = page.getByLabel('P kW', { exact: true }).first();
	if (!(await power.isVisible()))
		await page.getByRole('button', { name: 'Network', exact: true }).click();
	await expect(power).toBeVisible();
	const base = Number(await power.inputValue());
	const beforeEdit = await callTool(page, 'inspect_case', {});
	if (!beforeEdit.ok) throw new Error(beforeEdit.error.message);
	const editStart = Date.now();
	await power.fill(String(base * 1.01));
	await power.press('Tab');
	// A zero-count check can pass before the debounced edit starts. Wait for
	// the accepted result's revision to advance and the calculation to finish.
	await expect
		.poll(
			async () => {
				const value = await callTool(page, 'inspect_case', {});
				if (!value.ok) return null;
				return {
					case_id: value.data.case_id,
					revision_changed: value.data.revision !== beforeEdit.data.revision,
					solving: value.data.solving,
					converged: (value.data.solution as { converged?: boolean } | null)?.converged
				};
			},
			{ timeout: 90_000 }
		)
		.toEqual({
			case_id: beforeEdit.data.case_id,
			revision_changed: true,
			solving: false,
			converged: true
		});
	await expect(power).toHaveValue(String(base * 1.01));
	const editMs = Date.now() - editStart;
	const transfer = await page.evaluate(() =>
		performance
			.getEntriesByType('resource')
			.filter((r) => r.name.includes('/case'))
			.map((r) => {
				const t = r as PerformanceResourceTiming;
				return {
					url: new URL(t.name).pathname,
					encoded_bytes: t.encodedBodySize,
					decoded_bytes: t.decodedBodySize,
					duration_ms: t.duration
				};
			})
	);
	await page.screenshot({ path: testInfo.outputPath('texas7k-distribution.png') });
	await sampleMemory('edited');
	const longTasks = await page.evaluate(
		() => (window as unknown as { __texasLongTasks: number[] }).__texasLongTasks
	);
	const longTasksSupported = await page.evaluate(() =>
		PerformanceObserver.supportedEntryTypes.includes('longtask')
	);
	const server = new URL(testInfo.project.use.baseURL!);
	const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(server.hostname);
	const metrics = {
		server_origin: server.origin,
		browser_rss_max_at_checkpoints_bytes: memorySamples.length ? peakBrowserRssBytes : null,
		memory_samples: memorySamples,
		browser_engine: browserEngine,
		browser: browser.version(),
		viewport: page.viewportSize(),
		mobile_emulation: Boolean(testInfo.project.use.isMobile),
		memory_note: memorySamples.length
			? 'Sum of Chromium process RSS at workflow checkpoints, not a continuously sampled peak; shared pages may be counted more than once.'
			: 'Browser process RSS is unavailable through this browser engine; no memory qualification is claimed.',
		long_task_observation_supported: longTasksSupported,
		main_thread_long_task_count: longTasksSupported ? longTasks.length : null,
		main_thread_longest_task_ms: longTasksSupported ? Math.max(0, ...longTasks) : null,
		load_ms: loadMs,
		ui_solve_ms: solveMs,
		edit_ms: editMs,
		edit_result_revision_advanced: true,
		result_sha256: resultHash,
		transfer,
		note: `Single ${browserEngine} run against ${loopback ? 'a loopback server' : 'the configured public HTTPS server'}; includes network and UI scheduling, with no controlled cache or host-load conditions. Viewport/device emulation does not qualify physical mobile memory or performance.`
	};
	await writeFile(
		testInfo.outputPath('texas7k-browser.json'),
		JSON.stringify(metrics, null, 2) + '\n'
	);
	console.info(JSON.stringify(metrics));
});
