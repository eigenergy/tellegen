/** Smallest demand or rating edit that counts: a slider released closer than
 * this to the committed value commits nothing and starts no solve. */
export const DEAD_ZONE_MW = 0.25;

const KEY_DIRECTIONS: Record<string, number> = {
	ArrowRight: 1,
	ArrowUp: 1,
	ArrowLeft: -1,
	ArrowDown: -1
};

/** Arrow-key step of a demand or rating slider spanning `span` MW: about 1%
 * of the span, rounded up to 1, 2 or 5 times a power of ten. It is at least
 * twice the dead zone, so one press always leaves it, rounding error
 * included. The native step of a `step="any"` range is 1% of the span, which
 * on a small bus never leaves the dead zone. */
export function sliderKeyStep(span: number): number {
	const target = Math.max(span / 100, 2 * DEAD_ZONE_MW);
	const power = 10 ** Math.floor(Math.log10(target));
	for (const factor of [1, 2, 5]) {
		if (factor * power >= target) return factor * power;
	}
	return 10 * power;
}

/** The value an arrow key moves a slider to, kept within [min, max], or null
 * for any other key. Right and up raise it. */
export function sliderKeyValue(
	key: string,
	value: number,
	min: number,
	max: number
): number | null {
	const direction = KEY_DIRECTIONS[key];
	if (direction === undefined) return null;
	return Math.min(max, Math.max(min, value + direction * sliderKeyStep(max - min)));
}
