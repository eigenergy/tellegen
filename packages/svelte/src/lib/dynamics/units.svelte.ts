/** The generating units of the active case for the unit editor (experimental):
 * every unit with its DYR records as named parameters (static units too),
 * the model library and the published units; and the map glyphs that open a
 * unit's editor. The data is the engine's (`dyrParameters`).
 * @block Units for the editor */

import {
	dyrParameters,
	rmsLibrary,
	unitPresets,
	type DyrGeneratorView,
	type LibraryModel,
	type NetworkBus,
	type UnitPreset
} from '@tellegen/engine';
import type { Layer } from '@deck.gl/core';
import type { TextLayer as TextLayerT } from '@deck.gl/layers';

export type UnitKind = 'machine' | 'inverter' | 'static';

export const unitKey = (u: { bus: number; machine_id: string }) => `${u.bus}|${u.machine_id.trim()}`;

export function unitKind(u: DyrGeneratorView): UnitKind {
	if (u.records.some((r) => r.role === 'machine')) return 'machine';
	return u.inverter.length ? 'inverter' : 'static';
}

export function unitSummary(u: DyrGeneratorView): string {
	if (u.inverter.length) return `inverter · ${u.inverter.join(' ')}`;
	if (!u.records.length) return 'static (power flow only)';
	return u.records.map((r) => r.model).join(' · ');
}

export class UnitCatalog {
	units: DyrGeneratorView[] = $state.raw([]);
	library: LibraryModel[] = $state.raw([]);
	presets: UnitPreset[] = $state.raw([]);
	error = $state<string | null>(null);
	#input: { id: string; moduleJson: string; dyrText: string } | null = null;
	#static = false;

	/** The local case whose units these are (`null`: none loaded). */
	get caseId(): string | null {
		return this.#input?.id ?? null;
	}

	/** Load the units of `moduleJson` with `dyrText` (once per input). */
	sync(id: string | null, moduleJson: string | null | undefined, dyrText: string | null): void {
		if (!id || !moduleJson || dyrText === null) {
			if (this.#input) {
				this.#input = null;
				this.error = null;
				this.units = [];
			}
			return;
		}
		const previous = this.#input;
		if (id === previous?.id && moduleJson === previous.moduleJson && dyrText === previous.dyrText) return;
		const input = { id, moduleJson, dyrText };
		this.#input = input;
		// An edit of the same case keeps its units on screen until the new
		// ones arrive: the list and the open editor stay, and so does the
		// keyboard focus in them. Another case starts empty.
		if (id !== previous?.id) this.units = [];
		this.error = null;
		dyrParameters(moduleJson, dyrText).then(
			(u) => {
				if (this.#input !== input) return;
				this.units = u;
				this.error = null;
			},
			(e) => {
				if (this.#input === input) this.error = (e as Error).message;
			}
		);
		if (!this.#static) {
			this.#static = true;
			rmsLibrary().then(
				(l) => (this.library = l as LibraryModel[]),
				() => (this.#static = false)
			);
			unitPresets().then(
				(p) => (this.presets = p),
				() => {}
			);
		}
	}

	ofRole(role: string): LibraryModel[] {
		return this.library.filter((m) => m.role === role);
	}

	find(key: string | null): DyrGeneratorView | null {
		return key ? (this.units.find((u) => unitKey(u) === key) ?? null) : null;
	}
}

interface Glyph {
	unit: DyrGeneratorView;
	kind: UnitKind;
	position: [number, number];
	offset: [number, number];
}

const GLYPH: Record<UnitKind, { text: string; dark: [number, number, number]; light: [number, number, number] }> = {
	machine: { text: 'G', dark: [96, 165, 250], light: [37, 99, 235] },
	inverter: { text: 'I', dark: [74, 222, 128], light: [22, 128, 61] },
	static: { text: 'S', dark: [148, 163, 184], light: [100, 116, 139] }
};

/** One clickable chip per unit, beside its bus (G machine, I inverter,
 * S static), several units of a bus side by side. */
export function unitGlyphLayer(
	TextLayer: typeof TextLayerT,
	units: readonly DyrGeneratorView[],
	buses: readonly NetworkBus[],
	theme: 'light' | 'dark',
	onClick: (u: DyrGeneratorView) => void
): Layer {
	const pos = new Map(buses.map((b) => [b.id, [b.lon, b.lat] as [number, number]]));
	const count = new Map<number, number>();
	const data: Glyph[] = [];
	for (const u of units) {
		const p = pos.get(u.bus);
		if (!p) continue;
		const i = count.get(u.bus) ?? 0;
		count.set(u.bus, i + 1);
		data.push({ unit: u, kind: unitKind(u), position: p, offset: [12 + 15 * i, -12] });
	}
	return new TextLayer<Glyph>({
		id: 'dyn-units',
		data,
		getPosition: (g) => g.position,
		getText: (g) => GLYPH[g.kind].text,
		getPixelOffset: (g) => g.offset,
		getSize: 11,
		fontWeight: 700,
		getColor: theme === 'dark' ? [10, 20, 31, 255] : [255, 255, 255, 255],
		background: true,
		getBackgroundColor: (g) => [...GLYPH[g.kind][theme], 235] as [number, number, number, number],
		backgroundPadding: [3, 1],
		pickable: true,
		onClick: (info) => {
			if (!info.object) return false;
			onClick(info.object.unit);
			return true;
		},
		updateTriggers: { getBackgroundColor: theme, getColor: theme }
	}) as unknown as Layer;
}
