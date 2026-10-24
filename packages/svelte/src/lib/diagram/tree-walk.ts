import type { DistGraph } from '@tellegen/engine';
import type { DisplayTree } from './display-tree.js';
import { ATTACHMENT_NAMES, ATTACHMENT_ORDER, neutralEarths } from './schematic-equipment.js';

export type WalkKey = 'parent' | 'child' | 'previous' | 'next' | 'root';

/** Keyboard navigation over the display tree, ordered as drawn: siblings and roots by their
 * position (top to bottom, then left to right), so Up and Down move as the eye does. */
export interface TreeWalk {
	step(from: number, key: WalkKey): number;
	children: number[][];
}

export function treeWalk(tree: DisplayTree, positions: ArrayLike<number>, stride = 3): TreeWalk {
	const n = tree.parent.length;
	const y = (i: number) => positions[i * stride + 1];
	const x = (i: number) => positions[i * stride];
	const order = (a: number, b: number) => y(a) - y(b) || x(a) - x(b) || a - b;
	const children: number[][] = Array.from({ length: n }, () => []);
	for (let i = 0; i < n; i++) if (tree.parent[i] !== i) children[tree.parent[i]].push(i);
	for (const list of children) list.sort(order);
	const roots = [...tree.roots].sort(order);
	const siblings = (i: number) => (tree.parent[i] === i ? roots : children[tree.parent[i]]);
	return {
		children,
		step(from, key) {
			if (from < 0 || from >= n) return roots[0] ?? -1;
			switch (key) {
				case 'parent':
					return tree.parent[from];
				case 'child': {
					// Straight ahead: the child drawn nearest this bus's row.
					const kids = children[from];
					if (!kids.length) return from;
					return kids.reduce((best, k) =>
						Math.abs(y(k) - y(from)) < Math.abs(y(best) - y(from)) ? k : best
					);
				}
				case 'previous':
				case 'next': {
					const list = siblings(from);
					const at = list.indexOf(from);
					const to = at + (key === 'next' ? 1 : -1);
					return to >= 0 && to < list.length ? list[to] : from;
				}
				case 'root': {
					let i = from;
					while (tree.parent[i] !== i) i = tree.parent[i];
					return i;
				}
			}
		}
	};
}

/** A spoken description of bus `i` for the keyboard cursor: where it sits in the tree, what hangs
 * off it, and an optional result note (for example the worst voltage against its band). */
export function describeBus(
	graph: DistGraph,
	tree: DisplayTree,
	walk: TreeWalk,
	i: number,
	note?: string | null
): string {
	const bus = graph.buses[i];
	let root = i;
	while (tree.parent[root] !== root) root = tree.parent[root];
	const depth = tree.depth[i];
	const where =
		depth === 0
			? bus.has_source
				? 'source bus'
				: 'not reached from a source'
			: `${depth} step${depth === 1 ? '' : 's'} from ${graph.buses[root].id}`;
	const via =
		tree.parentEdge[i] >= 0
			? `, fed by ${graph.edges[tree.parentEdge[i]].kind} ${graph.edges[tree.parentEdge[i]].id}`
			: '';
	const onward = walk.children[i].length;
	const counts = new Map<string, Set<string>>();
	const earths = neutralEarths(bus);
	for (const list of Object.values(bus.terminal_attachments ?? {}))
		for (const a of list) {
			const kind = a.kind === 'shunt' && earths.has(a.id) ? 'earth' : a.kind;
			counts.set(kind, (counts.get(kind) ?? new Set()).add(a.id));
		}
	if (bus.grounded?.length) counts.set('ground', new Set(bus.grounded));
	const attached = ATTACHMENT_ORDER.filter((k) => counts.has(k)).map((k) => {
		const c = counts.get(k)!.size;
		return `${c} ${ATTACHMENT_NAMES[k]}${c === 1 ? '' : 's'}`;
	});
	return [
		`${bus.id}: ${where}${via}`,
		`${onward} branch${onward === 1 ? '' : 'es'} onward`,
		...(attached.length ? [attached.join(', ')] : []),
		...(note ? [note] : [])
	].join('. ');
}
