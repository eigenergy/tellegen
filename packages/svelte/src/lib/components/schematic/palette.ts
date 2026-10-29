import type { RGBA } from '../../colors.js';
import type { VoltageClass, ZoneState } from '../../diagram/index.js';

export const INK: RGBA = [32, 36, 43, 255];
export const ACCENT: RGBA = [178, 94, 0, 255];
export const PAPER: RGBA = [252, 251, 247, 255];
export const DANGER: RGBA = [184, 50, 42, 255];
export const WARNING: RGBA = [178, 94, 0, 255];
export const SYMBOL_INK: RGBA = [52, 56, 62, 255];
/** Not connected to any source through closed branches: drawn faint in every colour mode. */
export const DEAD: RGBA = [196, 191, 182, 110];
export const HIDDEN: RGBA = [0, 0, 0, 0];
/** LV networks within limits: corridors, outlines and tags in the warm grey of LV lines, so blue
 * stays the colour of MV and above. */
export const ZONE_INK: RGBA = [112, 104, 90, 255];

export const MONO = 'IBM Plex Mono, ui-monospace, monospace';

/** A colour at another opacity. */
export const alpha = (c: RGBA, a: number): RGBA => [c[0], c[1], c[2], a];

/** Ink for a label or mark by severity. */
export const stateInk = (state: string | undefined): RGBA =>
	state === 'violation' ? DANGER : state === 'warning' ? WARNING : INK;

/** Screen width of a zone's corridor on geographic positions. */
export const CORRIDOR_PX = 9;

/** Line colour per voltage class in the topology view: shades of one blue, darker for higher
 * levels, and LV in the quiet grey that carries most of a distribution drawing. Clear of the
 * warning and violation colours and of the accent used for selection and flow. */
export const LEVEL_COLOURS: Record<VoltageClass, RGBA> = {
	hv: [38, 44, 112, 240],
	sub: [30, 76, 140, 240],
	mv: [58, 118, 178, 235],
	lv: [138, 131, 117, 210]
};

/** Hover emphasis for a network and its feeder, beneath the lines, whatever the network's state. */
export const HALO: RGBA = [64, 72, 92, 255];

/** The hovered element's path back to the source: green, the one family no level, state or
 * selection uses, so it reads over the halos. */
export const PATH: RGBA = [16, 138, 96, 245];

/** Fill and outline per zone state. */
export const ZONE_COLOURS: Record<ZoneState, [RGBA, RGBA]> = {
	missing: [
		[112, 104, 90, 16],
		[112, 104, 90, 90]
	],
	normal: [
		[112, 104, 90, 16],
		[112, 104, 90, 90]
	],
	warning: [
		[178, 94, 0, 30],
		[178, 94, 0, 170]
	],
	violation: [
		[184, 50, 42, 34],
		[184, 50, 42, 190]
	]
};

/** MV and LV line widths in pixels: heavier when results colour the lines. */
export const lineWidths = (resultColours: boolean): [number, number] =>
	resultColours ? [3, 1.8] : [2.4, 1.1];
