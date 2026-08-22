import { describe, expect, it } from 'vitest';
import {
	DropBatchGate,
	MAX_DROP_FILES,
	MAX_DROP_FILE_BYTES,
	MAX_DROP_TOTAL_BYTES,
	readDropFileBytes,
	validateDropBatch
} from '../src/lib/drop-limits.js';

type FileDescriptor = {
	name: string;
	size: number;
	arrayBuffer?: () => Promise<ArrayBuffer>;
};

function file(name: string, size: number): FileDescriptor {
	return { name, size };
}

describe('validateDropBatch', () => {
	it('accepts the exact count, per-file, and aggregate boundaries', () => {
		expect(
			validateDropBatch(Array.from({ length: MAX_DROP_FILES }, (_, i) => file(`${i}.m`, 0)))
		).toHaveLength(MAX_DROP_FILES);
		expect(validateDropBatch([file('full.m', MAX_DROP_FILE_BYTES)])[0]).toMatchObject({
			size: MAX_DROP_FILE_BYTES
		});
		expect(
			validateDropBatch([
				file('a.m', MAX_DROP_TOTAL_BYTES / 2),
				file('b.m', MAX_DROP_TOTAL_BYTES / 2)
			])
		).toHaveLength(2);
	});

	it('rejects one past every limit', () => {
		expect(() =>
			validateDropBatch(Array.from({ length: MAX_DROP_FILES + 1 }, (_, i) => file(`${i}.m`, 0)))
		).toThrow(`at most ${MAX_DROP_FILES}`);
		expect(() => validateDropBatch([file('large.m', MAX_DROP_FILE_BYTES + 1)])).toThrow(
			'per-file limit'
		);
		expect(() =>
			validateDropBatch([
				file('a.m', MAX_DROP_TOTAL_BYTES / 2),
				file('b.m', MAX_DROP_TOTAL_BYTES / 2 + 1)
			])
		).toThrow('total limit');
	});

	it.each([NaN, Infinity, -1, 1.5, Number.MAX_SAFE_INTEGER + 1])(
		'rejects invalid declared length %s without touching an entry',
		(length) => {
			let entriesRead = 0;
			const input = {
				length,
				get 0() {
					entriesRead++;
					return file('never.m', 1);
				}
			};
			expect(() => validateDropBatch(input as ArrayLike<FileDescriptor>)).toThrow('invalid');
			expect(entriesRead).toBe(0);
		}
	);

	it.each([NaN, Infinity, -1, 0.5, Number.MAX_SAFE_INTEGER + 1])(
		'rejects invalid declared file size %s',
		(size) => {
			expect(() => validateDropBatch([file('invalid.m', size)])).toThrow('invalid file size');
		}
	);

	it('validates the whole batch before a caller can read the first file', () => {
		let contentReads = 0;
		const first = {
			...file('first.m', 1),
			async arrayBuffer() {
				contentReads++;
				return new ArrayBuffer(1);
			}
		};
		expect(() => validateDropBatch([first, file('later.m', MAX_DROP_FILE_BYTES + 1)])).toThrow(
			'per-file limit'
		);
		expect(contentReads).toBe(0);
	});

	it('rejects sparse or throwing array-like entries', () => {
		expect(() => validateDropBatch({ length: 1 } as unknown as ArrayLike<FileDescriptor>)).toThrow(
			'file 1 is invalid'
		);
		const input = {
			length: 1,
			get 0(): FileDescriptor {
				throw new Error('entry getter failed');
			}
		};
		expect(() => validateDropBatch(input as ArrayLike<FileDescriptor>)).toThrow(
			'file 1 is invalid'
		);
	});
});

describe('DropBatchGate', () => {
	it('refuses overlap until the active batch releases', () => {
		const gate = new DropBatchGate();
		const release = gate.enter();
		expect(release).not.toBeNull();
		expect(gate.enter()).toBeNull();

		release?.();
		const nextRelease = gate.enter();
		expect(nextRelease).not.toBeNull();
		nextRelease?.();
	});

	it('makes a release callback idempotent', () => {
		const gate = new DropBatchGate();
		const release = gate.enter();
		release?.();
		const nextRelease = gate.enter();
		release?.();
		expect(gate.enter()).toBeNull();
		nextRelease?.();
	});
});

describe('readDropFileBytes', () => {
	it('reuses retained routing bytes without reading the file again', async () => {
		let reads = 0;
		const retained = new Uint8Array([1, 2, 3]);
		const result = await readDropFileBytes(
			{
				async arrayBuffer() {
					reads++;
					return new ArrayBuffer(3);
				}
			},
			retained
		);
		expect(result).toBe(retained);
		expect(reads).toBe(0);
	});

	it('reads once when no routing buffer was retained', async () => {
		let reads = 0;
		const result = await readDropFileBytes({
			async arrayBuffer() {
				reads++;
				return Uint8Array.from([4, 5]).buffer;
			}
		});
		expect([...result]).toEqual([4, 5]);
		expect(reads).toBe(1);
	});
});
