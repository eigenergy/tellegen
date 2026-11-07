import { beforeEach, expect, it, vi } from 'vitest';
vi.mock('@tellegen/engine', async (importOriginal) => ({
	...(await importOriginal<typeof import('@tellegen/engine')>()),
	resolveSettings: vi.fn(),
	createStudy: vi.fn()
}));
import {
	resolveSettings,
	createStudy,
	type ResolvedSettings,
	type Solution
} from '@tellegen/engine';
import { Controller } from '../src/lib/controller.svelte.js';
import { AppState, CaseState, LocalCase } from '../src/lib/state.svelte.js';
const resolved: ResolvedSettings = {
	version: 1,
	calculation: 'acpf',
	engine_defaults: { start: 'flat' },
	baseline: { start: 'flat' },
	overrides: { start: 'case' },
	effective: { start: 'case' }
};
function setup() {
	const app = new AppState();
	const ctrl = new Controller(app, { apiBase: null, loadDefaultCases: false });
	const c = new LocalCase({
		id: 'case',
		label: 'Case',
		fileName: 'case.pio.json',
		studyInputJson: 'input',
		coordsKind: 'file'
	});
	return { ctrl, c };
}
beforeEach(() => vi.resetAllMocks());
it('an input change during asynchronous settings validation prevents a stale patch from committing', async () => {
	const { ctrl, c } = setup();
	let finish!: (value: ResolvedSettings) => void;
	vi.mocked(resolveSettings).mockReturnValue(new Promise((resolve) => (finish = resolve)));
	const pending = ctrl.updateSettings(c, 'acpf', { start: 'case' }, 0);
	c.revisionGeneration++;
	finish(resolved);
	await expect(pending).rejects.toThrow('Case or settings changed');
	expect(c.settings.revision).toBe(0);
	expect(c.settings.calculation).toBeNull();
	expect(c.formulation).toBe('dcopf');
	ctrl.dispose();
});
it('a failed candidate calculation retains the accepted result and its provenance', async () => {
	const { ctrl, c } = setup();
	const previous = { objective: 123 } as unknown as Solution;
	const accepted: ResolvedSettings = {
		version: 1,
		calculation: 'dcopf',
		engine_defaults: {},
		baseline: {},
		overrides: {},
		effective: {}
	};
	c.solution = previous;
	c.settings.accepted = accepted;
	c.settings.calculation = 'acpf';
	c.settings.options = { acpf: { start: 'case' } };
	vi.mocked(resolveSettings).mockResolvedValue(resolved);
	vi.mocked(createStudy).mockRejectedValue(new Error('candidate did not converge'));
	await expect(ctrl.runSettings(c)).rejects.toThrow('candidate did not converge');
	expect(c.solution).toBe(previous);
	expect(c.settings.accepted).toBe(accepted);
	expect(c.formulation).toBe('dcopf');
	expect(c.solving).toBe(false);
	ctrl.dispose();
});

it('a legacy server fallback never silently replaces custom calculation settings with defaults', () => {
	const { ctrl } = setup();
	const c = new CaseState({
		id: 'server',
		name: 'Server'
	} as ConstructorParameters<typeof CaseState>[0]);
	const previous = { objective: 123 } as unknown as Solution;
	c.solution = previous;
	c.solving = true;
	c.settings.accepted = {
		version: 1,
		calculation: 'dcopf',
		engine_defaults: { angle_difference: true },
		baseline: { angle_difference: true },
		overrides: { angle_difference: false },
		effective: { angle_difference: false }
	};
	ctrl.serverSolve(c, null);
	expect(c.solution).toBe(previous);
	expect(c.solving).toBe(false);
	expect(ctrl.app.error).toContain('cannot apply these calculation settings');
	ctrl.dispose();
});

it('a saved balanced network restores its recorded calculation as the initial selection', () => {
	const c = new LocalCase({
		id: 'saved',
		label: 'Saved',
		fileName: 'saved.pio.json',
		coordsKind: 'file',
		studyInputJson: JSON.stringify({
			extensions: {
				'tellegen.calculation_settings': {
					version: 1,
					calculation: 'acpf',
					overrides: { start: 'case' }
				}
			}
		})
	});
	expect(c.formulation).toBe('acpf');
	expect(c.settings.calculation).toBe('acpf');
	expect(c.settings.baselines.acpf).toEqual({ start: 'case' });
});

it('cancelling a candidate Study discards its result and frees the candidate', async () => {
	const { ctrl, c } = setup();
	const previous = { objective: 123 } as unknown as Solution;
	c.solution = previous;
	c.settings.calculation = 'acpf';
	c.settings.options = { acpf: { start: 'case' } };
	vi.mocked(resolveSettings).mockResolvedValue(resolved);
	let finish!: (value: unknown) => void;
	const commit = vi.fn(() => new Promise((resolve) => (finish = resolve)));
	const free = vi.fn();
	vi.mocked(createStudy).mockResolvedValue({
		currentSolution: async () => previous,
		commit,
		free
	} as unknown as Awaited<ReturnType<typeof createStudy>>);
	const pending = ctrl.runSettings(c);
	const rejected = expect(pending).rejects.toThrow();
	await vi.waitFor(() => expect(commit).toHaveBeenCalled());
	expect(ctrl.canCancelSettings(c)).toBe(true);
	ctrl.cancelSettings(c);
	finish({ solution: { objective: 999 }, iterations: [] });
	await rejected;
	expect(c.solution).toBe(previous);
	expect(c.formulation).toBe('dcopf');
	expect(c.solving).toBe(false);
	expect(free).toHaveBeenCalledOnce();
	ctrl.dispose();
});

it('a cleared field removes its override instead of sending an undefined setting', async () => {
	const { ctrl, c } = setup();
	c.settings.options = { acpf: { start: 'case' } };
	vi.mocked(resolveSettings).mockResolvedValue({
		...resolved,
		overrides: {},
		effective: {}
	});
	await ctrl.updateSettings(c, 'acpf', { start: undefined }, 0);
	expect(vi.mocked(resolveSettings)).toHaveBeenCalledWith({
		calculation: 'acpf',
		baseline: {},
		overrides: {}
	});
	ctrl.dispose();
});

it('a successful partial lazy-settings run is no longer pending', async () => {
	const { ctrl, c } = setup();
	const normalized: ResolvedSettings = {
		version: 1,
		calculation: 'dcopf',
		engine_defaults: { angle_difference: true, thermal: 'all', lazy: null },
		baseline: { angle_difference: true, thermal: 'all', lazy: null },
		overrides: { lazy: { near_binding: 0.9 } },
		effective: {
			angle_difference: true,
			thermal: 'all',
			lazy: { near_binding: 0.9, max_rounds: 20 }
		}
	};
	vi.mocked(resolveSettings).mockResolvedValue(normalized);
	await ctrl.updateSettings(c, 'dcopf', { lazy: { near_binding: 0.9 } }, 0);
	c.solveDetail = '42 NLP iterations';
	const solution = { objective: 123 } as unknown as Solution;
	vi.mocked(createStudy).mockResolvedValue({
		currentSolution: async () => solution,
		commit: async () => ({ solution, iterations: [] }),
		free: vi.fn()
	} as unknown as Awaited<ReturnType<typeof createStudy>>);
	await ctrl.runSettings(c);
	expect(ctrl.inspectSettings(c).selected).toEqual(normalized.effective);
	expect(ctrl.inspectSettings(c).pending).toBe(false);
	expect(c.solveDetail).toBeNull();
	ctrl.dispose();
});

it('patching a reopened distribution Study preserves restored model settings', async () => {
	const { MulticonductorCase } = await import('../src/lib/state.svelte.js');
	const { DEFAULT_MC_PF_OPTIONS } = await import('@tellegen/engine');
	const { ctrl } = setup();
	const c = new MulticonductorCase({
		id: 'restored',
		label: 'Restored',
		fileName: 'saved.pio.json',
		summary: {
			module_json: 'input',
			mc_pf_enabled: true,
			mc_pf_supported: true
		},
		graph: { buses: [], edges: [] },
		coordsKind: 'planar'
	} as ConstructorParameters<typeof MulticonductorCase>[0]);
	// StudyWorkspace.#showMulti restores settings this way after ingestion.
	c.mcOptions = {
		...DEFAULT_MC_PF_OPTIONS,
		v_min_pu: 1.01,
		max_iterations: 200
	};
	c.settings.baselines = { mc_ac_pf: { ...c.mcOptions } };
	vi.mocked(resolveSettings).mockImplementation(
		async (request) =>
			({
				version: 1,
				calculation: request.calculation,
				engine_defaults: { ...DEFAULT_MC_PF_OPTIONS },
				baseline: { ...DEFAULT_MC_PF_OPTIONS, ...request.baseline },
				overrides: request.overrides ?? {},
				effective: {
					...DEFAULT_MC_PF_OPTIONS,
					...request.baseline,
					...request.overrides
				}
			}) as ResolvedSettings
	);
	await ctrl.updateSettings(c, 'mc_ac_pf', { damping: 0.8 }, 0);
	expect(c.mcOptions.v_min_pu).toBe(1.01);
	expect(c.mcOptions.max_iterations).toBe(200);
	await ctrl.updateSettings(c, 'mc_ac_pf', {}, c.settings.revision, true);
	expect(c.mcOptions).toEqual({
		...DEFAULT_MC_PF_OPTIONS,
		v_min_pu: 1.01,
		max_iterations: 200
	});
	ctrl.dispose();
});
