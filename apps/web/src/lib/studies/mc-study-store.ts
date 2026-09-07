import type { McStudySnapshot } from '@tellegen/engine';

const MC_STUDIES_DB = 'tellegen-multiconductor-studies-v1';
const MC_STUDIES_STORE = 'snapshots';

export class McStudyStore {
	#db: Promise<IDBDatabase> | null = null;
	private db(): Promise<IDBDatabase> {
		if (typeof indexedDB === 'undefined')
			throw new Error('Saved distribution studies require IndexedDB');
		return (this.#db ??= new Promise((resolve, reject) => {
			const request = indexedDB.open(MC_STUDIES_DB, 1);
			request.onupgradeneeded = () =>
				request.result.createObjectStore(MC_STUDIES_STORE, { keyPath: 'id' });
			request.onsuccess = () => resolve(request.result);
			request.onerror = () =>
				reject(request.error ?? new Error('Unable to open saved distribution studies'));
		}));
	}
	async list(): Promise<McStudySnapshot[]> {
		const db = await this.db();
		return new Promise((resolve, reject) => {
			const request = db
				.transaction(MC_STUDIES_STORE, 'readonly')
				.objectStore(MC_STUDIES_STORE)
				.getAll();
			request.onsuccess = () => resolve(request.result as McStudySnapshot[]);
			request.onerror = () =>
				reject(request.error ?? new Error('Unable to list saved distribution studies'));
		});
	}
	async get(id: string): Promise<McStudySnapshot | null> {
		const db = await this.db();
		return new Promise((resolve, reject) => {
			const request = db
				.transaction(MC_STUDIES_STORE, 'readonly')
				.objectStore(MC_STUDIES_STORE)
				.get(id);
			request.onsuccess = () => resolve((request.result as McStudySnapshot | undefined) ?? null);
			request.onerror = () =>
				reject(request.error ?? new Error('Unable to open saved distribution study'));
		});
	}
	async put(snapshot: McStudySnapshot): Promise<void> {
		const db = await this.db();
		return new Promise((resolve, reject) => {
			const transaction = db.transaction(MC_STUDIES_STORE, 'readwrite');
			transaction.objectStore(MC_STUDIES_STORE).add(snapshot);
			transaction.oncomplete = () => resolve();
			transaction.onerror = () =>
				reject(
					new Error(
						transaction.error?.name === 'QuotaExceededError'
							? 'Browser storage is full. Export saved studies and free browser storage, then save again.'
							: transaction.error?.name === 'ConstraintError'
								? 'This study is already saved. Open it from Saved study.'
								: 'Unable to save distribution study. Try again.'
					)
				);
			transaction.onabort = () =>
				reject(
					new Error(
						transaction.error?.name === 'QuotaExceededError'
							? 'Browser storage is full. Export saved studies and free browser storage, then save again.'
							: transaction.error?.name === 'ConstraintError'
								? 'This study is already saved. Open it from Saved study.'
								: 'Unable to save distribution study. Try again.'
					)
				);
		});
	}
}
