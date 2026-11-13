/** The viewer half of the generated-grid hand-off benchmark (#261).
 *
 * Mounts tellegen's own shell (map, panels, solve card) and opens a generated
 * grid the way MapToGrid's `ViewerHandoff.open` does: the `IngestedCase` from
 * `SynthSession.ingest()` goes straight to `Controller.openBalancedModule`
 * with `formulation: 'dcopf'` and `frame: false`, so the page never runs
 * `ingest_json_drop` and the first solve is the DC OPF. */

import { mount } from 'svelte';
import '../../../../../packages/svelte/src/lib/styles.css';
import GeneratedHandoff from './GeneratedHandoff.svelte';

mount(GeneratedHandoff, { target: document.getElementById('app')! });

export interface GeneratedGrid {
	/** SynthSession run to its end, on the synthetic worker. */
	generate_ms: number;
	/** `ingest()`: the payload serialized, copied and parsed on this page. */
	ingest_ms: number;
	buses: number;
	module_json_chars: number;
}

export interface HandoffTiming {
	/** `openBalancedModule` until it resolves: the case built and activated. */
	open_ms: number;
	/** Until the DC OPF answer is in the case state. */
	solution_ms: number;
	/** Until the first animation frame whose DOM shows the solve card's time. */
	card_ms: number;
	formulation: string;
	verdict: string;
}

declare global {
	interface Window {
		generatedHandoff: {
			/** Generate and ingest a grid of `buses` buses over the eastern China
			 * proxy, or with `null` China at MapToGrid's default density; held
			 * for `open`. */
			generate(buses: number | null): Promise<GeneratedGrid>;
			/** Open the held grid in the viewer, replacing any earlier one. */
			open(): Promise<HandoffTiming>;
		};
	}
}
