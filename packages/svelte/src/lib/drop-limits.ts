/** Browser drop limits. Each accepted file is copied into JavaScript and worker/wasm
 * memory, so the whole batch is checked before any file contents are read. */
export const MAX_DROP_FILES = 32;
export const MAX_DROP_FILE_BYTES = 128 * 1024 * 1024;
export const MAX_DROP_TOTAL_BYTES = 128 * 1024 * 1024;

type DropFileDescriptor = {
	readonly name: string;
	readonly size: number;
};

/** One-at-a-time admission for browser ingestion. A second drop is refused
 * while the first still owns its release callback, so independently valid
 * batches cannot queue enough worker copies to bypass the aggregate limit. */
export class DropBatchGate {
	#active = false;

	enter(): (() => void) | null {
		if (this.#active) return null;
		this.#active = true;
		let released = false;
		return () => {
			if (released) return;
			released = true;
			this.#active = false;
		};
	}
}

/** Return bytes retained by an earlier routing pass, or read the file once when
 * no retained buffer exists. Keeping this decision here makes the no-reread
 * guarantee independently testable. */
export async function readDropFileBytes(
	file: { arrayBuffer(): Promise<ArrayBuffer> },
	retained?: Uint8Array
): Promise<Uint8Array> {
	return retained ?? new Uint8Array(await file.arrayBuffer());
}

/** Validate and materialize an array-like browser file batch without calling
 * `Array.from` first. The count and every declared byte size must be safe integers,
 * and aggregate addition is guarded before it is performed. */
export function validateDropBatch<T extends DropFileDescriptor>(input: ArrayLike<T>): T[] {
	let length: unknown;
	try {
		length = input.length;
	} catch {
		throw new Error('the dropped file count is invalid');
	}
	if (typeof length !== 'number' || !Number.isSafeInteger(length) || length < 0) {
		throw new Error('the dropped file count is invalid');
	}
	if (length > MAX_DROP_FILES) {
		throw new Error(`drop at most ${MAX_DROP_FILES} files at a time`);
	}

	const files: T[] = [];
	let totalBytes = 0;
	for (let index = 0; index < length; index++) {
		let file: T | undefined;
		try {
			file = input[index];
		} catch {
			throw new Error(`dropped file ${index + 1} is invalid`);
		}
		if (!file || typeof file !== 'object') {
			throw new Error(`dropped file ${index + 1} is invalid`);
		}

		let size: unknown;
		let name: unknown;
		try {
			size = file.size;
			name = file.name;
		} catch {
			throw new Error(`dropped file ${index + 1} is invalid`);
		}
		const label = typeof name === 'string' && name.length > 0 ? name : `dropped file ${index + 1}`;
		if (typeof size !== 'number' || !Number.isSafeInteger(size) || size < 0) {
			throw new Error(`${label} has an invalid file size`);
		}
		if (size > MAX_DROP_FILE_BYTES) {
			throw new Error(`${label} exceeds the ${MAX_DROP_FILE_BYTES}-byte per-file limit`);
		}
		if (size > MAX_DROP_TOTAL_BYTES - totalBytes) {
			throw new Error(`the dropped files exceed the ${MAX_DROP_TOTAL_BYTES}-byte total limit`);
		}
		totalBytes += size;
		files.push(file);
	}
	return files;
}
