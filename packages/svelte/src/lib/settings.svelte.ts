import { settingsCatalog, type ResolvedSettings } from '@tellegen/engine';

/** Selected next-run settings and accepted-result provenance have separate lives. */
export class CaseSettings {
	revision = $state(0);
	calculation = $state<string | null>(null);
	baselines = $state<Record<string, Record<string, unknown>>>({});
	options = $state<Record<string, Record<string, unknown>>>({});
	/** Rust-normalized selections, including defaults inside nested options. */
	resolved = $state<Record<string, ResolvedSettings>>({});
	accepted = $state.raw<ResolvedSettings | null>(null);
}

export class UserPreferences {
	theme = $state<'light' | 'dark' | null>(null);
	busSizing = $state<'power' | 'uniform'>('power');
	coloring = $state<'price' | 'angle' | 'voltage' | 'demand'>('price');
	/** Zen mode hides the panels; it survives a reload so a presenter keeps the view. */
	zen = $state(false);
	error = $state<string | null>(null);
	revision = $state(0);
	#storage: Storage | undefined;
	initialize(storage?: Storage) {
		this.#storage = storage;
		try {
			const text = storage?.getItem('tellegen.preferences.v1');
			if (text) {
				const saved = JSON.parse(text);
				if (saved.version !== 1) throw new Error('Unsupported preferences version');
				this.update(saved.values, false);
			}
		} catch {
			this.error = 'Display preferences could not be restored. Defaults are in use.';
		}
	}
	inspect() {
		return {
			theme: this.theme,
			busSizing: this.busSizing,
			coloring: this.coloring,
			zen: this.zen
		};
	}
	update(patch: Record<string, unknown>, save = true) {
		for (const [key, value] of Object.entries(patch)) {
			const valid =
				key === 'theme'
					? value === null || value === 'light' || value === 'dark'
					: key === 'busSizing'
						? value === 'power' || value === 'uniform'
						: key === 'coloring'
							? ['price', 'angle', 'voltage', 'demand'].includes(value as string)
							: key === 'zen'
								? typeof value === 'boolean'
								: false;
			if (!valid) throw new Error(`Invalid display preference: ${key}`);
		}
		if ('theme' in patch) this.theme = patch.theme as typeof this.theme;
		if ('busSizing' in patch) this.busSizing = patch.busSizing as typeof this.busSizing;
		if ('coloring' in patch) this.coloring = patch.coloring as typeof this.coloring;
		if ('zen' in patch) this.zen = patch.zen as boolean;
		this.revision++;
		if (!save || !this.#storage) return;
		try {
			this.#storage.setItem(
				'tellegen.preferences.v1',
				JSON.stringify({ version: 1, values: this.inspect() })
			);
			this.error = null;
		} catch {
			this.error = 'Display preferences could not be saved. Changes last for this visit.';
		}
	}
	reset() {
		this.update({
			theme: null,
			busSizing: 'power',
			coloring: 'price',
			zen: false
		});
	}
}

/** The PowerIO module extension in which a saved Study records its settings. */
export const RECORDED_SETTINGS_EXTENSION = 'tellegen.calculation_settings';

/** Calculation settings a saved module records, read for this engine. */
export interface RecordedCalculationSettings {
	calculation: string;
	/** Recorded options this engine knows; the record keeps only non-defaults. */
	overrides: Record<string, unknown>;
	/** This engine's defaults with the recorded overrides applied. */
	effective: Record<string, unknown>;
	/** Recorded option keys this engine does not know. They are dropped. */
	ignored: string[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return !!value && typeof value === 'object' && !Array.isArray(value);
}

/** Read provenance only after the containing PowerIO input has been validated.
 * A record from another engine version still opens: options this engine does not
 * know are dropped and listed in `ignored`, as the Rust Study does. */
export function recordedCalculationSettings(input?: string): RecordedCalculationSettings | null {
	if (!input) return null;
	let value: unknown;
	try {
		value = JSON.parse(input)?.extensions?.[RECORDED_SETTINGS_EXTENSION];
	} catch {
		/* Input validation remains the engine's responsibility. */
		return null;
	}
	if (
		!isRecord(value) ||
		value.version !== 1 ||
		typeof value.calculation !== 'string' ||
		(value.overrides !== undefined && !isRecord(value.overrides))
	)
		return null;
	const calculation = value.calculation;
	const descriptor = settingsCatalog.calculations.find((c) => c.calculation === calculation);
	const defaults: Record<string, unknown> =
		descriptor && 'defaults' in descriptor ? { ...descriptor.defaults } : {};
	const overrides: Record<string, unknown> = {};
	const ignored: string[] = [];
	for (const [key, option] of Object.entries(value.overrides ?? {})) {
		if (Object.hasOwn(defaults, key)) overrides[key] = option;
		else ignored.push(key);
	}
	return {
		calculation,
		overrides,
		effective: { ...defaults, ...overrides },
		ignored
	};
}
