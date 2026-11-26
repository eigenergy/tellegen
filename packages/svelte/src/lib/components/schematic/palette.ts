import type { BasemapTheme } from '../../basemap.js';
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

/** The diagrams' label font: the UI's IBM Plex Sans. */
export const LABEL_FONT = 'IBM Plex Sans, system-ui, sans-serif';

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
const LEVEL_COLOURS: Record<VoltageClass, RGBA> = {
	hv: [38, 44, 112, 240],
	sub: [30, 76, 140, 240],
	mv: [58, 118, 178, 235],
	lv: [138, 131, 117, 210]
};

export interface SchematicPalette {
	levels: Record<VoltageClass, RGBA>;
	ink: RGBA;
	symbol: RGBA;
	paper: RGBA;
	muted: RGBA;
}

export const SCHEMATIC_PALETTES: Record<BasemapTheme, SchematicPalette> = {
	light: {
		levels: LEVEL_COLOURS,
		ink: INK,
		symbol: SYMBOL_INK,
		paper: PAPER,
		muted: [77, 81, 86, 255]
	},
	dark: {
		// LV lines fade at overview zoom. Keep enough opacity for thin laterals against dark land.
		levels: {
			hv: [188, 184, 255, 255],
			sub: [138, 185, 255, 255],
			mv: [105, 175, 227, 255],
			lv: [225, 214, 197, 255]
		},
		ink: [230, 234, 240, 255],
		symbol: [203, 212, 223, 255],
		paper: [17, 22, 29, 255],
		muted: [178, 188, 200, 255]
	}
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
