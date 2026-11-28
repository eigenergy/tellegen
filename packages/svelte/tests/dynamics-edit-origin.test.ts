import { describe, expect, it, vi } from 'vitest';
import type { RmsCaseOptions } from '@tellegen/engine';

// The RMS worker is not available here: a prepared case opens at once, or
// when the test releases it (`hold`), and reports the DYR text it was given.
let hold: Promise<void> | null = null;
vi.mock('@tellegen/engine', async (orig) => ({
	...(await orig<typeof import('@tellegen/engine')>()),
	openRmsCase: vi.fn(async (_module: string, dyr: string, options: RmsCaseOptions) => {
		if (hold) await hold;
		return {
			options,
			dyr,
			inspection: {
				options: { reactive_limits: true },
				plants: [],
				held_units: [],
				branches: [],
				generators: [],
				sources: [],
				notes: [],
				dyr_records: 1,
				base_frequency: 60
			},
			flatCheck: vi.fn(async () => ({
				seconds: 2,
				max_speed_drift: 0,
				max_voltage_drift: 0,
				steps: 1
			})),
			run: vi.fn(async () => ({ time: new Float64Array([0]) })),
			free: vi.fn()
		};
	})
}));

const { AppState } = await import('../src/lib/state.svelte.js');
const { DynamicsController, StaleEditError } =
	await import('../src/lib/dynamics/controller.svelte.js');

function deferred<T>() {
	let resolve!: (v: T) => void;
	const promise = new Promise<T>((r) => (resolve = r));
	return { promise, resolve };
}

/** Two local cases, A and B, each with its own dynamic data; A active. */
async function setup() {
	hold = null;
	const app = new AppState();
	const dyn = new DynamicsController(app, async () => {});
	const a = { id: 'A', label: 'Case A', studyInputJson: 'network-A' };
	const b = { id: 'B', label: 'Case B', studyInputJson: 'network-B' };
	app.localCases = [a, b] as never;
	app.activeLocalId = 'A';
	await dyn.attach(a as never, 'a.dyr', 'dyr-A');
	app.activeLocalId = 'B';
	await dyn.attach(b as never, 'b.dyr', 'dyr-B');
	app.activeLocalId = 'A';
	return { app, dyn, a, b };
}

describe('an edit of the dynamic data stays with the case it started from', () => {
	it('lands on its own case when the user switched to another one meanwhile', async () => {
		const { app, dyn } = await setup();
		const origin = dyn.editOrigin();
		// The worker's reply (editDyr of A's text) is held …
		const reply = deferred<string>();
		const edit = (async () => dyn.editDynamics(origin, await reply.promise))();
		// … while the user switches to B …
		app.activeLocalId = 'B';
		// … and then arrives.
		reply.resolve('dyr-A edited');
		await edit;
		expect(dyn.attachments.A.text).toBe('dyr-A edited');
		expect(dyn.attachments.B.text).toBe('dyr-B');
		expect(dyn.active?.localId).toBe('B');
	});

	it('is refused when its case was attached again meanwhile', async () => {
		const { dyn, a } = await setup();
		const origin = dyn.editOrigin();
		const reply = deferred<string>();
		const edit = (async () => dyn.editDynamics(origin, await reply.promise))();
		// Another edit (or a new file) replaced A's data first.
		await dyn.attach(a as never, 'a.dyr', 'dyr-A newer');
		reply.resolve('dyr-A edited from the old text');
		await expect(edit).rejects.toBeInstanceOf(StaleEditError);
		expect(dyn.attachments.A.text).toBe('dyr-A newer');
	});

	it('is refused when its case changed network or was closed meanwhile', async () => {
		const { app, dyn, b } = await setup();
		const origin = dyn.editOrigin();
		app.localCases = [{ id: 'A', label: 'Case A', studyInputJson: 'network-A2' }, b] as never;
		await expect(dyn.editDynamics(origin, 'x')).rejects.toBeInstanceOf(StaleEditError);
		app.localCases = [b] as never;
		await expect(dyn.editDynamics(origin, 'x')).rejects.toBeInstanceOf(StaleEditError);
		expect(dyn.attachments.A.text).toBe('dyr-A');
	});

	it('of two overlapping edits, applies the first and refuses the one computed from older data', async () => {
		const { dyn } = await setup();
		const first = dyn.editOrigin();
		const second = dyn.editOrigin();
		await dyn.editDynamics(second, 'dyr-A second');
		await expect(dyn.editDynamics(first, 'dyr-A first')).rejects.toBeInstanceOf(StaleEditError);
		expect(dyn.attachments.A.text).toBe('dyr-A second');
	});

	it('an attach that fails late does not overwrite the one after it', async () => {
		const { dyn, a } = await setup();
		const gate = deferred<void>();
		hold = gate.promise.then(() => {
			throw new Error('the older data does not initialize');
		});
		const slow = dyn.attach(a as never, 'a.dyr', 'dyr-A slow');
		// Until the worker has the slow one.
		await new Promise((r) => setTimeout(r, 0));
		hold = null;
		await dyn.attach(a as never, 'a.dyr', 'dyr-A fast');
		gate.resolve();
		await slow;
		expect(dyn.attachments.A.text).toBe('dyr-A fast');
		expect(dyn.attachments.A.status).toBe('ready');
	});
});
