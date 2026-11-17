import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IDBFactory, IDBObjectStore } from 'fake-indexeddb';
import { IndexedDbStudyStore } from './study-document.js';
import type { McStudySnapshot, StudyBundle } from './index.js';

beforeEach(() => { globalThis.indexedDB = new IDBFactory(); });
const snapshot = { id: 'distribution-1', title: 'Feeder', input_module: 'original', options: {}, result: {} } as McStudySnapshot;
const bundle = { document: { id: 'tx-1', title: 'Transmission', revision: 0 }, artifacts: { input: { text: 'original' } } } as unknown as StudyBundle;
async function legacy() {
  return new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('tellegen-multiconductor-studies-v1', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('snapshots', { keyPath: 'id' }).add(snapshot);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

describe('saved study storage', () => {
  it('migrates once, preserves old saves, and does not resurrect a deleted save', async () => {
    const old = await legacy();
    const store = new IndexedDbStudyStore();
    await store.create(bundle);
    expect(await store.listDistribution()).toEqual([snapshot]);
    expect(await store.load('tx-1')).toEqual(bundle);
    await store.rename(snapshot.id, 'Renamed', 'distribution');
    await store.migrateDistribution();
    expect((await store.loadDistribution(snapshot.id))?.title).toBe('Renamed');
    await store.delete(snapshot.id, 'distribution');
    await store.close();
    const reopened = new IndexedDbStudyStore();
    expect(await reopened.listDistribution()).toEqual([]);
    const source = await new Promise(resolve => {
      const request = old.transaction('snapshots').objectStore('snapshots').get(snapshot.id);
      request.onsuccess = () => resolve(request.result);
    });
    expect(source).toEqual(snapshot);
    old.close(); await reopened.close();
  });
  it('keeps migration retryable when the destination write fails', async () => {
    const old = await legacy(); old.close();
    const store = new IndexedDbStudyStore();
    const original = IDBObjectStore.prototype.add;
    const add = vi.spyOn(IDBObjectStore.prototype, 'add').mockImplementation(function (this: IDBObjectStore, ...args) {
      if (this.name === 'distribution') throw new DOMException('Disk full', 'QuotaExceededError');
      return original.apply(this, args);
    });
    await expect(store.listDistribution()).rejects.toThrow('Free browser storage');
    add.mockRestore();
    expect(await store.listDistribution()).toEqual([snapshot]);
    await store.close();
  });
  it('renames atomically, preserves artifacts, and rejects stale commits', async () => {
    const store = new IndexedDbStudyStore();
    await store.create(bundle);
    await store.rename(bundle.document.id, '  New title  ');
    expect(await store.load(bundle.document.id)).toEqual({ ...bundle, document: { ...bundle.document, title: 'New title', revision: 1 } });
    await expect(store.commit(0, { ...bundle, document: { ...bundle.document, revision: 1 } })).rejects.toThrow('revision changed');
    await expect(store.rename(bundle.document.id, ' ')).rejects.toThrow('Enter a study name');
    await store.delete(bundle.document.id);
    expect(await store.load(bundle.document.id)).toBeNull();
    await expect(store.rename(bundle.document.id, 'Gone')).rejects.toThrow('not found');
    await store.close();
  });
  it('does not restore a deleted save after another tab finishes a delayed legacy read', async () => {
    const old = await legacy(); old.close();
    const slow = new IndexedDbStudyStore();
    const fast = new IndexedDbStudyStore();
    let release!: () => void;
    let readStarted!: () => void;
    const started = new Promise<void>(resolve => { readStarted = resolve; });
    const original = IDBObjectStore.prototype.getAll;
    let delay = true;
    const getAll = vi.spyOn(IDBObjectStore.prototype, 'getAll').mockImplementation(function (this: IDBObjectStore, ...args) {
      if (this.name !== 'snapshots' || !delay) return original.apply(this, args);
      delay = false;
      const held = { result: [snapshot], onsuccess: null, onerror: null } as unknown as IDBRequest<unknown[]>;
      release = () => held.onsuccess?.call(held, new Event('success'));
      readStarted();
      return held;
    });
    try {
      const pending = slow.listDistribution();
      await started;
      expect(await fast.listDistribution()).toEqual([snapshot]);
      await fast.delete(snapshot.id, 'distribution');
      release();
      expect(await pending).toEqual([]);
      expect(await fast.listDistribution()).toEqual([]);
    } finally {
      getAll.mockRestore();
      await slow.close(); await fast.close();
    }
  });
  it('does not overwrite an existing distribution save and supports concurrent first reads', async () => {
    const old = await legacy(); old.close();
    const store = new IndexedDbStudyStore();
    const rows = await Promise.all([store.listDistribution(), store.listDistribution()]);
    expect(rows).toEqual([[snapshot], [snapshot]]);
    await expect(store.saveDistribution({ ...snapshot, title: 'overwrite' })).rejects.toThrow();
    expect(await store.loadDistribution(snapshot.id)).toEqual(snapshot);
    await store.close();
  });
});
