import { expect, test } from '@playwright/test';

test('real browser worker solves raw and typed multiconductor PF', async ({ page }) => {
	const pageErrors: string[] = [];
	const workers: string[] = [];
	page.on('pageerror', (error) => pageErrors.push(String(error)));
	page.on('worker', (worker) => workers.push(worker.url()));
	const port = Number(
		process.env.TELLEGEN_MC_PREVIEW_PORT ?? Number(process.env.TELLEGEN_PREVIEW_PORT ?? 4173) + 1
	);
	await page.goto(`http://127.0.0.1:${port}/`);
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
		default_settings: {
			converged: boolean;
			actual: Record<string, number | boolean> | null;
			expected: Record<string, number | boolean>;
		};
		raw: { converged: boolean; factorization_count: number; terminals: unknown[] };
		typed: { converged: boolean; factorization_count: number; terminals: unknown[] };
		max_oracle_voltage_error: number;
		max_typed_voltage_error: number;
		invalid_rejected: boolean;
		oracle_terminal_count: number;
		raw_terminal_count: number;
		typed_terminal_count: number;
		tables_mismatch: string | null;
		tables_port_count: number;
		error?: string;
	};
	expect(result.error).toBeUndefined();
	expect(result.default_settings.converged).toBe(true);
	expect(result.default_settings.actual).toEqual(result.default_settings.expected);
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
	// A live session's result columns, moved as typed arrays, equal its JSON result exactly.
	expect(result.tables_mismatch).toBeNull();
	expect(result.tables_port_count).toBeGreaterThan(0);
	expect(workers.some((url) => url.includes('worker'))).toBe(true);
	expect(pageErrors).toEqual([]);
});
