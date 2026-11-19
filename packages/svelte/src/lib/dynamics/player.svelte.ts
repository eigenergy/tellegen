/** Playback of a dynamics run: the time shown on the map and the charts,
 * play/pause, speed, and the slow-motion windows around events. The rAF
 * loop only advances `t`; the map layers and the chart playhead read it.
 * @block Playback */

export const SPEEDS = [0.1, 0.25, 1, 4] as const;
export type Speed = (typeof SPEEDS)[number];

/** Slow motion around events: 0.25× from 0.1 s before to 1.5 s after. */
export const SLOW_BEFORE = 0.1;
export const SLOW_AFTER = 1.5;
export const SLOW_SPEED = 0.25;

/** The playback rate at simulated time `t`. */
export function rateAt(t: number, speed: number, events: readonly number[], slow: boolean): number {
	if (!slow) return speed;
	const near = events.some((e) => t >= e - SLOW_BEFORE && t <= e + SLOW_AFTER);
	return near ? Math.min(speed, SLOW_SPEED) : speed;
}

/** Where playback starts after a run: half a second before the first event
 * (autoplay), or, with reduced motion, parked one cycle after it. */
export function startTime(
	events: readonly number[],
	f0: number,
	reducedMotion: boolean
): { t: number; play: boolean } {
	const first = events.length ? Math.min(...events) : 0;
	if (reducedMotion) return { t: first + 1 / f0, play: false };
	return { t: Math.max(0, first - 0.5), play: true };
}

/** Advance `t` by `wallSeconds` of wall time, capped at `tEnd`. */
export function advance(
	t: number,
	wallSeconds: number,
	speed: number,
	events: readonly number[],
	slow: boolean,
	tEnd: number
): number {
	// Small steps so a slow window is not jumped over at 4×.
	let left = Math.min(wallSeconds, 0.25);
	let x = t;
	while (left > 1e-9 && x < tEnd) {
		const h = Math.min(left, 1 / 120);
		x += h * rateAt(x, speed, events, slow);
		left -= h;
	}
	return Math.min(x, tEnd);
}

export class DynamicsPlayer {
	t = $state(0);
	playing = $state(false);
	speed = $state<Speed>(1);
	slowAroundEvents = $state(true);
	tEnd = $state(0);
	events: number[] = $state.raw([]);
	#raf = 0;
	#last = 0;

	load(tEnd: number, events: number[]): void {
		this.pause();
		this.tEnd = tEnd;
		this.events = events;
		this.t = Math.min(this.t, tEnd);
	}

	seek(t: number): void {
		this.t = Math.max(0, Math.min(this.tEnd, t));
	}

	play(): void {
		if (this.playing || typeof requestAnimationFrame === 'undefined') return;
		if (this.t >= this.tEnd) this.t = 0;
		this.playing = true;
		this.#last = performance.now();
		const frame = (now: number) => {
			if (!this.playing) return;
			const dt = (now - this.#last) / 1000;
			this.#last = now;
			this.t = advance(this.t, dt, this.speed, this.events, this.slowAroundEvents, this.tEnd);
			if (this.t >= this.tEnd) {
				this.playing = false;
				return;
			}
			this.#raf = requestAnimationFrame(frame);
		};
		this.#raf = requestAnimationFrame(frame);
	}

	pause(): void {
		this.playing = false;
		if (typeof cancelAnimationFrame !== 'undefined') cancelAnimationFrame(this.#raf);
	}

	toggle(): void {
		if (this.playing) this.pause();
		else this.play();
	}

	/** The previous or next event marker from `t`. */
	jump(direction: -1 | 1): void {
		const sorted = [...this.events].sort((a, b) => a - b);
		const eps = 1e-6;
		const target =
			direction > 0
				? sorted.find((e) => e > this.t + eps)
				: [...sorted].reverse().find((e) => e < this.t - eps);
		if (target !== undefined) this.seek(target);
	}
}
