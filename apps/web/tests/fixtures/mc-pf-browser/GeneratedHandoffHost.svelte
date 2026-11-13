<script lang="ts">
	import { onMount } from 'svelte';
	import { createSynthSession, type IngestedCase, type SynthRequest } from '@tellegen/engine';
	import proxy from '../../../../../packages/engine/tests/fixtures/handoff-request.json';
	import china from '../../../../../packages/engine/tests/fixtures/handoff-china-request.json';
	import { getController } from '../../../../../packages/svelte/src/lib/context.svelte.js';
	import type { LocalCase } from '../../../../../packages/svelte/src/lib/state.svelte.js';

	const ctrl = getController();
	let payload: IngestedCase | null = null;
	let current: LocalCase | null = null;
	let samples = 0;

	const frame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));
	/** The first animation frame at which `ready()` holds, checked every frame. */
	async function until(ready: () => boolean, label: string) {
		const deadline = performance.now() + 5 * 60_000;
		while (!ready()) {
			if (performance.now() > deadline) throw new Error(`timed out waiting for ${label}`);
			await frame();
		}
		return performance.now();
	}
	const cardShowsTime = () =>
		[...document.querySelectorAll('.solvecard dt')].some((dt) => dt.textContent === 'Time');

	onMount(() => {
		window.generatedHandoff = {
			async generate(buses) {
				const started = performance.now();
				// No bus count: MapToGrid's request for China at its default density.
				const request = buses === null ? china : { ...proxy, target_bus_count: buses };
				const session = await createSynthSession(request as SynthRequest);
				try {
					await session.run();
					const generated = performance.now();
					payload = await session.ingest();
					return {
						generate_ms: generated - started,
						ingest_ms: performance.now() - generated,
						buses: payload.n_bus,
						module_json_chars: payload.module_json.length
					};
				} finally {
					session.free();
				}
			},
			async open() {
				if (!payload) throw new Error('generate a grid first');
				// Each sample opens on an empty viewer, as MapToGrid's first grid does.
				if (current) await ctrl.removeLocalCase(current);
				await until(() => !cardShowsTime(), 'the previous solve card to close');
				// New text for every sample, as each new grid is: trailing whitespace
				// keeps the module valid and defeats any cache keyed on the text.
				const fresh = { ...payload, module_json: payload.module_json + ' '.repeat(++samples) };
				const start = performance.now();
				const local = await ctrl.openBalancedModule({
					name: 'generated',
					payload: fresh,
					formulation: 'dcopf',
					frame: false
				});
				current = local;
				const opened = performance.now();
				const solved = await until(
					() =>
						local.solution != null ||
						(!local.solving && local.solveMs == null && local.solveBackend !== null),
					'the first solve'
				);
				const card = await until(cardShowsTime, 'the solve card');
				return {
					open_ms: opened - start,
					solution_ms: solved - start,
					card_ms: card - start,
					formulation: local.formulation,
					verdict: document.querySelector('.solvecard .verdict')?.textContent?.trim() ?? ''
				};
			}
		};
		document.body.dataset.ready = 'true';
	});
</script>
