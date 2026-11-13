import { expect, test } from './fixtures/page-errors.js';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { callTool, describeTool, installWebMcpHarness } from './fixtures/planning-case';

test('agents open case text, export it through PowerIO, and inspect the registration', async ({
	page
}) => {
	await installWebMcpHarness(page);
	await page.route('**/api/cases', (route) => {
		void route.fulfill({ json: [] });
	});
	// Production builds register the developer check only with `?debug`.
	await page.goto('/?debug');
	await expect(page.locator('html')).toHaveAttribute('data-webmcp', 'ready');

	// The debug tool is marked for developer tooling and reports the live registration.
	expect((await describeTool(page, 'inspect_webmcp_registration')).annotations).toEqual({
		readOnlyHint: true,
		debugging: true
	});
	const status = await callTool(page, 'inspect_webmcp_registration', {});
	expect(status).toMatchObject({
		ok: true,
		data: { registration_error: null, exposed_to: [], model_context: { register_tool: true } }
	});
	if (!status.ok) throw new Error('unreachable');
	expect(status.data.tools).toEqual(expect.arrayContaining(['import_case', 'export_case']));
	for (const name of ['import_case', 'export_case', 'update_case'])
		expect((await describeTool(page, name)).annotations).toEqual({
			readOnlyHint: false,
			untrustedContentHint: true
		});

	// Requests the tool cannot open are refused before PowerIO runs.
	expect(
		await callTool(page, 'import_case', { file_name: 'layer.geojson', text: '{}' })
	).toMatchObject({ ok: false, error: { code: 'INVALID_INPUT' } });
	expect(
		await callTool(page, 'import_case', { file_name: 'broken.m', text: 'not a matpower case' })
	).toMatchObject({ ok: false, error: { code: 'IMPORT_FAILED' } });

	const opened = await callTool(page, 'import_case', { file_name: 'case14.m', text: CASE14 });
	expect(opened, JSON.stringify(opened)).toMatchObject({
		ok: true,
		data: { opened: [{ kind: 'local', calculation: 'balanced' }], view: 'live_case' }
	});
	if (!opened.ok) throw new Error('unreachable');
	const caseId = (opened.data.opened as { case_id: string }[])[0].case_id;
	expect(opened.data.case_id).toBe(caseId);

	await expect
		.poll(async () => {
			const inspected = await callTool(page, 'inspect_case', {});
			return inspected.ok ? inspected.data.case_id : inspected.error.code;
		})
		.toBe(caseId);
	const inspected = await callTool(page, 'inspect_case', {});
	if (!inspected.ok) throw new Error(inspected.error.message);
	const revision = inspected.data.revision as string;

	expect(
		await callTool(page, 'export_case', {
			case_id: caseId,
			expected_revision: 'stale',
			format: 'matpower'
		})
	).toMatchObject({ ok: false, error: { code: 'STALE_REVISION' } });

	for (const [format, extension] of [
		['psse', '.raw'],
		['pio-module', '.pio.json']
	] as const) {
		const download = page.waitForEvent('download');
		const exported = await callTool(page, 'export_case', {
			case_id: caseId,
			expected_revision: revision,
			format
		});
		expect(exported, JSON.stringify(exported)).toMatchObject({
			ok: true,
			data: { format, downloaded: true, case_id: caseId, revision }
		});
		if (!exported.ok) throw new Error('unreachable');
		expect(exported.data.file_name).toMatch(new RegExp(`${extension.replace('.', '\\.')}$`));
		expect(exported.data.bytes).toBeGreaterThan(100);
		expect((await download).suggestedFilename()).toBe(exported.data.file_name);
	}
});
