//! Deterministic randomness. One ChaCha8 stream per stage, each seeded from
//! the request seed and a stage tag, so adding a draw to one stage never
//! perturbs another. The streams themselves are portable (ChaCha has no
//! platform dependent state; `rand`'s float and range conversions are
//! specified bit for bit), but the `ln`, `sin`, `cos` and `exp` applied to the
//! draws round differently on different platforms, so native and WebAssembly
//! values can differ in the last bits.

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Stage tags mixed into the seed. Fixed constants: renumbering them would
/// change every generated grid.
#[derive(Clone, Copy, Debug)]
#[repr(u64)]
pub enum StreamTag {
    Site = 0x5349_5445,
    Topology = 0x544f_504f,
    Electrical = 0x454c_4543,
    Loads = 0x4c4f_4144,
    Generators = 0x4745_4e53,
    Osm = 0x4f53_4d21,
    Calibrate = 0x4341_4c49,
}

/// A seeded stream with the few distributions the generator uses.
pub struct Stream {
    rng: ChaCha8Rng,
    spare_normal: Option<f64>,
}

impl Stream {
    /// The stream for `tag` under `seed`.
    pub fn new(seed: u64, tag: StreamTag) -> Self {
        // splitmix-style mixing so neighbouring seeds do not share stream prefixes.
        let mut z = seed ^ (tag as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        Self {
            rng: ChaCha8Rng::seed_from_u64(z),
            spare_normal: None,
        }
    }

    /// Uniform in `[0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        self.rng.random::<f64>()
    }

    /// Uniform in `[lo, hi)`; `lo` when the interval is empty.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        if hi > lo {
            lo + (hi - lo) * self.uniform()
        } else {
            lo
        }
    }

    /// Uniform integer in `[0, n)`; panics on `n == 0` like `random_range`.
    pub fn index(&mut self, n: usize) -> usize {
        self.rng.random_range(0..n)
    }

    /// Standard normal by Box–Muller, caching the second deviate.
    pub fn normal(&mut self) -> f64 {
        if let Some(z) = self.spare_normal.take() {
            return z;
        }
        let u1 = loop {
            let u = self.uniform();
            if u > 1e-300 {
                break u;
            }
        };
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        self.spare_normal = Some(r * theta.sin());
        r * theta.cos()
    }

    /// `exp(mu + sigma * z)`, clamped to `[exp(mu - 3 sigma), exp(mu + 3 sigma)]`
    /// so one draw never produces a physically absurd element.
    pub fn log_normal_bounded(&mut self, mu: f64, sigma: f64) -> f64 {
        let z = self.normal().clamp(-3.0, 3.0);
        (mu + sigma * z).exp()
    }

    /// Index drawn with probability proportional to `weights` (non-negative).
    /// A degenerate weight vector (empty or all zero) falls back to a uniform
    /// draw, so a weird fuel mix never aborts generation.
    pub fn weighted_index(&mut self, weights: &[f64]) -> Option<usize> {
        if weights.is_empty() {
            return None;
        }
        let total: f64 = weights.iter().filter(|w| w.is_finite() && **w > 0.0).sum();
        if total <= 0.0 {
            return Some(self.index(weights.len()));
        }
        let target = self.uniform() * total;
        let mut running = 0.0;
        for (i, w) in weights.iter().enumerate() {
            if w.is_finite() && *w > 0.0 {
                running += w;
                if target < running {
                    return Some(i);
                }
            }
        }
        Some(weights.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_reproducible_and_tag_separated() {
        let mut a = Stream::new(7, StreamTag::Site);
        let mut b = Stream::new(7, StreamTag::Site);
        let mut c = Stream::new(7, StreamTag::Topology);
        let xa: Vec<f64> = (0..8).map(|_| a.uniform()).collect();
        let xb: Vec<f64> = (0..8).map(|_| b.uniform()).collect();
        let xc: Vec<f64> = (0..8).map(|_| c.uniform()).collect();
        assert_eq!(xa, xb);
        assert_ne!(xa, xc);
    }

    #[test]
    fn normal_has_unit_scale() {
        let mut s = Stream::new(3, StreamTag::Electrical);
        let n = 20_000;
        let (mut sum, mut sq) = (0.0, 0.0);
        for _ in 0..n {
            let z = s.normal();
            sum += z;
            sq += z * z;
        }
        let mean = sum / n as f64;
        let var = sq / n as f64 - mean * mean;
        assert!(mean.abs() < 0.03, "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "var {var}");
    }

    #[test]
    fn weighted_index_respects_weights() {
        let mut s = Stream::new(1, StreamTag::Generators);
        let mut counts = [0usize; 3];
        for _ in 0..3000 {
            counts[s.weighted_index(&[0.0, 1.0, 3.0]).unwrap()] += 1;
        }
        assert_eq!(counts[0], 0);
        assert!(counts[2] > 2 * counts[1]);
        assert_eq!(s.weighted_index(&[]), None);
        assert!(s.weighted_index(&[0.0, 0.0]).is_some());
    }
}
