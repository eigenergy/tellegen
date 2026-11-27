/** The map's colour keys as data, so a host can draw a legend that matches
 * `TellegenMap`: the line loading steps and the active bus colour scale. */

import {
	LOADING_AMBER,
	LOADING_AT_LIMIT,
	LOADING_ATTENTION,
	LOADING_BASE,
	LOADING_RED,
	scalarDomain,
	scalarGradient,
	scalarStops,
	type ColorStop,
	type RGBA,
	type RampTheme
} from './colors.js';
import { limitWords } from './diagram/verdict.js';
import type { DisplayOption } from './display.js';
import { extent } from './format.js';
import type { DisplayMode } from './state.svelte.js';

/** One step of the line loading key. */
export interface LineLoadingStep {
	key: 'normal' | 'near-limit' | 'at-limit';
	label: string;
	color: RGBA;
	/** Loading, as a fraction of the rating, from which a line takes this colour. */
	from: number;
}

/** The map's three line colours by loading, as `branchColor` draws them. The
 * last label says "At limit" for an optimal power flow and "Over limit" for a
 * power flow, whose lines are not held to their ratings. */
export function lineLoadingKey(formulation?: string | null): LineLoadingStep[] {
	return [
		{
			key: 'normal',
			label: `Below ${Math.round(LOADING_ATTENTION * 100)}%`,
			color: LOADING_BASE,
			from: 0
		},
		{ key: 'near-limit', label: 'Near limit', color: LOADING_AMBER, from: LOADING_ATTENTION },
		{
			key: 'at-limit',
			label: limitWords(formulation).key,
			color: LOADING_RED,
			from: LOADING_AT_LIMIT
		}
	];
}

/** One end of a bus colour scale. `clamped` is true when some buses lie
 * beyond it and take the end colour, so a legend writes ≤ or ≥. */
export interface ScaleEnd {
	value: number;
	clamped: boolean;
}

/** The domain the map colours buses over for `values` in `mode`, with its
 * clamped ends, or `null` without a finite value. `uniform` is the value when
 * every bus has the same one, to within display noise. */
export function displayRange(
	mode: DisplayMode,
	values: readonly number[]
): { lo: ScaleEnd; hi: ScaleEnd; uniform: number | null } | null {
	const finite = values.filter(Number.isFinite);
	if (finite.length === 0) return null;
	const domain = scalarDomain(mode, finite);
	const { min, max } = extent(finite);
	const flatThreshold = mode === 'price' || mode === 'demand' ? 1 : 1e-5;
	return {
		lo: { value: domain.lo, clamped: min < domain.lo - flatThreshold / 20 },
		hi: { value: domain.hi, clamped: max > domain.hi + flatThreshold / 20 },
		uniform: max - min < flatThreshold ? finite[0] : null
	};
}

/** The bus colour scale the map draws for one display variable. */
export interface BusColorScale extends NonNullable<ReturnType<typeof displayRange>> {
	mode: DisplayMode;
	label: string;
	unit: string;
	/** Ramp stops along the bar, low end first. Demand's sit on its square-root scale. */
	stops: ColorStop[];
	/** The same ramp as a CSS `linear-gradient`. */
	gradient: string;
}

/** The scale behind the bus colours for a display variable and its values,
 * such as the controller's `activeDisplay`, in the map's theme. `null` when
 * no value is finite. */
export function busColorScale(
	display: Pick<DisplayOption, 'mode' | 'label' | 'unit'> & {
		values: readonly { value: number }[];
	},
	theme: RampTheme = 'light'
): BusColorScale | null {
	const range = displayRange(
		display.mode,
		display.values.map((entry) => entry.value)
	);
	if (!range) return null;
	return {
		...range,
		mode: display.mode,
		label: display.label,
		unit: display.unit,
		stops: scalarStops(display.mode, theme),
		gradient: scalarGradient(display.mode, theme)
	};
}
