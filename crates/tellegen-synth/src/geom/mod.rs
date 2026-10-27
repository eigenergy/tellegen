//! Spherical and planar geometry primitives: great-circle distance, a local
//! equal-area projection for neighbourhood structure, longitude frames that
//! stay continuous across the antimeridian, and polygon area on the sphere.

pub mod bluenoise;
pub mod delaunay;
pub mod region;

use crate::report::BBox;

/// Mean Earth radius, km. source: IUGG 1980 mean radius 6371.0 km.
pub const EARTH_RADIUS_KM: f64 = 6371.0;

/// Great-circle distance in km between two longitude/latitude points.
pub fn haversine_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().min(1.0).asin()
}

/// Bring a continuous-frame longitude back into `[-180, 180]`.
pub fn wrap_lon(lon: f64) -> f64 {
    if lon > 180.0 {
        lon - 360.0
    } else if lon < -180.0 {
        lon + 360.0
    } else {
        lon
    }
}

/// Lift a stored longitude into a frame whose east edge may exceed 180.
pub fn lift_lon(lon: f64, frame: &BBox) -> f64 {
    if frame.east > 180.0 && lon < frame.west {
        lon + 360.0
    } else {
        lon
    }
}

/// Longitude bounds across sub-polygon intervals, antimeridian aware. Each
/// interval has `west <= east` in `[-180, 180]`. The intervals are merged on
/// the circle; the widest uncovered gap is the empty arc and the bounding arc
/// is its complement. When that gap is interior the arc wraps the seam and
/// `east` is returned above 180 so `west < east` still holds.
pub fn longitude_arc_bounds(intervals: &[(f64, f64)]) -> (f64, f64) {
    if intervals.is_empty() {
        return (0.0, 0.0);
    }
    let mut sorted = intervals.to_vec();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for (w, e) in sorted {
        match spans.last_mut() {
            Some(last) if w <= last.1 => last.1 = last.1.max(e),
            _ => spans.push((w, e)),
        }
    }
    if spans.len() == 1 {
        return spans[0];
    }
    let (mut west, mut east) = (spans[0].0, spans[spans.len() - 1].1);
    let mut best_gap = (spans[0].0 + 360.0) - spans[spans.len() - 1].1;
    for k in 0..spans.len() - 1 {
        let gap = spans[k + 1].0 - spans[k].1;
        if gap > best_gap {
            best_gap = gap;
            west = spans[k + 1].0;
            east = spans[k].1 + 360.0;
        }
    }
    (west, east)
}

/// Area of a closed ring on the sphere in km², by the signed spherical excess
/// approximation Σ (λ₂ − λ₁)(2 + sin φ₁ + sin φ₂) · R² / 2 (Chamberlain &
/// Duquette 2007, "Some algorithms for polygons on a sphere"). Absolute value,
/// so ring orientation does not matter.
pub fn ring_area_km2(ring: &[(f64, f64)]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let n = ring.len();
    for i in 0..n {
        let (lon1, lat1) = ring[i];
        let (lon2, lat2) = ring[(i + 1) % n];
        sum +=
            (lon2 - lon1).to_radians() * (2.0 + lat1.to_radians().sin() + lat2.to_radians().sin());
    }
    (sum * EARTH_RADIUS_KM * EARTH_RADIUS_KM / 2.0).abs()
}

/// A local projection for neighbourhood structure (Delaunay, blue noise):
/// sinusoidal about the frame's central meridian, so distances near any point
/// are true to a few percent and the map is equal-area. Units: km.
#[derive(Clone, Copy, Debug)]
pub struct Projection {
    lon0: f64,
}

impl Projection {
    pub fn for_frame(frame: &BBox) -> Self {
        Self {
            lon0: (frame.west + frame.east) / 2.0,
        }
    }

    pub fn project(&self, lon: f64, lat: f64) -> (f64, f64) {
        let k = EARTH_RADIUS_KM.to_radians();
        let x = (lon - self.lon0) * lat.to_radians().cos() * k;
        let y = lat * k;
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_known_distances() {
        // Paris to London, about 344 km.
        let d = haversine_km(2.3522, 48.8566, -0.1276, 51.5072);
        assert!((d - 344.0).abs() < 3.0, "{d}");
        assert_eq!(haversine_km(10.0, 10.0, 10.0, 10.0), 0.0);
    }

    #[test]
    fn arc_bounds_cases() {
        assert_eq!(longitude_arc_bounds(&[(-5.0, 9.0)]), (-5.0, 9.0));
        assert_eq!(longitude_arc_bounds(&[(0.0, 2.0), (6.0, 8.0)]), (0.0, 8.0));
        assert_eq!(longitude_arc_bounds(&[(-180.0, 180.0)]), (-180.0, 180.0));
        assert_eq!(
            longitude_arc_bounds(&[(160.0, 180.0), (-180.0, -170.0)]),
            (160.0, 190.0)
        );
    }

    #[test]
    fn ring_area_of_a_degree_square_at_the_equator() {
        let ring = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)];
        let a = ring_area_km2(&ring);
        // 111.2 km × 111.2 km, slightly less from the cos(lat) taper.
        assert!((a - 12_300.0).abs() < 150.0, "{a}");
        assert_eq!(wrap_lon(190.0), -170.0);
        assert_eq!(wrap_lon(-190.0), 170.0);
    }
}
