//! Experimental: threads for the RMS engine in a WebAssembly build
//! with shared memory (`+atomics`). The page's RMS worker defines
//! `globalThis.__tellegenSpawnRmsThread(ptr)`, which starts a Web Worker on
//! the same module and memory that calls [`rms_thread_entry`] with `ptr`;
//! [`rms_threads_init`] starts the engine's pools through it.
//!
//! @block Threads in WebAssembly

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// The RMS worker's hook: a Web Worker that calls `rms_thread_entry(ptr)`.
    #[wasm_bindgen(js_name = __tellegenSpawnRmsThread)]
    fn spawn_rms_thread(ptr: u32);
}

type Task = Box<dyn FnOnce() + Send>;

fn spawner(f: Task) {
    let ptr = Box::into_raw(Box::new(f)) as usize as u32;
    spawn_rms_thread(ptr);
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

/// Run the task a thread was started with (a pool's worker loop; it does
/// not return).
#[wasm_bindgen]
pub fn rms_thread_entry(ptr: u32) {
    // SAFETY: `ptr` is a `Box<Task>` leaked by `spawner`, handed over once.
    let f = unsafe { Box::from_raw(ptr as usize as *mut Task) };
    (*f)();
}

/// Start the RMS engine's thread pools: `global` threads for the step's
/// parallel parts and `relays` for the relay evaluation, besides the caller,
/// and one for the relays beside the step.
#[wasm_bindgen]
pub fn rms_threads_init(global: usize, relays: usize) {
    tellegen::rms::set_clock(now_ms);
    tellegen::rms::configure_threads(spawner, global, relays);
}

/// This build has threads.
#[wasm_bindgen]
pub fn rms_threads_supported() -> bool {
    true
}
