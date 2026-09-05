import { isolatedEngineHost, type EngineHost } from "./host.js";
import type { CreateStudy, StudyBundle, StudyRequest, StudyOperationResult } from "./generated/study-contracts.js";

export type { CreateStudy, StudyBundle, StudyDocument, StudyRequest, StudyOperation, StudyOperationResult, GoalRevision, DecisionSpace, StudyObjective, StateNode, Comparison } from "./generated/study-contracts.js";

export interface StudyBackend {
  create(request: CreateStudy, signal?: AbortSignal): Promise<StudyBundle>;
  validate(text: string): Promise<StudyBundle>;
  execute(bundle: StudyBundle, request: StudyRequest, signal?: AbortSignal): Promise<{ bundle: StudyBundle; result: StudyOperationResult }>;
}

async function callIsolated(request: Parameters<EngineHost["call"]>[0], signal?: AbortSignal): Promise<string> {
  signal?.throwIfAborted();
  const host = isolatedEngineHost();
  const cooperative = request.op === "study_document_run";
  const cancel = () => cooperative ? host.requestStop?.() : host.cancel?.(new DOMException("Study operation cancelled", "AbortError"));
  signal?.addEventListener("abort", cancel, { once: true });
  try {
    const result = await host.call(request);
    if (!cooperative) signal?.throwIfAborted();
    if (result === null) throw new Error("The engine returned no Study result");
    return result;
  } finally {
    signal?.removeEventListener("abort", cancel);
    host.cancel?.(new Error("Study operation completed"));
  }
}

export const studyBackend: StudyBackend = {
  async create(request, signal) { return JSON.parse(await callIsolated({ op: "study_document_create", request: JSON.stringify(request) }, signal)) as StudyBundle; },
  async validate(text) { return JSON.parse(await callIsolated({ op: "study_document_import", bundle: text })) as StudyBundle; },
  async execute(bundle, request, signal) {
    return JSON.parse(await callIsolated({ op: "study_document_run", bundle: JSON.stringify(bundle), request: JSON.stringify(request) }, signal)) as { bundle: StudyBundle; result: StudyOperationResult };
  },
};

export interface StudyStore {
  load(id: string): Promise<StudyBundle | null>;
  create(bundle: StudyBundle): Promise<void>;
  commit(expectedRevision: number, bundle: StudyBundle): Promise<void>;
}

function storageError(error: unknown): Error {
  const cause = error instanceof Error ? error.message : String(error);
  return new Error(`Study was not saved: ${cause}. Free browser storage or export the Study and choose another destination before retrying.`);
}

/** IndexedDB commits the complete document and its deduplicated artifact map together. */
export class IndexedDbStudyStore implements StudyStore {
  #database: Promise<IDBDatabase>;
  constructor(name = "tellegen-studies") {
    this.#database = new Promise((resolve, reject) => {
      const request = indexedDB.open(name, 1);
      request.onupgradeneeded = () => request.result.createObjectStore("studies", { keyPath: "document.id" });
      request.onsuccess = () => { request.result.onversionchange = () => request.result.close(); resolve(request.result); };
      request.onerror = () => reject(storageError(request.error));
      request.onblocked = () => reject(new Error("Study storage is blocked by another tab; close its connection and reload"));
    });
  }
  async load(id: string): Promise<StudyBundle | null> {
    const db = await this.#database;
    return new Promise((resolve, reject) => {
      const tx = db.transaction("studies", "readonly");
      const request = tx.objectStore("studies").get(id);
      request.onsuccess = () => resolve((request.result as StudyBundle | undefined) ?? null);
      request.onerror = () => reject(storageError(request.error));
    });
  }
  async list(): Promise<Array<{ id: string; title: string; revision: number }>> {
    const db = await this.#database;
    return new Promise((resolve, reject) => {
      const rows: Array<{ id: string; title: string; revision: number }> = [];
      const tx = db.transaction("studies", "readonly");
      const request = tx.objectStore("studies").openCursor();
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor) return;
        const { id, title, revision } = (cursor.value as StudyBundle).document;
        rows.push({ id, title, revision }); cursor.continue();
      };
      tx.oncomplete = () => resolve(rows);
      tx.onabort = tx.onerror = () => reject(storageError(tx.error));
    });
  }
  async create(bundle: StudyBundle): Promise<void> { return this.#write(null, bundle); }
  async commit(expectedRevision: number, bundle: StudyBundle): Promise<void> { return this.#write(expectedRevision, bundle); }
  async #write(expected: number | null, bundle: StudyBundle): Promise<void> {
    const db = await this.#database;
    return new Promise((resolve, reject) => {
      const tx = db.transaction("studies", "readwrite");
      const store = tx.objectStore("studies");
      let conflict: Error | undefined;
      const request = store.get(bundle.document.id);
      request.onsuccess = () => {
        const current = request.result as StudyBundle | undefined;
        if (expected === null ? current !== undefined : current?.document.revision !== expected || bundle.document.revision !== expected + 1) {
          conflict = new Error("Study storage revision changed; reload the saved Study before retrying"); tx.abort(); return;
        }
        try { store.put(bundle); }
        catch (error) { conflict = storageError(error); tx.abort(); }
      };
      tx.oncomplete = () => resolve();
      tx.onabort = () => reject(conflict ?? storageError(tx.error));
      tx.onerror = () => reject(conflict ?? storageError(tx.error));
    });
  }
  async close(): Promise<void> { (await this.#database).close(); }
}

type Approval = { proposal: string; state: string; goal: string; base: string; revision: number };

/** Browser controls and agent tools share serialized, durable Study mutations. */
export class StudyDocumentController {
  #bundle: StudyBundle;
  #store: StudyStore;
  #backend: StudyBackend;
  #queue: Promise<unknown> = Promise.resolve();
  #approvals = new Map<string, Approval>();

  private constructor(bundle: StudyBundle, store: StudyStore, backend: StudyBackend) {
    this.#bundle = bundle; this.#store = store; this.#backend = backend;
  }
  static async create(request: CreateStudy, store: StudyStore, backend: StudyBackend = studyBackend, signal?: AbortSignal): Promise<StudyDocumentController> {
    const bundle = await backend.create(request, signal); signal?.throwIfAborted();
    await store.create(bundle);
    return new StudyDocumentController(bundle, store, backend);
  }
  static async open(id: string, store: StudyStore, backend: StudyBackend = studyBackend): Promise<StudyDocumentController> {
    const saved = await store.load(id);
    if (!saved) throw new Error(`Study ${id} is unavailable`);
    const bundle = await backend.validate(JSON.stringify(saved));
    return new StudyDocumentController(bundle, store, backend);
  }
  static async import(text: string, store: StudyStore, backend: StudyBackend = studyBackend): Promise<StudyDocumentController> {
    const bundle = await backend.validate(text); await store.create(bundle);
    return new StudyDocumentController(bundle, store, backend);
  }
  get bundle(): StudyBundle { return structuredClone(this.#bundle); }
  export(): string { return JSON.stringify(this.#bundle); }

  execute(request: StudyRequest, signal?: AbortSignal): Promise<StudyOperationResult> {
    if (request.operation.kind === "apply") return Promise.reject(new Error("Apply requires explicit user approval of the current proposal"));
    return this.#execute(request, signal);
  }
  #execute(request: StudyRequest, signal?: AbortSignal): Promise<StudyOperationResult> {
    const queuedRequest = structuredClone(request);
    const operation = this.#queue.then(async () => {
      signal?.throwIfAborted();
      if (queuedRequest.expected_revision !== this.#bundle.document.revision) throw new Error("Stale Study revision; inspect the current Study and retry");
      const completed = await this.#backend.execute(structuredClone(this.#bundle), queuedRequest, signal);
      if (queuedRequest.operation.kind !== "propose") signal?.throwIfAborted();
      await this.#store.commit(queuedRequest.expected_revision, completed.bundle);
      this.#bundle = completed.bundle;
      if (["inspect", "compare", "record_evidence"].includes(queuedRequest.operation.kind)) {
        for (const approval of this.#approvals.values()) approval.revision = completed.bundle.document.revision;
      } else this.#approvals.clear();
      return completed.result;
    });
    this.#queue = operation.catch(() => undefined);
    return operation;
  }
  /** Call only from the user's approval action; tokens stay in this controller. */
  recordUserApproval(proposal: string): string {
    const d = this.#bundle.document;
    const experiment = d.experiments[proposal];
    if (!experiment || experiment.kind !== "planning" || !experiment.start_state || experiment.goal !== d.active_goal || !d.active_goal || !d.recommended_state || !experiment.result_states.includes(d.recommended_state)) throw new Error("The proposal no longer matches the active goal and recommendation");
    const token = crypto.randomUUID();
    this.#approvals.set(token, { proposal, state: d.recommended_state, goal: d.active_goal, base: experiment.start_state, revision: d.revision });
    return token;
  }
  isApprovalCurrent(token: string): boolean {
    const approval = this.#approvals.get(token), d = this.#bundle.document;
    return !!approval && d.revision === approval.revision && d.active_goal === approval.goal &&
      d.recommended_state === approval.state && d.experiments[approval.proposal]?.start_state === approval.base;
  }
  applyApprovedProposal(token: string, signal?: AbortSignal): Promise<StudyOperationResult> {
    const approval = this.#approvals.get(token);
    if (!approval || !this.isApprovalCurrent(token)) return Promise.reject(new Error("Proposal approval expired; review the current proposal again"));
    return this.#execute({ expected_revision: approval.revision, operation: { kind: "apply", proposal: approval.proposal, state: approval.state, goal: approval.goal, base_state: approval.base } }, signal);
  }
}
