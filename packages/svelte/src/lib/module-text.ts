import {
	parseSynchronousAreas,
	type SynchronousAreaView
} from './synchronous-areas.js';

/** Read saved calculation settings and generated-grid annotations from module text.
 * Large modules are parsed once across settings, bus-kind and area readers;
 * only the most recent text and its display data remain cached. */

/** The PowerIO module extension in which a saved Study records its settings. */
export const RECORDED_SETTINGS_EXTENSION = 'tellegen.calculation_settings';

/** The `extras` key under which tellegen-synth records each element's
 * annotation in the modules it writes. */
export const SYNTH_EXTRAS = 'tellegen_synth';

export const SYNCHRONOUS_AREAS_EXTENSION = 'tellegen.synchronous_areas';

export interface ModuleTextReads {
	synchronousAreas: SynchronousAreaView | null;
	synchronousAreasInvalid: boolean;
	/** `extensions["tellegen.calculation_settings"]` as written, unvalidated. */
	recordedSettings: unknown;
	/** Each bus's tellegen-synth kind by bus id; empty for other modules. */
	busKinds: ReadonlyMap<number, string>;
}

const NOTHING: ModuleTextReads = {
	recordedSettings: undefined,
	synchronousAreas: null,
	synchronousAreasInvalid: false,
	busKinds: new Map()
};
let last: { text: string; reads: ModuleTextReads } | null = null;

/** Read `moduleJson` once. Text that does not parse reads as nothing. */
export function readModuleText(
	moduleJson: string | null | undefined
): ModuleTextReads {
	if (!moduleJson) return NOTHING;
	if (last?.text === moduleJson) return last.reads;
	const reads = parse(moduleJson);
	last = { text: moduleJson, reads };
	return reads;
}

function parse(text: string): ModuleTextReads {
	const hasSettings = text.includes(`"${RECORDED_SETTINGS_EXTENSION}"`);
	const hasKinds = text.includes(`"${SYNTH_EXTRAS}"`);
	const hasAreas = text.includes(`"${SYNCHRONOUS_AREAS_EXTENSION}"`);
	if (!hasSettings && !hasKinds && !hasAreas) return NOTHING;
	let root: unknown;
	try {
		root = JSON.parse(text);
	} catch {
		return NOTHING;
	}
	const record = root as {
		extensions?: Record<string, unknown>;
		value?: { data?: { buses?: unknown } };
	} | null;
	const busKinds = new Map<number, string>();
	const buses = hasKinds ? record?.value?.data?.buses : undefined;
	if (Array.isArray(buses))
		for (const bus of buses) {
			const kind = bus?.extras?.[SYNTH_EXTRAS]?.kind;
			if (typeof bus?.id === 'number' && typeof kind === 'string')
				busKinds.set(bus.id, kind);
		}
	const rawAreas = record?.extensions?.[SYNCHRONOUS_AREAS_EXTENSION];
	const synchronousAreas = parseSynchronousAreas(rawAreas);
	return {
		synchronousAreas,
		synchronousAreasInvalid: rawAreas !== undefined && !synchronousAreas,
		recordedSettings: hasSettings
			? record?.extensions?.[RECORDED_SETTINGS_EXTENSION]
			: undefined,
		busKinds
	};
}
