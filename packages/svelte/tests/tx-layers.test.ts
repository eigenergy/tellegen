import { describe, expect, it } from 'vitest';
import { raisedSegments } from '../src/lib/components/schematic/tx-layers.js';
import type { SchematicGeometry } from '../src/lib/diagram/index.js';

describe('raised branches', () => {
	// Branch 0 has two segments, branch 1 one, branch 2 two.
	const geometry = {
		segmentEdge: Int32Array.from([0, 0, 1, 2, 2])
	} as unknown as SchematicGeometry;

	it('selects every segment of each flagged branch, in drawing order', () => {
		expect(Array.from(raisedSegments(geometry, Uint8Array.from([0, 1, 1]))!)).toEqual([2, 3, 4]);
	});

	it('returns null when no branch is flagged', () => {
		expect(raisedSegments(geometry, new Uint8Array(3))).toBeNull();
	});
});
