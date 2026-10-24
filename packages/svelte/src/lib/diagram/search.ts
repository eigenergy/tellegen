/** One search over buses, branches and attachments by source identity. Linear scan, no fuzzy
 * guessing: an engineer searching for `LV12` must not be offered `LV21`. */
export type EntityKind = 'bus' | 'branch' | 'attachment';

export interface SearchEntity {
	kind: EntityKind;
	index: number;
	id: string;
	/** `bus`, or the equipment family such as `line` or `transformer / 2w`. */
	family: string;
	/** Source label when a drawing combines several networks. */
	source?: string;
	/** `source/id` when a drawing combines several networks; equal to `id` otherwise. */
	qualified: string;
}

export interface SearchIndex {
	entries: (SearchEntity & { idLower: string; qualifiedLower: string })[];
	byKey: Map<string, SearchEntity>;
}

export interface SearchMatch extends SearchEntity {
	exact: boolean;
}

const KIND_ORDER: Record<EntityKind, number> = { bus: 0, branch: 1, attachment: 2 };

/** Readable URL identity, e.g. `branch:line:feeder/L1`, unique even when raw IDs repeat. */
export function entityKey(e: Pick<SearchEntity, 'kind' | 'family' | 'qualified'>): string {
	const family = e.kind === 'bus' ? '' : `${e.family.replace(' / ', '.')}:`;
	return `${e.kind}:${family}${e.qualified}`;
}

export function buildSearchIndex(entities: SearchEntity[]): SearchIndex {
	const entries = entities.map((e) => ({
		...e,
		idLower: e.id.toLowerCase(),
		qualifiedLower: e.qualified.toLowerCase()
	}));
	return { entries, byKey: new Map(entities.map((e) => [entityKey(e), e])) };
}

/** Rank: exact ID, exact qualified ID, ID prefix, then substring; ties prefer buses, shorter IDs,
 * then network order. */
export function search(
	index: SearchIndex,
	query: string,
	{ limit = 8, kinds = null as EntityKind[] | null } = {}
): { matches: SearchMatch[]; total: number } {
	const q = query.trim().toLowerCase();
	if (!q) return { matches: [], total: 0 };
	const hits: { rank: number; e: SearchIndex['entries'][number] }[] = [];
	for (const e of index.entries) {
		if (kinds && !kinds.includes(e.kind)) continue;
		const rank =
			e.idLower === q
				? 0
				: e.qualifiedLower === q
					? 1
					: e.idLower.startsWith(q)
						? 2
						: e.idLower.includes(q) || e.qualifiedLower.includes(q)
							? 3
							: -1;
		if (rank >= 0) hits.push({ rank, e });
	}
	hits.sort(
		(a, b) =>
			a.rank - b.rank ||
			KIND_ORDER[a.e.kind] - KIND_ORDER[b.e.kind] ||
			a.e.id.length - b.e.id.length ||
			a.e.index - b.e.index
	);
	return {
		matches: hits.slice(0, limit).map(({ rank, e }) => {
			const { idLower: _i, qualifiedLower: _q, ...entity } = e;
			return { ...entity, exact: rank <= 1 };
		}),
		total: hits.length
	};
}
