//! Poisson-disk ("blue noise") fill inside a region: candidate points are
//! rejected when closer than a separation radius to any accepted point or
//! anchor, so fill buses spread like substations instead of clumping. A
//! sparse background grid keyed on projected km keeps each test O(1) without
//! allocating buckets for the empty area outside long, thin regions. The
//! routine always returns `count` points: when the region cannot hold them at
//! the chosen spacing, the constraint is relaxed at the end so the bus total
//! stays deterministic.

use std::collections::HashMap;

use crate::geom::region::RegionShape;
use crate::geom::Projection;
use crate::places::PlacedCity;
use crate::rng::Stream;

struct Grid {
    cell: f64,
    min_x: f64,
    min_y: f64,
    cols: usize,
    rows: usize,
    cells: HashMap<(usize, usize), Vec<(f64, f64)>>,
}

impl Grid {
    fn new(cell: f64, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        let cols = (((max_x - min_x) / cell).ceil() as usize).clamp(1, 4096);
        let rows = (((max_y - min_y) / cell).ceil() as usize).clamp(1, 4096);
        Self {
            cell,
            min_x,
            min_y,
            cols,
            rows,
            cells: HashMap::new(),
        }
    }

    fn index(&self, x: f64, y: f64) -> (usize, usize) {
        let c = (((x - self.min_x) / self.cell).floor().max(0.0) as usize).min(self.cols - 1);
        let r = (((y - self.min_y) / self.cell).floor().max(0.0) as usize).min(self.rows - 1);
        (c, r)
    }

    fn insert(&mut self, x: f64, y: f64) {
        let index = self.index(x, y);
        self.cells.entry(index).or_default().push((x, y));
    }

    fn far(&self, x: f64, y: f64, sep2: f64) -> bool {
        let (c, r) = self.index(x, y);
        let c0 = c.saturating_sub(2);
        let r0 = r.saturating_sub(2);
        for rr in r0..=(r + 2).min(self.rows - 1) {
            for cc in c0..=(c + 2).min(self.cols - 1) {
                let Some(points) = self.cells.get(&(cc, rr)) else {
                    continue;
                };
                for &(px, py) in points {
                    if (px - x).powi(2) + (py - y).powi(2) <= sep2 {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// `count` points inside `shape`, at least `sep` km from each other and from
/// `existing` (frame longitudes), using `weights` as the polygon selection
/// weights. Returned in generation order.
pub fn blue_noise_points(
    shape: &RegionShape,
    weights: &[f64],
    count: usize,
    existing: &[(f64, f64)],
    rng: &mut Stream,
) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(count);
    if count == 0 {
        return out;
    }
    let proj = Projection::for_frame(&shape.frame);
    let area = shape.area_km2.max(1.0);
    // Hexagonal packing would allow sep = sqrt(2A / (sqrt(3) n)); 0.62 of the
    // square-lattice spacing leaves room for rejection sampling to find spots.
    let sep = 0.62 * (area / (count + existing.len()).max(1) as f64).sqrt();
    let sep2 = sep * sep;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for &(lon, lat) in [
        (shape.frame.west, shape.frame.south),
        (shape.frame.east, shape.frame.south),
        (shape.frame.west, shape.frame.north),
        (shape.frame.east, shape.frame.north),
        (
            (shape.frame.west + shape.frame.east) / 2.0,
            shape.frame.south,
        ),
        (
            (shape.frame.west + shape.frame.east) / 2.0,
            shape.frame.north,
        ),
    ]
    .iter()
    {
        let (x, y) = proj.project(lon, lat);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let cell = (sep / 2.0_f64.sqrt()).max(1e-3);
    let mut grid = Grid::new(cell, min_x - sep, min_y - sep, max_x + sep, max_y + sep);
    for &(lon, lat) in existing {
        let (x, y) = proj.project(lon, lat);
        grid.insert(x, y);
    }
    let mut tries = 0usize;
    let max_tries = 80 * count;
    while out.len() < count && tries < max_tries {
        tries += 1;
        let (lon, lat) = shape.sample_point(rng, weights);
        let (x, y) = proj.project(lon, lat);
        if grid.far(x, y, sep2) {
            grid.insert(x, y);
            out.push((lon, lat));
        }
    }
    while out.len() < count {
        out.push(shape.sample_point(rng, weights));
    }
    out
}

/// A population density surface on a projected grid: each place adds its
/// population through a Gaussian kernel of bandwidth `h` km, cut at 3h. Values
/// are relative to the densest cell, compressed by a power (`fill_gamma`, a
/// square root by default) so a metropolis does not take every bus, and
/// floored at `floor` so the countryside still gets substations.
struct Density {
    proj: Projection,
    cell: f64,
    min_x: f64,
    min_y: f64,
    cols: usize,
    rows: usize,
    value: Vec<f64>,
}

impl Density {
    fn new(shape: &RegionShape, cities: &[PlacedCity], h: f64, floor: f64, gamma: f64) -> Self {
        let proj = Projection::for_frame(&shape.frame);
        let cell = (h / 2.0).max(1.0);
        let corners = [
            (shape.frame.west, shape.frame.south),
            (shape.frame.east, shape.frame.south),
            (shape.frame.west, shape.frame.north),
            (shape.frame.east, shape.frame.north),
            (
                (shape.frame.west + shape.frame.east) / 2.0,
                shape.frame.south,
            ),
            (
                (shape.frame.west + shape.frame.east) / 2.0,
                shape.frame.north,
            ),
        ];
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for &(lon, lat) in &corners {
            let (x, y) = proj.project(lon, lat);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let cols = (((max_x - min_x) / cell).ceil() as usize + 1).clamp(1, 2048);
        let rows = (((max_y - min_y) / cell).ceil() as usize + 1).clamp(1, 2048);
        let cell = ((max_x - min_x) / (cols - 1).max(1) as f64)
            .max((max_y - min_y) / (rows - 1).max(1) as f64)
            .max(cell);
        let mut value = vec![0.0; cols * rows];
        let reach = (3.0 * h / cell).ceil() as isize;
        for c in cities {
            let (x, y) = proj.project(c.lon, c.lat);
            let ci = ((x - min_x) / cell).round() as isize;
            let ri = ((y - min_y) / cell).round() as isize;
            for r in (ri - reach).max(0)..=(ri + reach).min(rows as isize - 1) {
                for q in (ci - reach).max(0)..=(ci + reach).min(cols as isize - 1) {
                    let dx = min_x + q as f64 * cell - x;
                    let dy = min_y + r as f64 * cell - y;
                    let w = (-(dx * dx + dy * dy) / (2.0 * h * h)).exp();
                    value[r as usize * cols + q as usize] += c.place.population.max(0.0) * w;
                }
            }
        }
        let peak = value.iter().copied().fold(0.0, f64::max);
        for v in value.iter_mut() {
            let rel = if peak > 0.0 {
                (*v / peak).powf(gamma)
            } else {
                0.0
            };
            *v = floor + (1.0 - floor) * rel;
        }
        Self {
            proj,
            cell,
            min_x,
            min_y,
            cols,
            rows,
            value,
        }
    }

    fn at(&self, lon: f64, lat: f64) -> f64 {
        let (x, y) = self.proj.project(lon, lat);
        let q = (((x - self.min_x) / self.cell).round().max(0.0) as usize).min(self.cols - 1);
        let r = (((y - self.min_y) / self.cell).round().max(0.0) as usize).min(self.rows - 1);
        self.value[r * self.cols + q]
    }
}

/// `count` points inside `shape` distributed like the population around
/// `cities` (kernel bandwidth `bandwidth_km`, relative floor `floor`; see
/// [`Density`]), at least a separation apart from each other and from
/// `existing`. A candidate is drawn uniformly and kept with probability
/// equal to the density where it falls; the separation is the even spacing
/// of the density-weighted area, so dense areas fill without clumping. With
/// no cities this is [`blue_noise_points`].
#[allow(clippy::too_many_arguments)]
pub fn weighted_blue_noise_points(
    shape: &RegionShape,
    weights: &[f64],
    count: usize,
    existing: &[(f64, f64)],
    cities: &[PlacedCity],
    bandwidth_km: f64,
    floor: f64,
    rng: &mut Stream,
) -> Vec<(f64, f64)> {
    if cities.is_empty() || count == 0 {
        return blue_noise_points(shape, weights, count, existing, rng);
    }
    let gamma = crate::tuning::tuning().fill_gamma.clamp(0.05, 2.0);
    let density = Density::new(
        shape,
        cities,
        bandwidth_km.max(1.0),
        floor.clamp(1e-3, 1.0),
        gamma,
    );
    // Density-weighted area, from samples of the region.
    let mut probe = 0.0;
    let samples = 2000;
    for _ in 0..samples {
        let (lon, lat) = shape.sample_point(rng, weights);
        probe += density.at(lon, lat);
    }
    let effective = shape.area_km2.max(1.0) * probe / samples as f64;
    let sep = 0.5 * (effective / (count + existing.len()).max(1) as f64).sqrt();
    let sep2 = sep * sep;
    let proj = Projection::for_frame(&shape.frame);
    let (min_x, min_y, max_x, max_y) = (
        density.min_x,
        density.min_y,
        density.min_x + density.cell * density.cols as f64,
        density.min_y + density.cell * density.rows as f64,
    );
    let mut grid = Grid::new(
        (sep / 2.0_f64.sqrt()).max(1e-3),
        min_x - sep,
        min_y - sep,
        max_x + sep,
        max_y + sep,
    );
    for &(lon, lat) in existing {
        let (x, y) = proj.project(lon, lat);
        grid.insert(x, y);
    }
    let mut out = Vec::with_capacity(count);
    let mut tries = 0usize;
    let max_tries = 400 * count;
    while out.len() < count && tries < max_tries {
        tries += 1;
        let (lon, lat) = shape.sample_point(rng, weights);
        if rng.uniform() >= density.at(lon, lat) {
            continue;
        }
        let (x, y) = proj.project(lon, lat);
        if grid.far(x, y, sep2) {
            grid.insert(x, y);
            out.push((lon, lat));
        }
    }
    while out.len() < count {
        out.push(shape.sample_point(rng, weights));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::haversine_km;
    use crate::rng::{Stream, StreamTag};

    #[test]
    fn grid_allocates_buckets_only_for_occupied_cells() {
        // A very elongated frame exercises the 4096-cell dimension cap while
        // proving an empty region does not eagerly allocate millions of Vecs.
        let mut grid = Grid::new(1.0, 0.0, 0.0, 10_000.0, 1_000.0);
        assert_eq!(grid.cols, 4096);
        assert_eq!(grid.rows, 1000);
        assert!(grid.cells.is_empty());
        grid.insert(10.0, 10.0);
        grid.insert(10.2, 10.1);
        assert_eq!(grid.cells.len(), 1);
        assert!(grid.far(100.0, 100.0, 1.0));
    }

    #[test]
    fn fill_is_spread_and_complete() {
        let geom = serde_json::json!({"type":"Polygon","coordinates":[[[0.0,0.0],[2.0,0.0],[2.0,2.0],[0.0,2.0],[0.0,0.0]]]});
        let shape = RegionShape::from_geometry(&geom).unwrap();
        let w = shape.polygon_areas();
        let mut rng = Stream::new(4, StreamTag::Site);
        let pts = blue_noise_points(&shape, &w, 60, &[], &mut rng);
        assert_eq!(pts.len(), 60);
        assert!(pts.iter().all(|p| shape.contains(p.0, p.1)));
        let mut min_d = f64::INFINITY;
        for i in 0..pts.len() {
            for j in i + 1..pts.len() {
                min_d = min_d.min(haversine_km(pts[i].0, pts[i].1, pts[j].0, pts[j].1));
            }
        }
        // 49,000 km² over 60 points: a square lattice would be ~29 km apart.
        assert!(min_d > 12.0, "nearest pair {min_d} km");
        // Over-filling still returns the requested count.
        let many = blue_noise_points(&shape, &w, 5000, &[], &mut rng);
        assert_eq!(many.len(), 5000);
    }
}
