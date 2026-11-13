/** Browser benchmark for the generated-grid hand-off to the viewer (#261).
 *
 * `synth-handoff-bench.spec.ts` calls `run` once per sample. Each sample uses
 * the public engine the way the viewer does: a staged generation on the
 * synthetic worker, `ingest()` to the page, `createStudy()` on the shared
 * study worker, then the first DC OPF answer. The finer split of `ingest` and
 * `study build` (serialize, copy, parse) is in the Node twin,
 * `packages/engine/scripts/bench-synth-handoff.mjs`. */

import { createStudy, createSynthSession, type SynthRequest } from '@tellegen/engine';
import base from '../../../../../packages/engine/tests/fixtures/handoff-request.json';

async function run(buses: number) {
	const ms: Record<string, number> = {};
	let mark = performance.now();
	const lap = (name: string) => {
		const now = performance.now();
		ms[name] = now - mark;
		mark = now;
	};
	const session = await createSynthSession({ ...base, target_bus_count: buses } as SynthRequest);
	let payload;
	try {
		await session.run();
		lap('generate');
		payload = await session.ingest();
		lap('ingest to page');
	} finally {
		session.free();
	}
	const study = await createStudy(payload.module_json, 'dcopf');
	try {
		lap('study build');
		await study.currentSolution();
		lap('first solution');
	} finally {
		study.free();
	}
	ms['hand-off'] = ms['ingest to page'] + ms['study build'] + ms['first solution'];
	return { ms, buses: payload.n_bus, module_json_chars: payload.module_json.length };
}

(window as unknown as { synthHandoff: unknown }).synthHandoff = { run };
document.getElementById('status')!.textContent = 'ready';
