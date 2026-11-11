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

/** A running layout transition: bus positions `cur` between the layouts, and the straight
 * chords `s`→`t` between each branch's `endpoints`, for the renderer to draw. */
export interface Morph {
	from: Float32Array;
	cur: Float32Array;
	s: Float32Array;
	t: Float32Array;
	endpoints: Int32Array;
	start: number | null;
	warm: boolean;
	raf: number;
}

/** Start a transition from `before` (on screen through camera `from`) to `destination` (through
 * `to`). `onframe` receives the morph each frame and null when it ends, for the caller to store
 * and redraw. A warm-up frame lets the renderer upload the target first. Cancel with
 * `cancelAnimationFrame(morph.raf)`. */
export function startMorph(
	before: ArrayLike<number>,
	destination: ArrayLike<number>,
	endpoints: Int32Array,
	from: Camera,
	to: Camera,
	onframe: (morph: Morph | null) => void
): Morph {
	const n = endpoints.length / 2;
	const m: Morph = {
		from: screenAnchored(before, from, to),
		cur: new Float32Array(before.length),
		s: new Float32Array(n * 3),
		t: new Float32Array(n * 3),
		endpoints,
		start: null,
		warm: false,
		raf: 0
	};
	const tick = (now: number) => {
		if (!m.warm) {
			m.warm = true;
			m.raf = requestAnimationFrame(tick);
			return;
		}
		m.start ??= now;
		const k = ease(Math.min(1, (now - m.start) / MORPH_MS));
		lerpInto(m.cur, m.from, destination, k);
		chordsInto(m.s, m.t, m.endpoints, m.cur);
		if (k < 1) {
			onframe(m);
			m.raf = requestAnimationFrame(tick);
		} else onframe(null);
	};
	m.raf = requestAnimationFrame(tick);
	return m;
}
