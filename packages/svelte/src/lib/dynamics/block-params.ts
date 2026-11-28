/** The parameters of each controller block, as the block-diagram editor
 * shows them: label, unit, what the parameter does and the bound the engine
 * checks (`checkGraph` refuses the rest, such as `min > max`). The same
 * field (`ParameterField`) shows them and
 * a unit's DYR parameters.
 * @block Block parameters */
import type { GraphBlock } from '@tellegen/engine';

export interface ParamSpec {
	key: string;
	label: string;
	/** `''` for a pure number (per unit, a gain). */
	unit: string;
	description: string;
	/** The smallest value the engine accepts. */
	min?: number;
}

const gain = (description = 'Gain K'): ParamSpec => ({ key: 'k', label: 'K', unit: 'pu', description });
const time = (key: string, label: string, description: string): ParamSpec => ({
	key,
	label,
	unit: 's',
	description,
	min: 0
});
const lim = (key: 'min' | 'max', what = 'output'): ParamSpec => ({
	key,
	label: key === 'min' ? 'Min' : 'Max',
	unit: 'pu',
	description: `${key === 'min' ? 'Lower' : 'Upper'} limit of the ${what}`
});

export const BLOCK_PARAMS: Partial<Record<GraphBlock['type'], ParamSpec[]>> = {
	const: [{ key: 'value', label: 'Value', unit: 'pu', description: 'The constant output' }],
	gain: [gain()],
	integrator: [gain('K of K/s')],
	clamp: [lim('min'), lim('max')],
	gate: [
		{ key: 'lo', label: 'Low', unit: 'pu', description: 'Passes the first input while the second is at least this (0: no lower limit)' },
		{ key: 'hi', label: 'High', unit: 'pu', description: 'Passes the first input while the second is at most this (0: no upper limit)' }
	],
	lag: [gain('K of K/(1 + sT)'), time('t', 'T', 'Time constant; 0 makes the block algebraic')],
	washout: [gain('K of Ks/(1 + sT); a unity-gain washout has K = T'), time('t', 'T', 'Time constant')],
	lag_limited: [
		gain('K of K/(1 + sT)'),
		time('t', 'T', 'Time constant; 0 makes the block algebraic'),
		lim('min', 'state (non-windup)'),
		lim('max', 'state (non-windup)')
	],
	lead_lag: [
		gain('K of K(1 + sT1)/(1 + sT2)'),
		{ key: 't1', label: 'T1', unit: 's', description: 'Lead time constant' },
		time('t2', 'T2', 'Lag time constant; 0 makes the block algebraic')
	],
	pi_limited: [
		{ key: 'kp', label: 'Kp', unit: 'pu', description: 'Proportional gain' },
		{ key: 'ki', label: 'Ki', unit: '1/s', description: 'Integral gain' },
		lim('min'),
		lim('max')
	],
	lag2: [
		gain('K of K/(1 + sT1 + s²T2)'),
		{ key: 't1', label: 'T1', unit: 's', description: 'First-order coefficient' },
		time('t2', 'T2', 'Second-order coefficient (s²)')
	],
	lead_lag2: [
		{ key: 't1', label: 'T1', unit: 's', description: 'Denominator, first order: (1 + sT3 + s²T4)/(1 + sT1 + s²T2)' },
		time('t2', 'T2', 'Denominator, second order (s²)'),
		{ key: 't3', label: 'T3', unit: 's', description: 'Numerator, first order' },
		{ key: 't4', label: 'T4', unit: 's', description: 'Numerator, second order (s²)' }
	],
	sat_quadratic: [
		{ key: 'a', label: 'A', unit: 'pu', description: 'Saturation starts above A: S(u) = B(u − A)²/u' },
		{ key: 'b', label: 'B', unit: '1/pu', description: 'Saturation factor B' }
	],
	min: [
		{
			key: 'tie',
			label: 'Tie',
			unit: 'pu',
			description: 'With two inputs, the second wins unless the first is below it by more than this (0: plain minimum)',
			min: 0
		}
	]
};
