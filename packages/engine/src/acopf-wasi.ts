/** Minimal WASI Preview 1 host for the compute-only AC OPF reactor. */

const ERRNO_SUCCESS = 0;
const ERRNO_BADF = 8;
const ERRNO_NOSYS = 52;

export function createAcOpfWasi(onOutput: (text: string) => void) {
  let memory: WebAssembly.Memory | null = null;
  const decoder = new TextDecoder();
  const view = () => {
    if (!memory) throw new Error("AC OPF WASI memory is not bound");
    return new DataView(memory.buffer);
  };

  const wasi_snapshot_preview1 = {
    fd_write(fd: number, iovs: number, iovsLen: number, nwritten: number) {
      const data = view();
      let written = 0;
      let output = "";
      for (let index = 0; index < iovsLen; index += 1) {
        const ptr = data.getUint32(iovs + index * 8, true);
        const len = data.getUint32(iovs + index * 8 + 4, true);
        if (len > 0 && memory) {
          output += decoder.decode(new Uint8Array(memory.buffer, ptr, len));
        }
        written += len;
      }
      data.setUint32(nwritten, written, true);
      if (output && (fd === 1 || fd === 2)) onOutput(output);
      return ERRNO_SUCCESS;
    },
    clock_time_get(_id: number, _precision: bigint, out: number) {
      view().setBigUint64(
        out,
        BigInt(Math.round(performance.now() * 1e6)),
        true,
      );
      return ERRNO_SUCCESS;
    },
    clock_res_get(_id: number, out: number) {
      view().setBigUint64(out, 1_000n, true);
      return ERRNO_SUCCESS;
    },
    random_get(buf: number, len: number) {
      if (!memory) return ERRNO_BADF;
      const bytes = new Uint8Array(memory.buffer, buf, len);
      // Web Crypto limits one call to 65,536 bytes.
      for (let offset = 0; offset < len; offset += 65_536) {
        crypto.getRandomValues(
          bytes.subarray(offset, Math.min(len, offset + 65_536)),
        );
      }
      return ERRNO_SUCCESS;
    },
    environ_sizes_get(count: number, size: number) {
      const data = view();
      data.setUint32(count, 0, true);
      data.setUint32(size, 0, true);
      return ERRNO_SUCCESS;
    },
    environ_get: () => ERRNO_SUCCESS,
    args_sizes_get(count: number, size: number) {
      const data = view();
      data.setUint32(count, 0, true);
      data.setUint32(size, 0, true);
      return ERRNO_SUCCESS;
    },
    args_get: () => ERRNO_SUCCESS,
    proc_exit(code: number): never {
      throw new Error(`AC OPF wasm called proc_exit(${code})`);
    },
    sched_yield: () => ERRNO_SUCCESS,
    fd_close: () => ERRNO_BADF,
    fd_fdstat_get: () => ERRNO_BADF,
    fd_fdstat_set_flags: () => ERRNO_BADF,
    fd_prestat_get: () => ERRNO_BADF,
    fd_prestat_dir_name: () => ERRNO_BADF,
    fd_read: () => ERRNO_BADF,
    fd_seek: () => ERRNO_BADF,
    path_create_directory: () => ERRNO_NOSYS,
    path_filestat_get: () => ERRNO_NOSYS,
    path_open: () => ERRNO_NOSYS,
  };

  return {
    imports: { wasi_snapshot_preview1 },
    bind(instance: WebAssembly.Instance) {
      memory = instance.exports.memory as WebAssembly.Memory;
      if (!(memory instanceof WebAssembly.Memory)) {
        throw new Error("AC OPF wasm did not export memory");
      }
      const initialize = instance.exports._initialize;
      if (typeof initialize === "function") initialize();
    },
  };
}
