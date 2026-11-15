/** Keyboard behaviour of a tablist (WAI-ARIA tabs pattern, automatic
 * activation): arrows move and select, Home and End go to the ends. */

/** The tab index a key moves to from `i` of `n`, or null for other keys. */
export function tabKey(key: string, i: number, n: number): number | null {
	if (n <= 0) return null;
	switch (key) {
		case 'ArrowRight':
			return (i + 1) % n;
		case 'ArrowLeft':
			return (i - 1 + n) % n;
		case 'Home':
			return 0;
		case 'End':
			return n - 1;
		default:
			return null;
	}
}

/** Handle a keydown on a tablist element whose tabs are `ids`, in order. */
export function onTabKey<T>(
	e: KeyboardEvent,
	ids: readonly T[],
	current: T,
	select: (id: T) => void
): void {
	const j = tabKey(e.key, Math.max(0, ids.indexOf(current)), ids.length);
	if (j === null) return;
	e.preventDefault();
	select(ids[j]);
	const tabs = (e.currentTarget as HTMLElement | null)?.querySelectorAll<HTMLElement>(
		'[role="tab"]'
	);
	tabs?.[j]?.focus();
}
