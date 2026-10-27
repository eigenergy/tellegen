//! Heap accounting for the benchmark binaries: a counting global allocator and
//! a timed region that reports its peak and retained heap. A binary opts in by
//! declaring `#[global_allocator] static ALLOCATOR: CountingAllocator =
//! CountingAllocator;`; without that declaration the counters stay at zero.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// Counts live and peak heap bytes on top of the system allocator.
pub struct CountingAllocator;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

fn shrink(bytes: usize) {
    LIVE.fetch_sub(bytes, Ordering::Relaxed);
}

// SAFETY: every call forwards to `System` with the caller's layout; the
// counters are side bookkeeping and never affect the returned pointers.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        shrink(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            if new_size >= layout.size() {
                grow(new_size - layout.size());
            } else {
                shrink(layout.size() - new_size);
            }
        }
        moved
    }
}

/// One timed region with its heap accounting.
pub struct Measured<T> {
    pub value: T,
    pub ms: f64,
    /// Highest live heap during the region, above the live heap at its start.
    pub peak_bytes: usize,
    /// Live heap after the region (with `value` still alive) minus before.
    pub retained_bytes: i64,
}

pub fn measure<T>(region: impl FnOnce() -> T) -> Measured<T> {
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let started = Instant::now();
    let value = region();
    let ms = started.elapsed().as_secs_f64() * 1e3;
    let after = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    Measured {
        value,
        ms,
        peak_bytes: peak.saturating_sub(before),
        retained_bytes: after as i64 - before as i64,
    }
}
