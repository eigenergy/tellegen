import type { RGBA } from '../colors.js';

/** A bus-value field rendered as an image over the drawing: the shaded contour display of
 * Overbye and Weber, which lets the eye find low voltages or price separation by area before it
 * reads any number. */
export interface FieldImage {
	data: Uint8ClampedArray<ArrayBuffer>;
	width: number;
	height: number;
	/** Drawing-space extent covered by the image: `x0, y0, x1, y1`. Row 0 is at `y0`. */
	bounds: [number, number, number, number];
}

/**
 * Inverse-distance-weighted interpolation of point values onto a coarse image.
 *
 * Each pixel takes the weighted mean of points within `reach` (drawing units), weighted by
 * `1 / d²`; pixels farther than `reach` from every point stay transparent, so the field hugs the
 * network instead of filling the map. Opacity fades towards the edge of reach. `colour` maps a
 * value to a colour, or to null where the value needs no shading.
 */
export function fieldImage(
	positions: Float32Array | Float64Array,
	values: ArrayLike<number>,
	colour: (value: number) => RGBA | null,
	options: { reach: number; maxSize?: number }
): FieldImage | null {
	const { reach } = options;
	const maxSize = options.maxSize ?? 256;
	const n = values.length;
	if (!(reach > 0) || n === 0) return null;
	let x0 = Infinity,
		y0 = Infinity,
		x1 = -Infinity,
		y1 = -Infinity,
		count = 0;
	for (let i = 0; i < n; i++) {
		const x = positions[i * 3],
			y = positions[i * 3 + 1];
		if (!Number.isFinite(values[i]) || !Number.isFinite(x) || !Number.isFinite(y)) continue;
		x0 = Math.min(x0, x);
		y0 = Math.min(y0, y);
		x1 = Math.max(x1, x);
		y1 = Math.max(y1, y);
		count++;
	}
	if (!count) return null;
	x0 -= reach;
	y0 -= reach;
	x1 += reach;
	y1 += reach;
	const spanX = x1 - x0,
		spanY = y1 - y0;
	const scale = maxSize / Math.max(spanX, spanY);
	const width = Math.max(2, Math.round(spanX * scale));
	const height = Math.max(2, Math.round(spanY * scale));

	// Bucket the points in cells one reach wide, so each pixel checks its 3 × 3 neighbourhood.
	const cols = Math.max(1, Math.ceil(spanX / reach));
	const rows = Math.max(1, Math.ceil(spanY / reach));
	const head = new Int32Array(cols * rows).fill(-1);
	const next = new Int32Array(n).fill(-1);
	for (let i = 0; i < n; i++) {
		const x = positions[i * 3],
			y = positions[i * 3 + 1];
		if (!Number.isFinite(values[i]) || !Number.isFinite(x) || !Number.isFinite(y)) continue;
		const c = Math.min(cols - 1, Math.floor((x - x0) / reach));
		const r = Math.min(rows - 1, Math.floor((y - y0) / reach));
		next[i] = head[r * cols + c];
		head[r * cols + c] = i;
	}

	const data = new Uint8ClampedArray(width * height * 4);
	const reach2 = reach * reach;
	for (let py = 0; py < height; py++) {
		const y = y0 + ((py + 0.5) / height) * spanY;
		const r = Math.min(rows - 1, Math.floor((y - y0) / reach));
		for (let px = 0; px < width; px++) {
			const x = x0 + ((px + 0.5) / width) * spanX;
			const c = Math.min(cols - 1, Math.floor((x - x0) / reach));
			let sum = 0,
				weight = 0,
				nearest = Infinity;
			for (let dr = -1; dr <= 1; dr++) {
				const rr = r + dr;
				if (rr < 0 || rr >= rows) continue;
				for (let dc = -1; dc <= 1; dc++) {
					const cc = c + dc;
					if (cc < 0 || cc >= cols) continue;
					for (let i = head[rr * cols + cc]; i >= 0; i = next[i]) {
						const dx = positions[i * 3] - x,
							dy = positions[i * 3 + 1] - y;
						const d2 = dx * dx + dy * dy;
						if (d2 > reach2) continue;
						nearest = Math.min(nearest, d2);
						const w = 1 / Math.max(d2, 1e-12);
						sum += w * values[i];
						weight += w;
					}
				}
			}
			if (!weight) continue;
			const rgba = colour(sum / weight);
			if (!rgba) continue;
			// Full strength within half the reach of a bus, fading to nothing at the reach.
			const fade = Math.min(1, 2 * (1 - Math.sqrt(nearest) / reach));
			const k = (py * width + px) * 4;
			data[k] = rgba[0];
			data[k + 1] = rgba[1];
			data[k + 2] = rgba[2];
			data[k + 3] = Math.round(rgba[3] * fade);
		}
	}
	return { data, width, height, bounds: [x0, y0, x1, y1] };
}

const LOW: RGBA = [33, 102, 172, 255];
const HIGH: RGBA = [182, 106, 53, 255];

/** Where a voltage sits in its bus's operating band: -1 at the minimum, 0 mid-band, +1 at the
 * maximum, beyond ±1 outside it. Undefined without valid bounds. */
export function voltageBandPosition(
	value: number | undefined,
	vmin: number | undefined,
	vmax: number | undefined
): number | undefined {
	if (value === undefined || vmin === undefined || vmax === undefined || !(vmax > vmin))
		return undefined;
	return (value - (vmin + vmax) / 2) / ((vmax - vmin) / 2);
}

/** Voltage shading by band position (`voltageBandPosition`): clear in the middle half of the band,
 * rising to full strength at a limit; blue towards the minimum, brown towards the maximum.
 * Normal operation stays quiet, and buses near or past their limits show by area. */
export function voltageFieldColour(position: number, maxAlpha = 150): RGBA | null {
	const strength = Math.min(1, Math.max(0, (Math.abs(position) - 0.5) / 0.5));
	if (strength <= 0) return null;
	const [r, g, b] = position < 0 ? LOW : HIGH;
	return [r, g, b, Math.round(maxAlpha * strength)];
}

/** The median of the finite values, or undefined when there are none. */
export function median(values: Iterable<number>): number | undefined {
	const sorted = [...values].filter(Number.isFinite).sort((a, b) => a - b);
	if (!sorted.length) return undefined;
	const mid = sorted.length >> 1;
	return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

/** The deviation from the median LMP that shades at full strength: the largest deviation in the
 * result, but never less than 5% of the median price or one price unit, so numerical noise and
 * small spreads stay faint. */
export function priceSpread(prices: ArrayLike<number>, centre: number): number {
	let largest = 0;
	for (let i = 0; i < prices.length; i++)
		if (Number.isFinite(prices[i])) largest = Math.max(largest, Math.abs(prices[i] - centre));
	return Math.max(largest, 0.05 * Math.abs(centre), 1);
}

/** Price shading by deviation from the median LMP, over `spread` (`priceSpread`): clear within a
 * tenth of the spread, blue where power is cheaper and brown where it is dearer. It shows where
 * congestion separates prices, whatever their level. */
export function priceFieldColour(deviation: number, spread: number, maxAlpha = 130): RGBA | null {
	const strength = Math.min(1, Math.max(0, (Math.abs(deviation) / spread - 0.1) / 0.9));
	if (strength <= 0) return null;
	const [r, g, b] = deviation < 0 ? LOW : HIGH;
	return [r, g, b, Math.round(maxAlpha * strength)];
}
