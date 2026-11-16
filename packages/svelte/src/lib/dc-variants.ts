import type { DcFormula } from '@tellegen/engine';

/** How a DC calculation models each line (docs/src/dc-compatibility.md). Settings offers these as
 * the DC line model, next to "Default", the case's own choice. `matches` names the tool whose
 * convention each follows, for a tooltip. */
export const DC_VARIANTS: ReadonlyArray<{
	id: DcFormula;
	label: string;
	matches: string;
}> = [
	{ id: 'series_susceptance', label: 'Series impedance', matches: 'Tellegen' },
	{
		id: 'tap_adjusted_reactance',
		label: 'Reactance with taps',
		matches: 'MATPOWER'
	},
	{
		id: 'series_susceptance_no_phase_shift',
		label: 'Series impedance, no phase shift',
		matches: 'PowerModels'
	},
	{
		id: 'tap_adjusted_series_susceptance',
		label: 'Series impedance with taps',
		matches: ''
	},
	{ id: 'reactance_only', label: 'Reactance only', matches: '' }
];

export function dcVariantLabel(id: string | null | undefined): string {
	if (id == null) return 'Default';
	return DC_VARIANTS.find((variant) => variant.id === id)?.label ?? id;
}
