import { expect, test } from './fixtures/page-errors.js';
import { CASE14 } from '../../../examples/browser-minimal/src/case14';
import { CASE14_COORDS } from './fixtures/local-case';

// The public host's usage counts go straight to Umami's collection address
// from the site's own code. The test serves the preview as tellegen.dev,
// answers the collector itself, and reads every payload that leaves.
test('public-host usage counts post only approved fields, carry the cache token and honor opt-out', async ({
	page,
	baseURL
}) => {
	const collected: Array<{
		headers: Record<string, string>;
		body: { type: string; payload: Record<string, unknown> };
	}> = [];
	let scripts = 0;
	await page.route('https://tellegen.dev/**', async (route) => {
		const address = new URL(route.request().url());
		const response = await route.fetch({ url: `${baseURL}${address.pathname}${address.search}` });
		await route.fulfill({ response });
	});
	await page.route('https://cloud.umami.is/**', (route) => {
		scripts++;
		return route.abort();
	});
	await page.route('https://gateway.umami.is/api/send', (route) => {
		const request = route.request();
		if (request.method() === 'POST')
			collected.push({ headers: request.headers(), body: request.postDataJSON() });
		return route.fulfill({
			status: 200,
			headers: {
				'Access-Control-Allow-Origin': '*',
				'Access-Control-Allow-Headers': '*'
			},
			json: { cache: 'c1', sessionId: 'session', visitId: 'visit' }
		});
	});
	await page.route('https://tellegen.dev/api/cases', (route) => route.fulfill({ json: [] }));
	await page.goto('https://tellegen.dev/?PRIVATE_SECRET#PRIVATE_SECRET');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.locator('input[type=file]').setInputFiles([
		{ name: 'PRIVATE_SECRET.m', mimeType: 'text/plain', buffer: Buffer.from(CASE14) },
		{ name: 'PRIVATE_SECRET.csv', mimeType: 'text/csv', buffer: Buffer.from(CASE14_COORDS) }
	]);
	await expect(page.locator('.solvecard')).toContainText('DC OPF', { timeout: 60_000 });
	await expect
		.poll(() => collected.map((event) => event.body.payload.name))
		.toContain('case.select');
	await expect
		.poll(() => collected.map((event) => event.body.payload.name))
		.toContain('calculation.solve');
	expect(scripts).toBe(0);
	expect(JSON.stringify(collected)).not.toContain('PRIVATE_SECRET');
	for (const event of collected) {
		expect(event.body.type).toBe('event');
		expect(event.headers['content-type']).toBe('application/json');
		expect(
			Object.keys(event.body.payload).every((key) =>
				['website', 'hostname', 'url', 'title', 'referrer', 'name', 'data'].includes(key)
			)
		).toBe(true);
		expect(event.body.payload.hostname).toBe('tellegen.dev');
		expect(event.body.payload.referrer).toBe('');
		expect(event.body.payload.url).toBe('/');
		expect(event.body.payload.title).toBe('Tellegen');
	}
	// The collector's cache token is kept in memory for the page and travels
	// with every later request: the solve comes seconds after the first event.
	const solve = collected.find((event) => event.body.payload.name === 'calculation.solve')!;
	expect(solve.headers['x-umami-cache']).toBe('c1');
	await page.goto('https://tellegen.dev/privacy/');
	await expect
		.poll(() => collected.find((event) => event.body.payload.title === 'Privacy'))
		.toBeTruthy();
	await page.getByRole('checkbox', { name: 'Disable usage analytics in this browser' }).check();
	await expect(page.getByRole('status')).toContainText('Analytics are disabled in this browser.');
	const count = collected.length;
	// A reload would report a page view if the choice were not kept.
	await page.goto('https://tellegen.dev/');
	await expect(page.getByText('no default cases loaded')).toBeVisible();
	await page.waitForTimeout(500);
	expect(collected).toHaveLength(count);
});
