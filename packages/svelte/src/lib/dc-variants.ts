import type { DcFormula } from '@tellegen/engine';

/** How a DC calculation models each line, named for what each model keeps. Settings offers these
 * as the DC line model. */
export const DC_VARIANTS: ReadonlyArray<{ id: DcFormula; label: string }> = [
	{ id: 'series_susceptance', label: 'Series susceptance' },
	{ id: 'series_susceptance_no_phase_shift', label: 'No phase shift' },
	{ id: 'tap_adjusted_reactance', label: 'Tap-adjusted reactance' },
	{ id: 'tap_adjusted_series_susceptance', label: 'Tap-adjusted susceptance' },
	{ id: 'reactance_only', label: 'Reactance only' }
];

export function dcVariantLabel(id: string | null | undefined): string {
	if (id == null) return 'Default';
	return DC_VARIANTS.find((variant) => variant.id === id)?.label ?? id;
}
