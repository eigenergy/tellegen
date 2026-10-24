import { priceColor, sensColor, type RGBA } from '../colors.js';
import type { PairVoltage } from './snapshot.js';

export type ColourMode = 'none' | 'voltage' | 'voltage-band' | 'loading' | 'loading-heat';

export const COLOUR_MODES: Record<ColourMode, { label: string; continuous: boolean }> = {
	none: { label: 'Equipment / topology', continuous: false },
	voltage: { label: 'Voltage limits', continuous: false },
	'voltage-band': { label: 'Voltage within band', continuous: true },
	loading: { label: 'Loading thresholds', continuous: false },
	'loading-heat': { label: 'Loading (continuous)', continuous: true }
};

/** Threshold and continuous variants of one quantity share issue focus, warnings and presets. */
export function colourFamily(mode: ColourMode): 'voltage' | 'loading' | 'none' {
	return mode.startsWith('voltage') ? 'voltage' : mode.startsWith('loading') ? 'loading' : 'none';
}

/** Position in the declared band: −1 at min, 0 mid-band, +1 at max; beyond ±1 is a violation. */
export function bandPosition(p: PairVoltage): number | null {
	if (p.min === null || p.max === null || !(p.max > p.min)) return null;
	return (2 * p.volts - p.min - p.max) / (p.max - p.min);
}

/** The pair nearest a limit decides a bus colour, so a violation can never be averaged away. */
export function worstBand(pairs: PairVoltage[]): number | null {
	let worst: number | null = null;
	for (const p of pairs) {
		const t = bandPosition(p);
		if (t !== null && (worst === null || Math.abs(t) > Math.abs(worst))) worst = t;
	}
	return worst;
}

/** The application's sequential ramp; light loads start part-way up so they stay visible on paper. */
export const loadingColour = (loading: number): RGBA =>
	priceColor(0.15 + 0.85 * Math.min(1, Math.max(0, loading / 100)));

/** The application's diverging ramp over the declared band, clamped at the limits. */
export const bandColour = (t: number): RGBA => sensColor(Math.max(-1, Math.min(1, t)));

export const MISSING_COLOUR: RGBA = [128, 128, 128, 170];
