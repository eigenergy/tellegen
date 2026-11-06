import {
	BrowserCaseCollection,
	MAX_COLLECTION_BYTES,
	MAX_COLLECTION_FILES,
	type CollectionCandidate,
	type CollectionInspection,
	type CollectionSelection,
	type ImportedCollectionCase
} from '@tellegen/engine';
import { validateCollectionFiles, type NamedUpload } from './collection-files.js';

export interface CollectionRow {
	id: number;
	session: number;
	label: string;
	path: string;
	selection: CollectionSelection;
	format: string;
	status: CollectionCandidate['status'];
	checked: boolean;
	outcome: 'pending' | 'importing' | 'imported' | 'view-only' | 'incomplete' | 'failed';
	message: string;
}
type Session = Pick<
	BrowserCaseCollection,
	'inspection' | 'ingest' | 'originalZip' | 'sourceZip' | 'free' | 'cancel'
>;
type Factory = {
	zip: (name: string, bytes: Uint8Array, signal: AbortSignal) => Promise<Session>;
	files: (
		name: string,
		files: { path: string; bytes: Uint8Array }[],
		signal: AbortSignal
	) => Promise<Session>;
};
const factory: Factory = {
	zip: (name, bytes, signal) => BrowserCaseCollection.fromZip(name, bytes, signal),
	files: (name, files, signal) => BrowserCaseCollection.fromFiles(name, files, signal)
};

/** One chooser owns one bounded batch. Imported cases own their modules; this
 * object owns only acquisition sessions and can release them independently. */
export class CollectionImport {
	open = $state(false);
	busy = $state(false);
	progress = $state('');
	errors = $state<string[]>([]);
	rows = $state<CollectionRow[]>([]);
	collections = $state<{ id: number; label: string; inspection: CollectionInspection }[]>([]);
	private sessions = new Map<number, Session>();
	private abort: AbortController | null = null;
	private generation = 0;
	private sequence = 0;
	private expanded = 0;
	private entries = 0;

	constructor(
		private readonly accept: (label: string, result: ImportedCollectionCase) => void,
		private readonly acquire: Factory = factory
	) {}

	async start(read: (signal: AbortSignal) => Promise<NamedUpload[]>): Promise<void> {
		if (this.open) throw new Error('Finish or close the current collection before opening another');
		this.open = true;
		this.busy = true;
		this.errors = [];
		this.rows = [];
		this.collections = [];
		this.expanded = 0;
		this.entries = 0;
		const generation = ++this.generation;
		const abort = (this.abort = new AbortController());
		this.progress = 'Reading collection files…';
		try {
			const files = await read(abort.signal);
			this.check(generation);
			validateCollectionFiles(files);
			const archives = files.filter((f) => !f.path.includes('/') && /\.zip$/i.test(f.path));
			if (archives.length > 32) throw new Error('Choose at most 32 ZIP uploads');
			const loose = files.filter((f) => !archives.includes(f));
			for (const [index, entry] of archives.entries()) {
				this.check(generation);
				const label = `${index + 1}. ${entry.path}`;
				this.progress = `Inspecting ${label}…`;
				try {
					const bytes = new Uint8Array(await entry.file.arrayBuffer());
					this.check(generation);
					await this.admit(
						label,
						await this.acquire.zip(entry.path, bytes, abort.signal),
						generation
					);
				} catch (error) {
					this.recordError(label, error, generation);
				}
			}
			if (loose.length) {
				try {
					const loaded: { path: string; bytes: Uint8Array }[] = [];
					for (const [index, entry] of loose.entries()) {
						this.check(generation);
						this.progress = `Reading ${entry.path} (${index + 1}/${loose.length})…`;
						loaded.push({
							path: entry.path,
							bytes: new Uint8Array(await entry.file.arrayBuffer())
						});
					}
					this.check(generation);
					this.progress = 'Inspecting folder/files…';
					await this.admit(
						'Folder/files',
						await this.acquire.files('Folder/files', loaded, abort.signal),
						generation
					);
				} catch (error) {
					this.recordError('Folder/files', error, generation);
				}
			}
			this.check(generation);
			this.progress = `${this.rows.length} candidates in ${this.entries} files. Choose cases to import.`;
		} catch (error) {
			this.recordError('Collection', error, generation);
		} finally {
			if (generation === this.generation) this.busy = false;
		}
	}

	private async admit(label: string, session: Session, generation: number): Promise<void> {
		const count = session.inspection.entries.length;
		const bytes = session.inspection.entries.reduce((n, entry) => n + entry.bytes, 0);
		if (
			generation !== this.generation ||
			this.expanded + bytes > MAX_COLLECTION_BYTES ||
			this.entries + count > MAX_COLLECTION_FILES
		) {
			await session.free();
			this.check(generation);
			throw new Error('Combined collections exceed 4096 files or 64 MiB expanded');
		}
		const id = ++this.sequence;
		this.sessions.set(id, session);
		this.expanded += bytes;
		this.entries += count;
		this.collections.push({ id, label, inspection: session.inspection });
		for (const candidate of session.inspection.candidates) this.addRow(id, label, candidate);
	}

	private addRow(session: number, label: string, candidate: CollectionCandidate) {
		const path =
			candidate.selection.primary ||
			candidate.selection.root ||
			candidate.selection.members?.join(' + ') ||
			'(selection)';
		this.rows.push({
			id: ++this.sequence,
			session,
			label: `${label} / ${path}`,
			path,
			selection: candidate.selection,
			format: candidate.selection.format || '',
			status: candidate.status,
			checked: false,
			outcome: 'pending',
			message: ''
		});
	}

	addExplicit(session: number, path: string, format: string) {
		if (this.busy || !this.open) return;
		const collection = this.collections.find((c) => c.id === session);
		if (!collection || !path || !format.trim()) return;
		if (this.rows.length >= MAX_COLLECTION_FILES) {
			this.errors.push('Candidate limit reached');
			return;
		}
		this.addRow(session, collection.label, {
			selection: { primary: path, format: format.trim() },
			status: 'selectable'
		});
	}

	groupSelected(format: 'cgmes' | 'goc3') {
		if (this.busy || !this.open) return;
		if (this.rows.length >= MAX_COLLECTION_FILES) {
			this.errors.push('Candidate limit reached');
			return;
		}
		const rows = this.rows.filter((r) => r.checked && r.outcome === 'pending');
		if (
			rows.length < 2 ||
			rows.some((r) => r.session !== rows[0].session || !r.selection.primary)
		) {
			this.errors.push('Choose at least two documents from the same collection to form a group.');
			return;
		}
		const collection = this.collections.find((c) => c.id === rows[0].session)!;
		this.addRow(collection.id, collection.label, {
			selection: { members: rows.map((r) => r.selection.primary!), format },
			status: 'selectable'
		});
		rows.forEach((r) => {
			r.checked = false;
		});
	}

	async importSelected(): Promise<void> {
		if (this.busy || !this.open) return;
		const selected = this.rows.filter(
			(r) =>
				r.checked &&
				r.outcome !== 'imported' &&
				r.outcome !== 'view-only' &&
				r.outcome !== 'incomplete'
		);
		if (!selected.length) return;
		if (selected.length > 32) {
			this.errors.push('Import at most 32 cases at a time.');
			return;
		}
		const generation = this.generation;
		this.busy = true;
		try {
			for (const [index, row] of selected.entries()) {
				this.check(generation);
				row.outcome = 'importing';
				row.message = '';
				this.progress = `Importing ${row.label} (${index + 1}/${selected.length})…`;
				try {
					const result = await this.sessions.get(row.session)!.ingest(this.selection(row));
					this.check(generation);
					this.accept(row.label, result);
					row.checked = false;
					const reason =
						result.kind === 'multiconductor' ? result.payload.mc_pf_unavailable_reason : null;
					row.outcome = result.incomplete ? 'incomplete' : reason ? 'view-only' : 'imported';
					row.message = result.incomplete
						? 'Missing dependencies; view-only. See case diagnostics.'
						: reason ||
							'Imported. Close this chooser to inspect the case and choose a calculation.';
				} catch (error) {
					this.check(generation);
					row.outcome = 'failed';
					row.message = error instanceof Error ? error.message : String(error);
				}
			}
			this.progress = 'Import finished. Successful cases are kept independently.';
		} catch (error) {
			this.recordError('Import', error, generation);
		} finally {
			if (generation === this.generation) this.busy = false;
		}
	}

	selection(row: CollectionRow): CollectionSelection {
		return { ...row.selection, format: row.format.trim() || null };
	}

	async download(
		session: number,
		row?: CollectionRow,
		scope: 'selected' | 'project' = 'selected'
	): Promise<Uint8Array> {
		if (this.busy || !this.open) throw new Error('Wait for collection work to finish');
		const owner = this.sessions.get(session);
		if (!owner) throw new Error('Collection is closed');
		this.busy = true;
		const generation = this.generation;
		try {
			const bytes = row
				? await owner.sourceZip(this.selection(row), scope)
				: await owner.originalZip();
			this.check(generation);
			return bytes;
		} finally {
			if (generation === this.generation) this.busy = false;
		}
	}

	close() {
		++this.generation;
		this.abort?.abort();
		this.abort = null;
		for (const session of this.sessions.values()) session.cancel();
		this.sessions.clear();
		this.open = false;
		this.busy = false;
		this.rows = [];
		this.collections = [];
	}

	private check(generation: number) {
		if (generation !== this.generation || this.abort?.signal.aborted)
			throw new DOMException('Import cancelled', 'AbortError');
	}
	private recordError(label: string, error: unknown, generation: number) {
		if (generation !== this.generation || this.abort?.signal.aborted) return;
		this.errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`);
	}
}
