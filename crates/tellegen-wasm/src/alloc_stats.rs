//! Live and peak heap accounting for the browser engine.
//!
//! WASM linear memory only grows, so its size records the high-water mark of
//! every allocation since the worker started, not what a session retains.
//! Counting allocations as they pass through the system allocator separates
//! the two: the live count is what the engine holds right now and the peak is
//! the largest transient since the last reset. The counters cost two relaxed
//! atomic operations per allocation, which are plain loads and stores on the
//! single-threaded wasm32 target.
//!
//! The same wrapper makes linear memory grow in larger steps. The system
//! allocator (dlmalloc) extends its free top by just enough 64 KiB pages
//! whenever a request does not fit, so a large parse grows memory tens of
//! thousands of times, and a browser can charge each grow by how much memory
//! is already in use: in Chromium, growing to 1 GiB one page at a time while
//! writing to each new page took 7.8 s, and in sixteen-page steps 78 ms.
//! After a small request grew memory, the wrapper takes one block of an
//! eighth of linear memory (1 MiB to 64 MiB) from the system allocator and
//! returns it at once. dlmalloc grows for that block in one step and keeps
//! the freed block in its free top, so the requests after it are served
//! without growing. A request that grew memory by a step or more is left
//! alone: large requests come in doubling runs while a buffer is built, and
//! headroom left after one would sit unused below the next. Linear memory
//! never shrinks, so the headroom can leave its high-water mark higher than
//! the heap needed.

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

const PAGE_BYTES: usize = 65_536;
const MIN_GROW_AHEAD_PAGES: usize = 16;
const MAX_GROW_AHEAD_PAGES: usize = 1_024;

/// Pages to grow ahead once linear memory has reached `pages`: an eighth of
/// it, from 1 MiB to 64 MiB.
fn grow_ahead_pages(pages: usize) -> usize {
    (pages / 8).clamp(MIN_GROW_AHEAD_PAGES, MAX_GROW_AHEAD_PAGES)
}

/// Linear memory size in pages; zero off wasm, where nothing grows ahead.
fn memory_pages() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        core::arch::wasm32::memory_size::<0>()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// When the request just served grew linear memory by less than a step, grow
/// ahead by taking one step's block from the system allocator and returning
/// it. The block is not counted: it is never held.
fn grow_ahead(pages_before: usize) {
    let pages = memory_pages();
    let step = grow_ahead_pages(pages);
    if pages == pages_before || pages - pages_before >= step {
        return;
    }
    let Ok(layout) = Layout::from_size_align(step * PAGE_BYTES, 16) else {
        return;
    };
    // SAFETY: the block goes straight back to the allocator that returned it,
    // with the same layout, and is never read or written. A null return (no
    // room left to grow) leaves nothing to return.
    unsafe {
        let block = System.alloc(layout);
        if !block.is_null() {
            System.dealloc(block, layout);
        }
    }
}

// SAFETY: every call forwards to the system allocator unchanged; the counters
// only observe the sizes it was asked for, and growing ahead only takes and
// returns a block of its own.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pages = memory_pages();
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            self.added(layout.size());
        }
        grow_ahead(pages);
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pages = memory_pages();
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            self.added(layout.size());
        }
        grow_ahead(pages);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        self.removed(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let pages = memory_pages();
        let moved = unsafe { System.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            self.removed(layout.size());
            self.added(new_size);
        }
        grow_ahead(pages);
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

    #[test]
    fn grows_ahead_by_an_eighth_of_memory_from_one_to_sixty_four_mebibytes() {
        let mib = |pages: usize| (pages * PAGE_BYTES) >> 20;
        assert_eq!(mib(grow_ahead_pages(0)), 1);
        assert_eq!(mib(grow_ahead_pages(17)), 1);
        // 256 MiB of linear memory grows ahead by 32 MiB.
        assert_eq!(mib(grow_ahead_pages(4_096)), 32);
        assert_eq!(mib(grow_ahead_pages(16_384)), 64);
        // The wasm32 maximum, 4 GiB, still steps by 64 MiB.
        assert_eq!(mib(grow_ahead_pages(65_536)), 64);
    }
}
