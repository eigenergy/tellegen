/** The viewer half of the generated-grid hand-off benchmark (#261).
 *
 * Mounts tellegen's own shell (map, panels, solve card) and opens a generated
 * grid the way MapToGrid's `ViewerHandoff.open` does: the `IngestedCase` of
 * the session goes straight to `Controller.openBalancedModule` with
 * `formulation: 'dcopf'` and `frame: false`, so the page never runs
 * `ingest_json_drop` and the first solve is the DC OPF. */

import { mount } from 'svelte';
import '../../../../../packages/svelte/src/lib/styles.css';
import GeneratedHandoff from './GeneratedHandoff.svelte';

mount(GeneratedHandoff, { target: document.getElementById('app')! });

export interface GeneratedGrid {
	/** SynthSession run to its end, on the synthetic worker. */
	generate_ms: number;
}

/** One hand-off. Times are milliseconds since the Unix epoch
 * (`performance.timeOrigin + performance.now()`), the clock the engine
 * workers report on. */
export interface HandoffTiming {
	/** The grid is ready on the synthetic worker: the hand-off starts. */
	ready: number;
	/** The payload is parsed on the page and `openBalancedModule` is called. */
	start: number;
	/** `openBalancedModule` resolved: the case is built and active. */
	opened: number;
	/** The DC OPF answer is in the case state. */
	solved: number;
	/** The first animation frame whose DOM shows the solve card's time. */
	card: number;
	buses: number;
	module_json_chars: number;
	formulation: string;
	/** The calculation the solve card names. */
	calculation: string;
	status: string;
}

declare global {
	interface Window {
		generatedHandoff: {
			/** Generate a grid of `buses` buses over the eastern China proxy, or
			 * with `null` China at MapToGrid's default density, and keep its
			 * session for `open`. */
			generate(buses: number | null): Promise<GeneratedGrid>;
			/** Hand the generated grid to the viewer: `serial` as before #261
			 * (the payload, then the viewer builds its Study), `ahead` with the
			 * Study started before the payload is prepared. */
			open(path: 'serial' | 'ahead'): Promise<HandoffTiming>;
		};
	}
}
