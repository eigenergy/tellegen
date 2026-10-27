/** The IEC two-winding transformer symbol as a prepacked deck.gl icon atlas:
 * two overlapping circles on a canvas that deck.gl uploads as a texture.
 * Nothing is fetched, so the symbol needs no `data:` in a CSP's connect-src.
 * Drawn white on transparent with `mask: true`, so the layer's getColor tints
 * it. */

const SIZE = 128;
const RADIUS = 34;
const OFFSET = 17;
const STROKE = 9;

/** The icon name for the layer's getIcon. */
export const TRANSFORMER_ICON = 'transformer';

/** The atlas holds the one symbol, anchored at its center. */
export const transformerIconMapping = {
	[TRANSFORMER_ICON]: {
		x: 0,
		y: 0,
		width: SIZE,
		height: SIZE,
		anchorX: SIZE / 2,
		anchorY: SIZE / 2,
		mask: true
	}
};

let cached: HTMLCanvasElement | null = null;

/** Rasterize the glyph lazily; layers only build client-side, after the map
 * modules load, so document is always available here. The 128 px backing keeps
 * a ~16 px on-screen symbol crisp on 2x/3x displays. Every call returns the
 * same canvas, so deck.gl's prop diff keeps the texture it made from it. */
export function transformerIconAtlas(): HTMLCanvasElement {
	if (cached) return cached;
	const canvas = document.createElement('canvas');
	canvas.width = SIZE;
	canvas.height = SIZE;
	const ctx = canvas.getContext('2d')!;
	ctx.strokeStyle = '#fff';
	ctx.lineWidth = STROKE;
	for (const dx of [-OFFSET, OFFSET]) {
		ctx.beginPath();
		ctx.arc(SIZE / 2 + dx, SIZE / 2, RADIUS, 0, 2 * Math.PI);
		ctx.stroke();
	}
	cached = canvas;
	return cached;
}
