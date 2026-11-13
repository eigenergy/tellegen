/** The two things the viewer reads from a PowerIO module's text on the page:
 * the calculation settings a saved Study recorded and the bus kinds a
 * generated grid records. A case opens with three reads of the same text
 * (the import log, the case's settings, the map's bus kinds), and a generated
 * 3,000-bus module is about 12.6 M characters, so the text is parsed once and
 * the last answer kept (#261). Only the last text and its small answer are
 * held; a module that names neither key is not parsed at all. */

/** The PowerIO module extension in which a saved Study records its settings. */
export const RECORDED_SETTINGS_EXTENSION = 'tellegen.calculation_settings';

/** The `extras` key under which tellegen-synth records each element's
 * annotation in the modules it writes. */
export const SYNTH_EXTRAS = 'tellegen_synth';

export interface ModuleTextReads {
	/** `extensions["tellegen.calculation_settings"]` as written, unvalidated. */
	recordedSettings: unknown;
	/** Each bus's tellegen-synth kind by bus id; empty for other modules. */
	busKinds: ReadonlyMap<number, string>;
}

const NOTHING: ModuleTextReads = {
	recordedSettings: undefined,
	busKinds: new Map()
};
let last: { text: string; reads: ModuleTextReads } | null = null;

/** Read `moduleJson` once. Text that does not parse reads as nothing. */
export function readModuleText(moduleJson: string | null | undefined): ModuleTextReads {
	if (!moduleJson) return NOTHING;
	if (last?.text === moduleJson) return last.reads;
	const reads = parse(moduleJson);
	last = { text: moduleJson, reads };
	return reads;
}

function parse(text: string): ModuleTextReads {
	const hasSettings = text.includes(`"${RECORDED_SETTINGS_EXTENSION}"`);
	const hasKinds = text.includes(`"${SYNTH_EXTRAS}"`);
	if (!hasSettings && !hasKinds) return NOTHING;
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
			if (typeof bus?.id === 'number' && typeof kind === 'string') busKinds.set(bus.id, kind);
		}
	return {
		recordedSettings: hasSettings ? record?.extensions?.[RECORDED_SETTINGS_EXTENSION] : undefined,
		busKinds
	};
}
