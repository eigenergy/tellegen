import type { ResolvedSettings } from '@tellegen/engine';

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
			coloring: this.coloring
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
							: false;
			if (!valid) throw new Error(`Invalid display preference: ${key}`);
		}
		if ('theme' in patch) this.theme = patch.theme as typeof this.theme;
		if ('busSizing' in patch) this.busSizing = patch.busSizing as typeof this.busSizing;
		if ('coloring' in patch) this.coloring = patch.coloring as typeof this.coloring;
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
		this.update({ theme: null, busSizing: 'power', coloring: 'price' });
	}
}

/** Read provenance only after the containing PowerIO input has been validated. */
export function recordedCalculationSettings(
	input?: string
): { calculation: string; effective: Record<string, unknown> } | null {
	if (!input) return null;
	try {
		const value = JSON.parse(input)?.extensions?.['tellegen.calculation-settings.v1'];
		if (
			value &&
			typeof value.calculation === 'string' &&
			value.effective &&
			typeof value.effective === 'object' &&
			!Array.isArray(value.effective)
		)
			return value;
	} catch {
		/* Input validation remains the engine's responsibility. */
	}
	return null;
}
