/** The validated WECC 240 scenarios (docs/src/rms-validation.md): four
 * 20 s runs at a 5 ms step on the network with the validation study's loads,
 * under the options that simulate what PowerSimulationsDynamics.jl 0.16.2
 * simulates. A run is "validated" only when all of that holds; the badge
 * names whatever differs. Pure.
 * @block Validated WECC 240 scenarios */

import type { RmsCaseOptions } from '@tellegen/engine';
import { busFault, branchTrip, generatorTrip, type DynEvent } from './events.js';

export const VALIDATION_OPTIONS: Required<
	Pick<
		RmsCaseOptions,
		| 'load_model'
		| 'psid_lv_gain'
		| 'reactive_limits'
		| 'psid_saturation'
		| 'psid_converter_ramp'
		| 'low_voltage_load'
	>
> = {
	load_model: 'constant_impedance',
	psid_lv_gain: true,
	reactive_limits: false,
	psid_saturation: true,
	psid_converter_ramp: true,
	low_voltage_load: { kind: 'exact' }
};

export const VALIDATION_DT = 0.005;
export const VALIDATION_T_END = 20;

export interface Preset {
	id: string;
	title: string;
	events: () => DynEvent[];
}

export const WECC240_PRESETS: Preset[] = [
	{
		id: 'trip-1101-1401',
		title: 'Trip line 1101–1401',
		events: () => [branchTrip(1101, 1401, '1')]
	},
	{
		id: 'trip-1431-N',
		title: 'Trip Palo Verde unit',
		events: () => [generatorTrip(1431, 'N')]
	},
	{
		id: 'fault-1032',
		title: 'Fault at bus 1032',
		events: () => [busFault(1032, 60, { x: 0.05, clearAfter: 0.1 } as Partial<DynEvent>)]
	},
	{
		id: 'fault-1101',
		title: 'Fault at bus 1101',
		events: () => [busFault(1101, 60, { x: 0.01, clearAfter: 0.1 } as Partial<DynEvent>)]
	}
];

/** The run repeats the validation study's scenario and settings. */
export const VALIDATED_CLAIM = 'Matches';

const sameOptions = (o: RmsCaseOptions): string[] => {
	const diff: string[] = [];
	const v = VALIDATION_OPTIONS;
	if ((o.load_model ?? 'power_flow') !== v.load_model) diff.push('load model');
	if (!!o.psid_lv_gain !== v.psid_lv_gain) diff.push('renewable low-voltage gain');
	if ((o.reactive_limits ?? true) !== v.reactive_limits) diff.push('reactive limits');
	if (!!o.psid_saturation !== v.psid_saturation) diff.push('saturation');
	if (!!o.psid_converter_ramp !== v.psid_converter_ramp) diff.push('converter ramp');
	if ((o.low_voltage_load?.kind ?? 'break') !== 'exact') diff.push('low-voltage load');
	if (o.renewable_plants === false) diff.push('renewable plants held');
	return diff;
};

/** The badge of a run of preset `presetId`: validated, or which settings
 * differ from the study's. */
export function validationBadge(input: {
	presetId: string | null;
	presetEvents: readonly DynEvent[] | null;
	events: readonly DynEvent[];
	options: RmsCaseOptions;
	dt: number;
	tEnd: number;
	validationNetwork: boolean;
}): { validated: boolean; text: string; detail?: string } | null {
	if (!input.presetId || !input.presetEvents) return null;
	const diff = sameOptions(input.options);
	if (Math.abs(input.dt - VALIDATION_DT) > 1e-12) diff.push('step');
	if (Math.abs(input.tEnd - VALIDATION_T_END) > 1e-12) diff.push('duration');
	if (!input.validationNetwork) diff.push('loads (the study uses its own load values)');
	const strip = (e: DynEvent) => JSON.stringify({ ...e, id: undefined });
	const same =
		input.events.length === input.presetEvents.length &&
		input.events.every((e, i) => strip(e) === strip(input.presetEvents![i]));
	if (!same) diff.push('events');
	if (diff.length === 0) return { validated: true, text: VALIDATED_CLAIM };
	return { validated: false, text: 'Settings differ', detail: diff.join(', ') };
}
