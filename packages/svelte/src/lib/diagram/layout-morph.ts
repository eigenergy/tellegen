/** Animated transition between two layouts of the same buses, so the eye can follow a feeder. */
export const MORPH_MS = 900;
/** Measured smooth up to 98,633 buses on the reference machine (M4 Max, #177 M0 evidence, with a
 * warm-up frame); larger drawings switch instantly until measured. */
export const MORPH_MAX_BUSES = 100_000;

export interface Camera {
	target: ArrayLike<number>;
	zoom: number;
}

export const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

/** Re-express layout A positions in layout B's world, so each bus starts where it is on screen. */
export function screenAnchored(
	positions: ArrayLike<number>,
	from: Camera,
	to: Camera
): Float32Array {
	const k = 2 ** from.zoom / 2 ** to.zoom;
	const out = new Float32Array(positions.length);
	for (let i = 0; i < positions.length; i += 3) {
		out[i] = to.target[0] + (positions[i] - from.target[0]) * k;
		out[i + 1] = to.target[1] + (positions[i + 1] - from.target[1]) * k;
	}
	return out;
}

export function lerpInto(out: Float32Array, a: ArrayLike<number>, b: ArrayLike<number>, t: number) {
	for (let i = 0; i < out.length; i++) out[i] = a[i] + (b[i] - a[i]) * t;
	return out;
}

/** Straight bus-to-bus chords while moving; routed geometry returns when the transition ends. */
export function chordsInto(
	sources: Float32Array,
	targets: Float32Array,
	endpoints: ArrayLike<number>,
	positions: ArrayLike<number>
) {
	for (let b = 0; b < endpoints.length / 2; b++) {
		const u = endpoints[b * 2] * 3;
		const v = endpoints[b * 2 + 1] * 3;
		for (let k = 0; k < 3; k++) {
			sources[b * 3 + k] = positions[u + k];
			targets[b * 3 + k] = positions[v + k];
		}
	}
}
