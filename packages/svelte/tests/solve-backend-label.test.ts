import { describe, expect, it } from 'vitest';
import { solveBackendLabel } from '../src/lib/format.js';

describe('solveBackendLabel', () => {
	it('names the browser solvers as WebAssembly', () => {
		expect(solveBackendLabel('clarabel-wasm')).toBe('Clarabel (WebAssembly)');
		expect(solveBackendLabel('pounce-wasi')).toBe('POUNCE (WebAssembly)');
	});

	it('says when a server took part', () => {
		expect(solveBackendLabel('clarabel-wasm-server-sensitivity')).toMatch(/server/);
		expect(solveBackendLabel('rust-server')).toBe('Server');
	});
});
