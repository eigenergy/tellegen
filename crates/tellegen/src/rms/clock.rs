//! Experimental: the clock of the timing statistics and of the spin
//! pool's idle naps. In the browser (`wasm32-unknown-unknown`)
//! `std::time::Instant` panics; there the adapter hands in a clock
//! ([`set_clock`], milliseconds), and without one every time reads zero.
//!
//! @block Timing clock

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time::Instant;

/// Set the browser's clock (milliseconds); natively a no-op.
pub fn set_clock(now_ms: fn() -> f64) {
    #[cfg(target_arch = "wasm32")]
    let _ = NOW.set(now_ms);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = now_ms;
}

#[cfg(target_arch = "wasm32")]
static NOW: std::sync::OnceLock<fn() -> f64> = std::sync::OnceLock::new();

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Instant(f64);

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub(crate) fn now() -> Self {
        Self(NOW.get().map_or(0.0, |f| f()))
    }
    pub(crate) fn elapsed(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(((Self::now().0 - self.0) / 1e3).max(0.0))
    }
}
