import type { DcFormula } from '@tellegen/engine';

/** How a DC calculation models each line, named for the tool whose convention each matches
 * (docs/src/dc-compatibility.md). Settings offers these as the DC line model. */
export const DC_VARIANTS: ReadonlyArray<{ id: DcFormula; label: string }> = [
	{ id: 'series_susceptance', label: 'Tellegen (default)' },
	{ id: 'tap_adjusted_reactance', label: 'MATPOWER' },
	{ id: 'series_susceptance_no_phase_shift', label: 'PowerModels DCP' },
	{ id: 'tap_adjusted_series_susceptance', label: 'Susceptance with taps' },
	{ id: 'reactance_only', label: 'Reactance only (1/x)' }
];

export function dcVariantLabel(id: string | null | undefined): string {
	if (id == null) return 'Default';
	return DC_VARIANTS.find((variant) => variant.id === id)?.label ?? id;
}
