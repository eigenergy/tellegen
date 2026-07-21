/** The IEC two-winding transformer symbol as a deck.gl icon: two overlapping
 * circles, rasterized once to a data URL. Drawn white on transparent with
 * `mask: true`, so the layer's getColor tints it. */

export interface TransformerIcon {
	url: string;
	width: number;
	height: number;
	anchorX: number;
	anchorY: number;
	mask: true;
}

const SIZE = 128;
const RADIUS = 34;
const OFFSET = 17;
const STROKE = 9;

let cached: TransformerIcon | null = null;

/** Rasterize the glyph lazily; layers only build client-side, after the map
 * modules load, so document is always available here. The 128 px backing keeps
 * a ~16 px on-screen symbol crisp on 2x/3x displays. */
export function transformerIcon(): TransformerIcon {
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
	cached = {
		url: canvas.toDataURL(),
		width: SIZE,
		height: SIZE,
		anchorX: SIZE / 2,
		anchorY: SIZE / 2,
		mask: true
	};
	return cached;
}
