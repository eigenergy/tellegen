import { expect, test } from './fixtures/page-errors.js';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

test('public-host action analytics use only approved fields and honor opt-out', async ({
	page,
	baseURL
}) => {
	const collected: Array<{ type: string; payload: Record<string, unknown> }> = [];
	await page.route('https://tellegen.dev/**', async (route) => {
		const address = new URL(route.request().url());
		const response = await route.fetch({ url: `${baseURL}${address.pathname}${address.search}` });
		await route.fulfill({ response });
	});
	await page.route('https://cloud.umami.is/script.js', (route) =>
		route.fulfill({
			contentType: 'application/javascript',
			body: `window.umami={track(payload){const clean=window.tellegenBeforeAnalyticsSend('event',payload);if(clean)return fetch('https://gateway.umami.is/api/send',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({type:'event',payload:clean})});}};`
		})
	);
	await page.route('https://gateway.umami.is/api/send', (route) => {
		if (route.request().method() === 'POST') collected.push(route.request().postDataJSON());
		return route.fulfill({
			status: 200,
			headers: {
				'Access-Control-Allow-Origin': 'https://tellegen.dev',
				'Access-Control-Allow-Headers': 'Content-Type'
			},
			json: {}
		});
	});
	await page.route('https://tellegen.dev/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('https://tellegen.dev/?PRIVATE_SECRET#PRIVATE_SECRET');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles([
		{ name: 'PRIVATE_SECRET.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'PRIVATE_SECRET.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	await expect(page.locator('.solvecard')).toContainText('OPF solve', { timeout: 60_000 });
	await expect.poll(() => collected.map((event) => event.payload.name)).toContain('case.select');
	await expect
		.poll(() => collected.map((event) => event.payload.name))
		.toContain('calculation.solve');
	expect(JSON.stringify(collected)).not.toContain('PRIVATE_SECRET');
	for (const event of collected) {
		expect(
			Object.keys(event.payload).every((key) =>
				['website', 'hostname', 'url', 'title', 'referrer', 'name', 'data'].includes(key)
			)
		).toBe(true);
		expect(event.payload.referrer).toBe('');
		expect(event.payload.url).toBe('/');
		expect(event.payload.title).toBe('Tellegen');
	}
	await page.goto('https://tellegen.dev/privacy/');
	await expect.poll(() => collected.some((event) => event.payload.title === 'Privacy')).toBe(true);
	await page.getByRole('checkbox', { name: 'Disable usage analytics in this browser' }).check();
	const count = collected.length;
	await page.evaluate(async () => {
		const tracker = (window as unknown as { umami: { track(data: unknown): Promise<void> } }).umami;
		await tracker.track({
			name: 'case.select',
			url: '/?PRIVATE_SECRET',
			data: { source: 'demo', demo_case: 'case7000' }
		});
	});
	expect(collected).toHaveLength(count);
});
