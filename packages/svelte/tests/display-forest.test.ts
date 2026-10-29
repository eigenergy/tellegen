import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { DistGraph } from '@tellegen/engine';
import { displayTree } from '../src/lib/diagram/index.js';

// The engine lays the schematic out on its own copy of this forest (`schematic.rs`), so the drawing,
// the keyboard walk and the voltage order agree only while both reproduce the same fixture.
const fixture = JSON.parse(
	readFileSync(
		new URL('../../../crates/tellegen/tests/data/schematic/display-forest.json', import.meta.url),
		'utf8'
	)
) as {
	graph: DistGraph;
	forest: { parent: number[]; parent_edge: number[]; depth: number[]; roots: number[] };
};

describe('display forest', () => {
	it('matches the forest the engine reproduces from the shared fixture', () => {
		const tree = displayTree(fixture.graph);
		expect({
			parent: [...tree.parent],
			parent_edge: [...tree.parentEdge],
			depth: [...tree.depth],
			roots: tree.roots
		}).toEqual(fixture.forest);
	});
});
