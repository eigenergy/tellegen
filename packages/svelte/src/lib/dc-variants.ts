import type { DcFormula } from '@tellegen/engine';

/** How a DC calculation models each line, in the names researchers know.
 * The Network panel offers these; Settings does not repeat the choice. */
export const DC_VARIANTS: ReadonlyArray<{ id: DcFormula; label: string }> = [
	{ id: 'series_susceptance', label: 'Series susceptance (default)' },
	{
		id: 'series_susceptance_no_phase_shift',
		label: 'PGLib and PowerModels DCP'
	},
	{ id: 'tap_adjusted_reactance', label: 'MATPOWER and PowerModels DCMP' },
	{
		id: 'tap_adjusted_series_susceptance',
		label: 'Tap-adjusted series susceptance'
	},
	{ id: 'reactance_only', label: 'Reactance only (1/x)' }
];

export function dcVariantLabel(id: string | null | undefined): string {
	if (id == null) return "The case's own choice";
	return DC_VARIANTS.find((variant) => variant.id === id)?.label ?? id;
}
