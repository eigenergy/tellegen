import { afterEach, describe, expect, it, vi } from "vitest";

import { isPermanentWasmLoadFailure } from "../src/errors.js";

describe("permanent wasm load failure", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("is decided by the error type before the message text", () => {
    // Neither message names the failure, and the first reads as a network one.
    const compile = new WebAssembly.CompileError("load failed at offset 12");
    const link = new WebAssembly.LinkError("import object field is missing");
    expect(isPermanentWasmLoadFailure(compile.message, compile)).toBe(true);
    expect(isPermanentWasmLoadFailure(link.message, link)).toBe(true);
    expect(isPermanentWasmLoadFailure(compile.message)).toBe(false);
    expect(isPermanentWasmLoadFailure(link.message)).toBe(false);
  });

  it("is true for any failure when the browser has no WebAssembly", () => {
    vi.stubGlobal("WebAssembly", undefined);
    expect(isPermanentWasmLoadFailure("Failed to fetch")).toBe(true);
  });

  it("falls back to the message text for other error types", () => {
    for (const message of [
      "CompileError: wasm validation error",
      "LinkError: import mismatch",
      "invalid opcode 0xfd",
      "WebAssembly is not defined",
    ]) {
      expect(isPermanentWasmLoadFailure(message)).toBe(true);
      expect(isPermanentWasmLoadFailure(message, new Error(message))).toBe(true);
    }
  });

  it("keeps network and aborted loads retryable", () => {
    for (const message of [
      "Failed to fetch dynamically imported module tellegen_bg.wasm",
      "NetworkError when attempting to fetch resource",
      "The operation was aborted",
      "net::ERR_INTERNET_DISCONNECTED",
      "HTTP 503",
    ]) {
      expect(isPermanentWasmLoadFailure(message)).toBe(false);
      expect(isPermanentWasmLoadFailure(message, new TypeError(message))).toBe(false);
    }
  });
});
