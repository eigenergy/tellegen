import { expect, test } from '@playwright/test';

test('real browser worker solves raw and typed multiconductor PF', async ({ page }) => {
	const pageErrors: string[] = [];
	const workers: string[] = [];
	page.on('pageerror', (error) => pageErrors.push(String(error)));
	page.on('worker', (worker) => workers.push(worker.url()));
	await page.goto('http://127.0.0.1:4174/');
	await expect
		.poll(
			async () => {
				const text = await page.locator('#result').innerText();
				return text.startsWith('{') ? JSON.parse(text) : undefined;
			},
			{ timeout: 60_000 }
		)
		.toMatchObject({ invalid_rejected: true });
	const result = JSON.parse(await page.locator('#result').innerText()) as {
		raw: { converged: boolean; factorization_count: number; terminals: unknown[] };
		typed: { converged: boolean; factorization_count: number; terminals: unknown[] };
		max_oracle_voltage_error: number;
		max_typed_voltage_error: number;
		invalid_rejected: boolean;
		oracle_terminal_count: number;
		raw_terminal_count: number;
		typed_terminal_count: number;
		error?: string;
	};
	expect(result.error).toBeUndefined();
	expect(result.raw.converged).toBe(true);
	expect(result.typed.converged).toBe(true);
	expect(result.raw.factorization_count).toBe(1);
	expect(result.typed.factorization_count).toBe(1);
	expect(result.raw.terminals).toHaveLength(result.typed.terminals.length);
	expect(result.raw_terminal_count).toBe(result.oracle_terminal_count);
	expect(result.typed_terminal_count).toBe(result.oracle_terminal_count);
	expect(result.max_oracle_voltage_error).toBeLessThan(1e-5);
	expect(result.max_typed_voltage_error).toBeLessThan(1e-10);
	expect(result.invalid_rejected).toBe(true);
	expect(workers.some((url) => url.includes('worker'))).toBe(true);
	expect(pageErrors).toEqual([]);
});
