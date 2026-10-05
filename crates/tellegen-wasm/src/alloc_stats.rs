//! Live and peak heap accounting for the browser engine.
//!
//! WASM linear memory only grows, so its size records the high-water mark of
//! every allocation since the worker started, not what a session retains.
//! Counting allocations as they pass through the system allocator separates
//! the two: the live count is what the engine holds right now and the peak is
//! the largest transient since the last reset. The counters cost two relaxed
//! atomic operations per allocation, which are plain loads and stores on the
//! single-threaded wasm32 target.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

pub(crate) struct CountingAllocator {
    live: AtomicUsize,
    peak: AtomicUsize,
}

impl CountingAllocator {
    pub(crate) const fn new() -> Self {
        Self {
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    fn added(&self, bytes: usize) {
        let live = self.live.fetch_add(bytes, Relaxed) + bytes;
        self.peak.fetch_max(live, Relaxed);
    }

    fn removed(&self, bytes: usize) {
        self.live.fetch_sub(bytes, Relaxed);
    }

    pub(crate) fn live(&self) -> usize {
        self.live.load(Relaxed)
    }

    pub(crate) fn peak(&self) -> usize {
        self.peak.load(Relaxed)
    }

    /// Start a new peak window at the current live size.
    pub(crate) fn reset_peak(&self) {
        self.peak.store(self.live(), Relaxed);
    }
}

// SAFETY: every call forwards to the system allocator unchanged; the counters
// only observe the sizes it was asked for.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            self.added(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            self.added(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        self.removed(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            self.removed(layout.size());
            self.added(new_size);
        }
        moved
    }
}

#[cfg(target_arch = "wasm32")]
#[global_allocator]
pub(crate) static ALLOCATOR: CountingAllocator = CountingAllocator::new();

/// Native builds keep the platform allocator, so the browser-facing stats
/// report zeros there.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) static ALLOCATOR: CountingAllocator = CountingAllocator::new();

pub(crate) fn linear_memory_bytes() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        core::arch::wasm32::memory_size::<0>() * 65_536
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_live_and_peak_bytes_through_each_entry_point() {
        let counter = CountingAllocator::new();
        let small = Layout::from_size_align(64, 8).unwrap();
        let large = Layout::from_size_align(4_096, 8).unwrap();
        unsafe {
            let a = counter.alloc(small);
            let b = counter.alloc_zeroed(large);
            assert_eq!(counter.live(), 4_160);
            assert_eq!(counter.peak(), 4_160);
            counter.dealloc(b, large);
            assert_eq!(counter.live(), 64);
            assert_eq!(counter.peak(), 4_160);
            counter.reset_peak();
            assert_eq!(counter.peak(), 64);
            let a = counter.realloc(a, small, 1_024);
            assert_eq!(counter.live(), 1_024);
            assert_eq!(counter.peak(), 1_024);
            counter.dealloc(a, Layout::from_size_align(1_024, 8).unwrap());
        }
        assert_eq!(counter.live(), 0);
    }
}
