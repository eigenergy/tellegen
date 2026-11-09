/** Browser file acquisition only. PowerIO resolves formats and dependencies. */
import { MAX_COLLECTION_BYTES, MAX_COLLECTION_FILES } from '@tellegen/engine';

export interface NamedUpload {
	path: string;
	file: File;
}
export function validateCollectionFiles(files: readonly NamedUpload[]): void {
	if (files.length > MAX_COLLECTION_FILES) throw new Error('Choose at most 4096 collection files');
	let bytes = 0;
	for (const { path, file } of files) {
		if (!path || new TextEncoder().encode(path).length > 4096 || path.split('/').length > 64)
			throw new Error('A collection path exceeds its length or depth limit');
		if (!Number.isSafeInteger(file.size) || file.size < 0)
			throw new Error('Invalid collection file size');
		bytes += file.size;
		if (bytes > MAX_COLLECTION_BYTES) throw new Error('Choose a collection of at most 64 MiB');
	}
}

export function namedFiles(files: ArrayLike<File>): NamedUpload[] {
	if (
		!Number.isSafeInteger(files.length) ||
		files.length < 0 ||
		files.length > MAX_COLLECTION_FILES
	)
		throw new Error('Choose at most 4096 collection files');
	const named = Array.from(files, (file) => ({ path: file.webkitRelativePath || file.name, file }));
	validateCollectionFiles(named);
	return named;
}

/** Capture entry handles synchronously during drop; DataTransfer is protected
 * after the event. Read directory batches until empty (Chromium pages them). */
export function captureDrop(data: DataTransfer): (signal: AbortSignal) => Promise<NamedUpload[]> {
	const items = Array.from(data.items);
	const entries = items
		.filter((item) => item.kind === 'file')
		.map((item) => item.webkitGetAsEntry?.());
	const fallback = Array.from(data.files);
	if (!entries.some((entry) => entry?.isDirectory)) return async () => namedFiles(fallback);
	if (entries.some((entry) => !entry))
		throw new Error('This browser could not read the complete folder; use Open folder');
	return async (signal) => {
		const files: NamedUpload[] = [];
		let count = 0;
		const check = () => {
			if (signal.aborted) throw new DOMException('Import cancelled', 'AbortError');
		};
		const walk = async (entry: FileSystemEntry, parent: string, depth: number): Promise<void> => {
			check();
			if (++count > MAX_COLLECTION_FILES || depth > 64)
				throw new Error('Folder exceeds 4096 entries or 64 levels');
			const path = parent ? `${parent}/${entry.name}` : entry.name;
			if (entry.isFile) {
				const file = await new Promise<File>((resolve, reject) =>
					(entry as FileSystemFileEntry).file(resolve, reject)
				);
				check();
				files.push({ path, file });
				validateCollectionFiles(files);
			} else if (entry.isDirectory) {
				const reader = (entry as FileSystemDirectoryEntry).createReader();
				for (;;) {
					check();
					const batch = await new Promise<FileSystemEntry[]>((resolve, reject) =>
						reader.readEntries(resolve, reject)
					);
					if (!batch.length) break;
					for (const child of batch) await walk(child, path, depth + 1);
				}
			} else throw new Error(`Cannot read folder entry ${path}`);
		};
		for (const entry of entries) await walk(entry!, '', 1);
		return files;
	};
}
