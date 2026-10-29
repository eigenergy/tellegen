/** The distinctive core of each id: parts (split at `_ - . : /` and spaces) that every id in the
 * set shares at its start or end are dropped, so `75_MVLV060_Transformer` among other
 * `75_…_Transformer` ids reads `MVLV060`. A single id, or a set where dropping would leave an id
 * empty or make two equal, keeps its ids whole. */
export function distinctiveCores(ids: string[]): string[] {
	if (ids.length < 2) return [...ids];
	const parts = ids.map((id) => [...id.matchAll(/[^_\-.:/\s]+/g)]);
	const shortest = Math.min(...parts.map((p) => p.length));
	const same = (pick: (p: RegExpMatchArray[]) => RegExpMatchArray | undefined) => {
		const first = pick(parts[0])?.[0];
		return first !== undefined && parts.every((p) => pick(p)?.[0] === first);
	};
	let lead = 0;
	while (lead < shortest - 1 && same((p) => p[lead])) lead++;
	let trail = 0;
	while (lead + trail < shortest - 1 && same((p) => p[p.length - 1 - trail])) trail++;
	if (!lead && !trail) return [...ids];
	const cores = ids.map((id, i) => {
		const p = parts[i];
		const start = p[lead].index!;
		const last = p[p.length - 1 - trail];
		return id.slice(start, last.index! + last[0].length);
	});
	return new Set(cores).size === cores.length ? cores : [...ids];
}

/** Short display names for long ids such as `lv_extension_02ad4cfa5ebe_0062383670bceb3c28d5`.
 * Parts every id shares at its start or end are dropped first ({@link distinctiveCores}); cores up
 * to `max` characters are kept, longer ones keep their head and tail around an ellipsis, with the
 * tail lengthened past `max` only where needed to keep every short name unique. The full id stays
 * in tooltips and panels. */
export function shortNames(ids: string[], max = 20): string[] {
	const cores = distinctiveCores(ids);
	const out = [...cores];
	const long = cores.flatMap((id, i) => (id.length > max ? [i] : []));
	const head = max - 6;
	let tail = 5;
	let pending = long;
	while (pending.length) {
		for (const i of pending) {
			const id = cores[i];
			out[i] = id.length <= head + tail + 1 ? id : `${id.slice(0, head)}…${id.slice(-tail)}`;
		}
		const seen = new Map<string, number>();
		for (const name of out) seen.set(name, (seen.get(name) ?? 0) + 1);
		pending = pending.filter((i) => seen.get(out[i])! > 1 && out[i] !== cores[i]);
		tail += 3;
	}
	return out;
}
