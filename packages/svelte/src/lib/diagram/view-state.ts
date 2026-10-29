/** Shareable view state. Selections travel as entity keys (source identities), never array offsets. */
export interface CameraState {
	target: [number, number, number];
	zoom: number;
}

export function encodeCamera(c: { target: ArrayLike<number>; zoom: number }): string {
	return [c.target[0].toFixed(1), c.target[1].toFixed(1), c.zoom.toFixed(2)].join(',');
}

export function decodeCamera(text: string | null | undefined): CameraState | null {
	const parts = String(text ?? '')
		.split(',')
		.map(Number);
	return parts.length === 3 && parts.every(Number.isFinite) && parts[2] >= -24 && parts[2] <= 5
		? { target: [parts[0], parts[1], 0], zoom: parts[2] }
		: null;
}

/** Merge state into a URL without dropping unrelated parameters; `null` or `''` removes a key. */
export function withView(
	href: string | URL,
	state: Record<string, string | number | null | undefined>
): URL {
	const url = new URL(href);
	for (const [k, v] of Object.entries(state)) {
		if (v === null || v === undefined || v === '') url.searchParams.delete(k);
		else url.searchParams.set(k, String(v));
	}
	return url;
}
