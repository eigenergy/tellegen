<script lang="ts">
	import { onMount } from 'svelte';
	import {
		createSynthSession,
		type BrowserSynthSession,
		type SynthRequest
	} from '@tellegen/engine';
	import proxy from '../../../../../packages/engine/tests/fixtures/handoff-request.json';
	import china from '../../../../../packages/engine/tests/fixtures/handoff-china-request.json';
	import { getController } from '../../../../../packages/svelte/src/lib/context.svelte.js';

	const ctrl = getController();
	let session: BrowserSynthSession | null = null;

	/** Milliseconds since the Unix epoch, the clock the workers report on. */
	const now = () => performance.timeOrigin + performance.now();
	const frame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));
	/** The first animation frame at which `ready()` holds, checked every frame. */
	async function until(ready: () => boolean, label: string) {
		const deadline = performance.now() + 5 * 60_000;
		while (!ready()) {
			if (performance.now() > deadline) throw new Error(`timed out waiting for ${label}`);
			await frame();
		}
		return now();
	}
	const cardShowsTime = () =>
		[...document.querySelectorAll('.solvecard dt')].some((dt) => dt.textContent === 'Time');

	onMount(() => {
		window.generatedHandoff = {
			async generate(buses) {
				const started = performance.now();
				// No bus count: MapToGrid's request for China at its default density.
				const request = buses === null ? china : { ...proxy, target_bus_count: buses };
				session = await createSynthSession(request as SynthRequest);
				await session.run();
				return { generate_ms: performance.now() - started };
			},
			async open(path) {
				if (!session) throw new Error('generate a grid first');
				const ready = now();
				// `serial` is the hand-off before #261; `ahead` starts the Study
				// before the payload is prepared and gives it to the viewer.
				const { payload, study } =
					path === 'ahead'
						? await session.ingestWithStudy('dcopf')
						: { payload: await session.ingest(), study: undefined };
				session.free();
				session = null;
				const start = now();
				const local = await ctrl.openBalancedModule({
					name: 'generated',
					payload,
					study,
					formulation: 'dcopf',
					frame: false
				});
				const opened = now();
				const solved = await until(
					() =>
						local.solution != null ||
						(!local.solving && local.solveMs == null && local.solveBackend !== null),
					'the first solve'
				);
				if (!local.solution) throw new Error('the first solve failed');
				const card = await until(cardShowsTime, 'the solve card');
				return {
					ready,
					start,
					opened,
					solved,
					card,
					buses: payload.n_bus,
					module_json_chars: payload.module_json.length,
					formulation: local.formulation,
					calculation: document.querySelector<HTMLElement>('.solvecard')?.dataset.calculation ?? '',
					status: local.solution?.status ?? ''
				};
			}
		};
		document.body.dataset.ready = 'true';
	});
</script>
