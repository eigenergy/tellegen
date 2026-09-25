import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	createStudy,
	probeAcOpfWorker,
	solveAcOpfModule,
	type SolveResponse
} from '@tellegen/engine';
import { AppState, CaseState, LocalCase } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';
import type { TellegenApiClient } from '../src/lib/api.js';

vi.mock('@tellegen/engine', async (importOriginal) => ({
	...(await importOriginal<typeof import('@tellegen/engine')>()),
	createStudy: vi.fn(),
	probeAcOpfWorker: vi.fn(),
	solveAcOpfModule: vi.fn()
}));

function pending<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function host() {
	const app = new AppState();
	const api = {
		getCaseModuleJson: vi.fn(async () => {
			throw new Error('HTTP 503');
		})
	};
	const ctrl = new Controller(app, {
		api: api as unknown as TellegenApiClient,
		acOpfWasmUrl: '/experimental-acopf/tellegen_acopf_wasi.wasm'
	});
	ctrl.acOpfAvailable = true;
	const c = new LocalCase({
		id: 'local-acopf',
		label: 'AC OPF',
		fileName: 'case.pio.json',
		formulation: 'acopf',
		studyInputJson: 'canonical-instance'
	});
	app.addLocal(c);
	return { app, ctrl, c };
}

const response: SolveResponse = {
	formulation: 'acopf',
	status: 'feasible',
	objective: 42,
	vm: [{ bus: 1, value: 1.01 }]
};

beforeEach(() => vi.resetAllMocks());

describe('AC OPF controller boundary', () => {
	it.each(['acopf', 'acpf', 'socwr'] as const)(
		'never sends %s to the DC server when module loading fails',
		async (formulation) => {
			const { app, ctrl } = host();
			const c = new CaseState({
				id: 'backend',
				name: 'Backend',
				n_bus: 3,
				n_branch: 3,
				n_gen: 2
			});
			c.formulation = formulation;
			app.cases = [c];
			const server = vi.spyOn(ctrl, 'serverSolve').mockImplementation(() => {});
			ctrl.runSolve(c, null);
			await vi.waitFor(() => expect(c.solving).toBe(false));
			expect(server).not.toHaveBeenCalled();
			expect(solveAcOpfModule).not.toHaveBeenCalled();
			expect(app.error).toContain('requires its PowerIO module');
			expect(app.error).toContain('HTTP 503');
		}
	);

	it('preserves the DC server fallback', async () => {
		const { ctrl } = host();
		const c = new CaseState({
			id: 'backend',
			name: 'Backend',
			n_bus: 3,
			n_branch: 3,
			n_gen: 2
		});
		const server = vi.spyOn(ctrl, 'serverSolve').mockImplementation(() => {});
		ctrl.runSolve(c, null);
		await vi.waitFor(() => expect(server).toHaveBeenCalledOnce());
	});

	it('passes the retained canonical module to the worker without building a Study', async () => {
		const { ctrl, c } = host();
		vi.mocked(solveAcOpfModule).mockResolvedValue(response);
		ctrl.runSolve(c, { bus: 1 });
		await vi.waitFor(() => expect(c.solving).toBe(false));
		expect(solveAcOpfModule).toHaveBeenCalledWith(
			'/experimental-acopf/tellegen_acopf_wasi.wasm',
			'canonical-instance',
			expect.any(AbortSignal)
		);
		expect(createStudy).not.toHaveBeenCalled();
		expect(c.solution?.objective).toBe(42);
		expect(c.solveBackend).toBe('pounce-wasi');
	});

	it('aborts a superseded solve and ignores its late result', async () => {
		const { ctrl, c } = host();
		const first = pending<SolveResponse>();
		vi.mocked(solveAcOpfModule).mockReturnValueOnce(first.promise).mockResolvedValueOnce(response);
		ctrl.runSolve(c, null);
		await vi.waitFor(() => expect(solveAcOpfModule).toHaveBeenCalledOnce());
		const signal = vi.mocked(solveAcOpfModule).mock.calls[0][2]!;
		ctrl.runSolve(c, null);
		expect(signal.aborted).toBe(true);
		await vi.waitFor(() => expect(c.solution?.objective).toBe(42));
		first.resolve({ ...response, objective: 99 });
		await first.promise;
		expect(c.solution?.objective).toBe(42);
	});

	it('aborts a removed background case and ignores its late result', async () => {
		const { app, ctrl, c } = host();
		const solve = pending<SolveResponse>();
		vi.mocked(solveAcOpfModule).mockReturnValue(solve.promise);
		ctrl.runSolve(c, null);
		await vi.waitFor(() => expect(solveAcOpfModule).toHaveBeenCalledOnce());
		app.activeLocalId = null;
		await ctrl.removeLocalCase(c);
		expect(vi.mocked(solveAcOpfModule).mock.calls[0][2]!.aborted).toBe(true);
		solve.resolve(response);
		await solve.promise;
		expect(c.solution).toBeNull();
	});

	it.each(['unavailable', 'edited'] as const)(
		'fails closed when the case is %s',
		async (reason) => {
			const { app, ctrl, c } = host();
			if (reason === 'unavailable') ctrl.acOpfAvailable = false;
			else c.deltas = { 1: 5 };
			ctrl.runSolve(c, null);
			await vi.waitFor(() => expect(c.solving).toBe(false));
			expect(solveAcOpfModule).not.toHaveBeenCalled();
			expect(app.error).toContain(
				reason === 'unavailable' ? 'worker is unavailable' : 'base case only'
			);
		}
	);

	it('starts a declared instance dropped before the asset probe completed', async () => {
		const { ctrl, c } = host();
		const probe = pending<boolean>();
		ctrl.acOpfAvailable = false;
		vi.mocked(probeAcOpfWorker).mockReturnValue(probe.promise);
		const start = vi.spyOn(ctrl, 'maybeStartLocalSolve').mockImplementation(() => {});
		ctrl.probeAcOpf();
		expect(start).not.toHaveBeenCalled();
		probe.resolve(true);
		await probe.promise;
		expect(ctrl.acOpfAvailable).toBe(true);
		expect(start).toHaveBeenCalledWith(c.id);
	});

	it('rejects save/export before attempting to create an unsupported Study', async () => {
		const { app, ctrl, c } = host();
		expect(ctrl.caseExportUnavailableReason(c)).toContain('not supported yet');
		await ctrl.saveCaseModule(c);
		expect(app.error).toContain('Saving and exporting AC OPF');
		await expect(ctrl.exportCaseAs(c, 'matpower')).resolves.toEqual([]);
		expect(createStudy).not.toHaveBeenCalled();
		c.formulation = 'dcopf';
		expect(ctrl.caseExportUnavailableReason(c)).toBeNull();
	});
});
