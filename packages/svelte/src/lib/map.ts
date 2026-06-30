import type { ComponentProps } from 'svelte';
import TellegenMap from './TellegenMap.svelte';

export { default as TellegenMap } from './TellegenMap.svelte';
export {
	branchColor,
	branchWidth,
	busNeutral,
	busRadius,
	lmpColor,
	lmpDomain,
	lmpGradient,
	scalarDomain,
	sensColor,
	sensFlatColor,
	sensGradient,
	sensNeutral,
	sensitivityDomain
} from './colors.js';
export { caseDeltas, displayMetaFor, displaySeriesFor } from './display.js';
export type { RGBA, SensitivityDomain } from './colors.js';
export type { DisplayOption } from './display.js';

export type TellegenMapProps = ComponentProps<typeof TellegenMap>;
