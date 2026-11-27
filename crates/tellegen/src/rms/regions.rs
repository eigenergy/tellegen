//! Experimental: the network split into regions (`TELLEGEN_REGIONS`, a
//! JSON map of bus number to region), and the parts of a step that each region
//! does as one task on the spin pool. Every region owns its buses' rows and the
//! states of the units at its buses, and evaluates them in the order the whole
//! system does, so the results are the serial ones bit for bit.
//!
//! @block Network regions

use std::sync::Arc;
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
use std::sync::{Mutex, OnceLock};

use super::system::DynSystem;

/// The buses, units, plants and sources of each region.
// @block Region index
#[derive(Debug)]
pub(crate) struct RegionIndex {
    pub(crate) nr: usize,
    pub(crate) buses: Vec<Vec<usize>>,
    pub(crate) devices: Vec<Vec<usize>>,
    pub(crate) held: Vec<Vec<usize>>,
    pub(crate) sources: Vec<Vec<usize>>,
    /// Per bus, its index among its region's buses.
    pub(crate) local: Vec<u32>,
    /// The system shape the index was built for (its cache key).
    #[cfg_attr(not(not(all(target_arch = "wasm32", not(target_feature = "atomics")))), allow(dead_code))]
    key: (usize, usize, usize, usize),
}

/// The region index of `sys` (built once per system shape), or `None` without
/// regions or on wasm.
pub(crate) fn region_index(sys: &DynSystem) -> Option<Arc<RegionIndex>> {
    #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
    {
        let _ = sys;
        return None;
    }
    #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
    {
        if !sys.threads || super::spin_pool::SpinPool::global().threads() == 0 {
            return None;
        }
        let reg = super::schur::region_map(sys)?;
        static CACHE: OnceLock<Mutex<Option<Arc<RegionIndex>>>> = OnceLock::new();
        let key = (sys.n, sys.devices.len(), sys.held.len(), sys.sources.len());
        let cache = CACHE.get_or_init(|| Mutex::new(None));
        let mut c = cache.lock().expect("region index");
        if let Some(ri) = c.as_ref().filter(|ri| ri.key == key) {
            return Some(Arc::clone(ri));
        }
        let nr = reg.iter().copied().max().map_or(1, |m| usize::from(m) + 1);
        let mut buses = vec![Vec::new(); nr];
        let mut local = vec![0u32; reg.len()];
        for (b, &r) in reg.iter().enumerate() {
            local[b] = buses[usize::from(r)].len() as u32;
            buses[usize::from(r)].push(b);
        }
        let mut devices = vec![Vec::new(); nr];
        for (k, d) in sys.devices.iter().enumerate() {
            devices[usize::from(reg[d.bus])].push(k);
        }
        let mut held = vec![Vec::new(); nr];
        for (k, h) in sys.held.iter().enumerate() {
            held[usize::from(reg[h.bus])].push(k);
        }
        let mut sources = vec![Vec::new(); nr];
        for (k, s) in sys.sources.iter().enumerate() {
            sources[usize::from(reg[s.bus])].push(k);
        }
        let ri = Arc::new(RegionIndex {
            nr,
            buses,
            devices,
            held,
            sources,
            local,
            key,
        });
        *c = Some(Arc::clone(&ri));
        Some(ri)
    }
}

/// `f(0..n)` on the spin pool.
pub(crate) fn par_for(n: usize, f: &(dyn Fn(usize) + Sync)) {
    #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
    super::spin_pool::SpinPool::global().run(n, f);
    #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
    (0..n).for_each(f);
}

/// A raw pointer that may cross threads: each task touches its own entries.
pub(crate) struct Shared<T>(pub(crate) *mut T);
impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Shared<T> {}
unsafe impl<T> Send for Shared<T> {}
unsafe impl<T> Sync for Shared<T> {}
impl<T> Shared<T> {
    /// # Safety
    /// No other task touches entry `i` meanwhile.
    #[inline]
    pub(crate) unsafe fn at(self, i: usize) -> &'static mut T {
        unsafe { &mut *self.0.add(i) }
    }
    /// # Safety
    /// No other task touches `i..i + k` meanwhile.
    #[inline]
    pub(crate) unsafe fn slice(self, i: usize, k: usize) -> &'static mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.0.add(i), k) }
    }
}

/// Experimental: residual wall time per call and per region task, µs.
pub(crate) static CALL_US: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) static TASK_US: [std::sync::atomic::AtomicU64; 16] = [const { std::sync::atomic::AtomicU64::new(0) }; 16];

/// Adds a region task's time when dropped.
pub(crate) struct RegionTimer(pub(crate) usize, pub(crate) crate::rms::clock::Instant);
impl Drop for RegionTimer {
    fn drop(&mut self) {
        let us = self.1.elapsed().as_micros() as u64;
        TASK_US[self.0.min(15)].fetch_add(us, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The residual's parallel timing as one line.
pub fn region_stats() -> String {
    use std::sync::atomic::Ordering::Relaxed;
    let calls = CALLS.load(Relaxed).max(1) as f64;
    let tasks: Vec<String> = TASK_US
        .iter()
        .map(|t| t.load(Relaxed))
        .filter(|&t| t > 0)
        .map(|t| format!("{:.0}", t as f64 / calls))
        .collect();
    format!(
        "residual by regions: {} calls, {:.0} us per call; per region task, us per call: {}",
        CALLS.load(Relaxed),
        CALL_US.load(Relaxed) as f64 / calls,
        tasks.join(" ")
    )
}
