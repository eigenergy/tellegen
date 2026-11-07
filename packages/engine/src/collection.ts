/** Collection acquisition has its own worker. Cancellation cannot destroy an
 * existing Study, and completed imports own independent portable modules. */
import { engineHost, isolatedEngineHost, type EngineHost } from "./host.js";
import type { EngineRequest, EngineValue } from "./protocol.js";
import type { IngestedCase, IngestedDistCase } from "./index.js";

export const MAX_COLLECTION_BYTES = 64 * 1024 * 1024;
export const MAX_COLLECTION_FILES = 4096;
const MAX_METADATA_BYTES = 1024 * 1024;

export const COLLECTION_UNAVAILABLE =
  "ZIP and folder imports are unavailable in this build. Open a single case file instead.";

/** Query the compiled backend, rather than inferring support from package versions. */
export async function collectionSupported(): Promise<boolean> {
  return await engineHost().call({ op: "collection_supported" }) === "true";
}

export interface CollectionSelection {
  root?: string | null;
  primary?: string | null;
  members?: string[] | null;
  format?: string | null;
  operation?: "parse" | "deserialize";
}
export interface CollectionCandidate {
  selection: CollectionSelection;
  status: "selectable" | "needs-options" | "needs-format" | "probe-limit" | "unsupported-build" | "incomplete";
}
export interface CollectionEntry { path: string; bytes: number }
export interface CollectionFile { path: string; bytes: Uint8Array }
export interface CollectionInspection {
  name: string;
  entries: CollectionEntry[];
  candidates: CollectionCandidate[];
  original_zip: boolean;
  limits: { files: number; expanded_bytes: number };
}
export type ImportedCollectionCase = {
  dependencies: string[];
  incomplete: boolean;
  origin: { origin: { collection: string; containers: string[] }; selection: CollectionSelection };
} & (
  | { kind: "balanced"; payload: IngestedCase }
  | { kind: "multiconductor"; payload: IngestedDistCase }
);

let sequence = 0;
const abortError = () => new DOMException("Collection import cancelled", "AbortError");
function text(value: EngineValue): string {
  if (typeof value !== "string") throw new Error("invalid collection response");
  return value;
}
function boundedJson(value: unknown): string {
  const text = JSON.stringify(value);
  if (new TextEncoder().encode(text).length > MAX_METADATA_BYTES)
    throw new Error("collection metadata exceeds 1 MiB");
  return text;
}
function validName(name: string): void {
  if (!name || new TextEncoder().encode(name).length > 4096)
    throw new Error("collection name must contain 1 to 4096 UTF-8 bytes");
}

/** One bounded collection, acquired once and parsed serially on demand.
 * Call free when its chooser/export session closes. Returned case modules
 * survive free, cancellation and worker failure; no source ZIP is embedded
 * in those modules. Nested transport ZIP traversal is not implicit. */
export class BrowserCaseCollection {
  readonly #handle = ++sequence;
  readonly #host: EngineHost;
  #closed = false;
  #busy = false;
  #detach: (() => void) | undefined;
  inspection!: CollectionInspection;

  private constructor(host: EngineHost, signal?: AbortSignal) {
    this.#host = host;
    const abort = () => this.cancel();
    if (signal?.aborted) throw abortError();
    signal?.addEventListener("abort", abort, { once: true });
    this.#detach = () => signal?.removeEventListener("abort", abort);
  }

  static async fromZip(name: string, bytes: Uint8Array, signal?: AbortSignal): Promise<BrowserCaseCollection> {
    validName(name);
    if (!(bytes instanceof Uint8Array) || bytes.byteLength > MAX_COLLECTION_BYTES)
      throw new Error("ZIP exceeds the 64 MiB compressed input limit");
    if (signal?.aborted) throw abortError();
    const session = new BrowserCaseCollection(isolatedEngineHost(), signal);
    return session.open({ op: "collection_zip", collection: session.#handle, name, bytes });
  }

  static async fromFiles(name: string, files: readonly CollectionFile[], signal?: AbortSignal): Promise<BrowserCaseCollection> {
    validName(name);
    if (signal?.aborted) throw abortError();
    if (files.length > MAX_COLLECTION_FILES) throw new Error("collection exceeds 4096 files");
    let total = 0;
    for (const file of files) {
      if (typeof file.path !== "string" || !(file.bytes instanceof Uint8Array))
        throw new Error("invalid collection file");
      total += file.bytes.byteLength;
      if (total > MAX_COLLECTION_BYTES) throw new Error("collection exceeds 64 MiB");
    }
    const names = boundedJson(files.map(file => file.path));
    const offsets = new Uint32Array(files.length + 1);
    const bytes = new Uint8Array(total);
    files.forEach((file, index) => {
      bytes.set(file.bytes, offsets[index]);
      offsets[index + 1] = offsets[index] + file.bytes.byteLength;
    });
    const session = new BrowserCaseCollection(isolatedEngineHost(), signal);
    return session.open({ op: "collection_files", collection: session.#handle, name, names, offsets, bytes });
  }

  private async open(request: EngineRequest): Promise<this> {
    try {
      this.inspection = JSON.parse(text(await this.call(request)));
      return this;
    } catch (error) {
      await this.free();
      throw error;
    }
  }

  private async call(request: EngineRequest): Promise<EngineValue> {
    if (this.#closed) throw new Error("collection was closed or cancelled");
    if (this.#busy) throw new Error("a collection operation is already running");
    this.#busy = true;
    try {
      const value = await this.#host.call(request);
      if (this.#closed) throw abortError();
      return value;
    } finally { this.#busy = false; }
  }

  async ingest(selection: CollectionSelection): Promise<ImportedCollectionCase> {
    return JSON.parse(text(await this.call({ op: "collection_ingest", collection: this.#handle, selection: boundedJson(selection) })));
  }

  async originalZip(): Promise<Uint8Array> {
    const value = await this.call({ op: "collection_original", collection: this.#handle });
    if (!(value instanceof Uint8Array)) throw new Error("invalid ZIP response");
    return value;
  }

  async sourceZip(selection: CollectionSelection, scope: "selected" | "project"): Promise<Uint8Array> {
    const value = await this.call({ op: "collection_source", collection: this.#handle, selection: boundedJson(selection), scope });
    if (!(value instanceof Uint8Array)) throw new Error("invalid source ZIP response");
    return value;
  }

  cancel(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#detach?.();
    // A synchronous fallback cannot be interrupted. Its queued cleanup runs
    // after the call; call() discards any late answer before publishing it.
    if (!this.#host.cancel?.(abortError()))
      void this.#host.call({ op: "collection_free", collection: this.#handle }).catch(() => {});
  }

  async free(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#detach?.();
    try { await this.#host.call({ op: "collection_free", collection: this.#handle }); }
    catch { /* A failed worker has already released all its handles. */ }
    finally { this.#host.cancel?.(new Error("collection closed")); }
  }
}
