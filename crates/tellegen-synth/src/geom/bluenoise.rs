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
