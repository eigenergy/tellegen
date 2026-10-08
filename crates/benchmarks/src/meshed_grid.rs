//! Deterministic synthetic meshed transmission grids for the DC OPF scale
//! benchmark (`dcopf-scale-bench`).
//!
//! A `rows x cols` lattice of 230 kV buses joined to their four neighbours,
//! written as a MATPOWER case so the benchmark reads it through the same PowerIO
//! parser as a file. Generators sit on a coarser sub-lattice; the western half
//! is cheap and the eastern half expensive, so the optimum moves power east
//! across the lattice and the thermal limits on that path bind. A fifth of the
//! branches are unrated, so the policy that keeps only rated limits has
//! something to drop.

use std::fmt::Write;

use crate::mc_feeder::SplitMix64;

/// The size and seed of one synthetic grid.
#[derive(Clone, Copy, Debug)]
pub struct GridShape {
    pub rows: usize,
    pub cols: usize,
    /// Generator spacing along both axes: a generator at every bus whose row and
    /// column are multiples of it, plus the reference bus.
    pub spacing: usize,
    pub seed: u64,
}

impl GridShape {
    /// A `rows x cols` grid with the default spacing and seed.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            spacing: 7,
            seed: 78_484,
        }
    }

    pub fn buses(&self) -> usize {
        self.rows * self.cols
    }

    /// Parse `ROWSxCOLS`.
    pub fn parse(text: &str) -> Option<Self> {
        let (rows, cols) = text.split_once('x')?;
        Some(Self::new(rows.parse().ok()?, cols.parse().ok()?))
    }
}

/// The grid as MATPOWER case text.
pub fn matpower(shape: &GridShape) -> String {
    let GridShape {
        rows,
        cols,
        spacing,
        seed,
    } = *shape;
    let mut rng = SplitMix64::new(seed);
    let id = |r: usize, c: usize| r * cols + c + 1;
    let hosts_generator = |r: usize, c: usize| {
        (r.is_multiple_of(spacing) && c.is_multiple_of(spacing)) || (r, c) == (0, 0)
    };

    let mut text = String::with_capacity(rows * cols * 200);
    let _ = writeln!(text, "function mpc = meshed_{rows}x{cols}");
    text.push_str("mpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [\n");
    for r in 0..rows {
        for c in 0..cols {
            let kind = if (r, c) == (0, 0) {
                3
            } else if hosts_generator(r, c) {
                2
            } else {
                1
            };
            let load = if hosts_generator(r, c) {
                0.0
            } else {
                rng.uniform(2.0, 8.0)
            };
            let _ = writeln!(
                text,
                " {} {kind} {load:.3} {:.3} 0 0 1 1 0 230 1 1.1 0.9;",
                id(r, c),
                0.3 * load
            );
        }
    }
    text.push_str("];\nmpc.gen = [\n");
    let mut costs = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if !hosts_generator(r, c) {
                continue;
            }
            let pmax = rng.uniform(250.0, 450.0);
            let _ = writeln!(
                text,
                " {} 0 0 {pmax:.1} {:.1} 1 100 1 {pmax:.1} 0 0 0 0 0 0 0 0 0 0 0 0;",
                id(r, c),
                -pmax
            );
            let linear = if c < cols / 2 {
                rng.uniform(10.0, 20.0)
            } else {
                rng.uniform(25.0, 40.0)
            };
            costs.push((rng.uniform(0.001, 0.01), linear));
        }
    }
    text.push_str("];\nmpc.branch = [\n");
    let branch = |text: &mut String, from: usize, to: usize, rng: &mut SplitMix64| {
        let x = rng.uniform(0.01, 0.03);
        let rate = if rng.next_f64() < 0.2 {
            0.0
        } else {
            rng.uniform(300.0, 600.0)
        };
        let _ = writeln!(
            text,
            " {from} {to} {:.5} {x:.5} 0 {rate:.1} {rate:.1} {rate:.1} 0 0 1 -360 360;",
            x / 8.0
        );
    };
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                branch(&mut text, id(r, c), id(r, c + 1), &mut rng);
            }
            if r + 1 < rows {
                branch(&mut text, id(r, c), id(r + 1, c), &mut rng);
            }
        }
    }
    text.push_str("];\nmpc.gencost = [\n");
    for (quadratic, linear) in costs {
        let _ = writeln!(text, " 2 0 0 3 {quadratic:.5} {linear:.3} 0;");
    }
    text.push_str("];\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_grid_parses_with_the_stated_shape() {
        let shape = GridShape::new(15, 15);
        let text = matpower(&shape);
        let source = powerio::Source::from_memory("grid.m", text.into_bytes()).expect("source");
        let options = powerio::ParseOptions::default()
            .format("matpower")
            .expect("format");
        let module = powerio::parse_with_options(source, &options).expect("parse");
        let network = tellegen::ir::balanced_module(module)
            .expect("balanced network")
            .into_value();
        assert_eq!(network.buses().len(), 225);
        // 15 x 14 horizontal plus 14 x 15 vertical branches.
        assert_eq!(network.branches().len(), 420);
        // Every 7th row and column: 3 x 3 generators, the reference among them.
        assert_eq!(network.generators().len(), 9);
        assert_eq!(matpower(&shape), matpower(&shape), "deterministic");
    }
}
