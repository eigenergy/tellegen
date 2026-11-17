/** Browser benchmark for the generated-grid hand-off to the viewer (#261).
 *
 * `synth-handoff-bench.spec.ts` calls `run` once per sample. Each sample uses
 * the public engine the way the viewer does: a staged generation on the
 * synthetic worker, the payload to the page, a Study on the shared study
 * worker, then the first DC OPF answer. It returns when each step
 * finished on the page; the spec joins those with the times the workers
 * report for every request to split each step into build, copy and parse. */

import { createStudy, createSynthSession, type SynthRequest } from '@tellegen/engine';
import base from '../../../../../packages/engine/tests/fixtures/handoff-request.json';

/** Milliseconds since the Unix epoch, the clock the workers report on. */
const now = () => performance.timeOrigin + performance.now();

/** `serial` is the hand-off before #261: the payload, then a Study built from
 * its module text. `ahead` starts the Study before the payload is prepared. */
async function run(buses: number, path: 'serial' | 'ahead') {
	const at: Record<string, number> = {};
	const session = await createSynthSession({ ...base, target_bus_count: buses } as SynthRequest);
	let payload;
	let study;
	try {
		await session.run();
		at.generated = now();
		if (path === 'ahead') {
			const handoff = await session.ingestWithStudy('dcopf');
			at.ingested = now();
			payload = handoff.payload;
			study = await handoff.study;
		} else {
			payload = await session.ingest();
			at.ingested = now();
			study = await createStudy(payload.module_json, 'dcopf');
		}
	} finally {
		session.free();
	}
	try {
		const solution = await study.currentSolution();
		at.solved = now();
		if (solution.objective == null) throw new Error('the first DC OPF has no objective');
	} finally {
		study.free();
	}
	return { at, buses: payload.n_bus, module_json_chars: payload.module_json.length };
}

(window as unknown as { synthHandoff: unknown }).synthHandoff = { run };
document.getElementById('status')!.textContent = 'ready';
