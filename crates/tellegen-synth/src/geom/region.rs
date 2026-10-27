//! Region shapes: GeoJSON polygons parsed once into a continuous longitude
//! frame, with containment, buffer, sampling and area queries.

use crate::geom::{haversine_km, longitude_arc_bounds, ring_area_km2};
use crate::report::BBox;
use crate::rng::Stream;

/// A ring of longitude/latitude vertices (closed: first == last).
pub type Ring = Vec<(f64, f64)>;

/// One polygon: the outer ring first, holes after, each with its bbox for a
/// cheap reject before the ray cast.
#[derive(Clone, Debug)]
pub struct Polygon {
    pub rings: Vec<Ring>,
    pub ring_bboxes: Vec<BBox>,
    /// Spherical area of the outer ring minus its holes, km².
    pub area_km2: f64,
}

impl Polygon {
    fn new(rings: Vec<Ring>) -> Option<Self> {
        if rings.is_empty() || rings[0].len() < 3 {
            return None;
        }
        let ring_bboxes = rings.iter().map(|r| ring_bbox(r)).collect();
        let mut area = ring_area_km2(&rings[0]);
        for hole in &rings[1..] {
            area -= ring_area_km2(hole);
        }
        Some(Self {
            rings,
            ring_bboxes,
            area_km2: area.max(0.0),
        })
    }

    pub fn bbox(&self) -> &BBox {
        &self.ring_bboxes[0]
    }

    /// Inside the outer ring and outside every hole.
    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        if !point_in_ring(lon, lat, &self.rings[0], &self.ring_bboxes[0]) {
            return false;
        }
        for (hole, bb) in self.rings[1..].iter().zip(&self.ring_bboxes[1..]) {
            if point_in_ring(lon, lat, hole, bb) {
                return false;
            }
        }
        true
    }

    pub fn outer_centroid(&self) -> (f64, f64) {
        let r = &self.rings[0];
        let n = r.len() as f64;
        (
            r.iter().map(|p| p.0).sum::<f64>() / n,
            r.iter().map(|p| p.1).sum::<f64>() / n,
        )
    }

    fn shift_east(&mut self) {
        for ring in &mut self.rings {
            for p in ring.iter_mut() {
                p.0 += 360.0;
            }
        }
        for bb in &mut self.ring_bboxes {
            bb.west += 360.0;
            bb.east += 360.0;
        }
    }
}

fn ring_bbox(ring: &[(f64, f64)]) -> BBox {
    let mut bb = BBox {
        west: f64::INFINITY,
        south: f64::INFINITY,
        east: f64::NEG_INFINITY,
        north: f64::NEG_INFINITY,
    };
    for &(lon, lat) in ring {
        bb.west = bb.west.min(lon);
        bb.east = bb.east.max(lon);
        bb.south = bb.south.min(lat);
        bb.north = bb.north.max(lat);
    }
    bb
}

/// Ray casting with a bbox reject: count the ring edges the eastward ray from
/// the point crosses; odd means inside.
pub fn point_in_ring(lon: f64, lat: f64, ring: &[(f64, f64)], bb: &BBox) -> bool {
    if ring.len() < 3 || lon < bb.west || lon > bb.east || lat < bb.south || lat > bb.north {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if (yi > lat) != (yj > lat) {
            let edge_lon = (xj - xi) * (lat - yi) / (yj - yi) + xi;
            if lon < edge_lon {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// The region in one continuous longitude frame. When the region straddles
/// the antimeridian, `frame.east` exceeds 180 and the western polygons are
/// shifted east by 360 so every polygon, the frame and every sampled point
/// share one frame. Longitudes are wrapped back only on output.
#[derive(Clone, Debug)]
pub struct RegionShape {
    pub polygons: Vec<Polygon>,
    pub frame: BBox,
    pub area_km2: f64,
}

/// Parse a GeoJSON geometry into rings. Errors name the problem for a 400.
pub fn geometry_polygons(geometry: &serde_json::Value) -> Result<Vec<Vec<Ring>>, String> {
    let obj = geometry
        .as_object()
        .ok_or_else(|| "region geometry must be a GeoJSON geometry object".to_owned())?;
    let gtype = obj
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "region geometry requires 'type' and 'coordinates'".to_owned())?;
    let coords = obj
        .get("coordinates")
        .ok_or_else(|| "region geometry requires 'type' and 'coordinates'".to_owned())?;
    match gtype {
        "Polygon" => Ok(vec![parse_rings(coords)?]),
        "MultiPolygon" => coords
            .as_array()
            .ok_or_else(|| "MultiPolygon coordinates must be an array".to_owned())?
            .iter()
            .map(parse_rings)
            .collect(),
        other => Err(format!(
            "region geometry type must be Polygon or MultiPolygon, got {other}"
        )),
    }
}

fn parse_rings(raw: &serde_json::Value) -> Result<Vec<Ring>, String> {
    let rings = raw
        .as_array()
        .ok_or_else(|| "polygon coordinates must be an array of rings".to_owned())?;
    rings
        .iter()
        .map(|ring| {
            let pts = ring
                .as_array()
                .ok_or_else(|| "a ring must be an array of positions".to_owned())?;
            let ring: Ring = pts
                .iter()
                .map(|p| {
                    let p = p
                        .as_array()
                        .filter(|p| p.len() >= 2)
                        .ok_or_else(|| "a position must be [lon, lat]".to_owned())?;
                    let lon = p[0].as_f64().ok_or("longitude must be a number")?;
                    let lat = p[1].as_f64().ok_or("latitude must be a number")?;
                    if !(lon.is_finite() && lat.is_finite()) {
                        return Err("coordinates must be finite".to_owned());
                    }
                    if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
                        return Err(format!("coordinate out of range (lon={lon}, lat={lat})"));
                    }
                    Ok((lon, lat))
                })
                .collect::<Result<_, String>>()?;
            check_antimeridian(&ring)?;
            Ok(ring)
        })
        .collect()
}

/// Refuse a ring edge that jumps more than 180 degrees of longitude. Such a
/// ring was written across the antimeridian the short way, and read as
/// GeoJSON it spans the planet the long way round. Edges along a pole are
/// exempt, since a ring around a pole (Antarctica) runs from 180 to -180
/// there.
fn check_antimeridian(ring: &Ring) -> Result<(), String> {
    let closing = ring.last().zip(ring.first());
    for (a, b) in ring.windows(2).map(|w| (&w[0], &w[1])).chain(closing) {
        let polar = a.1.abs() >= 89.99 && b.1.abs() >= 89.99;
        if (a.0 - b.0).abs() > 180.0 && !polar {
            return Err(format!(
                "ring crosses the antimeridian between lon {} and {}: split it at \
                 +/-180 as RFC 7946 asks, or give the region as a bounding box with \
                 east > 180",
                a.0, b.0
            ));
        }
    }
    Ok(())
}

/// Reject geometry no stage could use.
pub fn validate_geometry(geometry: &serde_json::Value) -> Result<(), String> {
    let polygons = geometry_polygons(geometry)?;
    if polygons
        .iter()
        .any(|poly| !poly.is_empty() && poly[0].len() >= 3)
    {
        Ok(())
    } else {
        Err("region geometry has no usable polygon ring".to_owned())
    }
}

impl RegionShape {
    /// Build the shape; `Err` when no polygon has a usable outer ring.
    pub fn from_geometry(geometry: &serde_json::Value) -> Result<Self, String> {
        let raw = geometry_polygons(geometry)?;
        let mut polygons: Vec<Polygon> = raw.into_iter().filter_map(Polygon::new).collect();
        if polygons.is_empty() {
            return Err("region geometry has no usable polygon ring".to_owned());
        }
        let intervals: Vec<(f64, f64)> = polygons
            .iter()
            .map(|p| (p.bbox().west, p.bbox().east))
            .collect();
        let (west, east) = longitude_arc_bounds(&intervals);
        let south = polygons
            .iter()
            .map(|p| p.bbox().south)
            .fold(f64::INFINITY, f64::min);
        let north = polygons
            .iter()
            .map(|p| p.bbox().north)
            .fold(f64::NEG_INFINITY, f64::max);
        if east > 180.0 {
            for poly in &mut polygons {
                if poly.bbox().west < west {
                    poly.shift_east();
                }
            }
        }
        let area_km2 = polygons.iter().map(|p| p.area_km2).sum();
        Ok(Self {
            polygons,
            frame: BBox {
                west,
                south,
                east,
                north,
            },
            area_km2,
        })
    }

    pub fn crosses_antimeridian(&self) -> bool {
        self.frame.east > 180.0
    }

    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        self.polygons.iter().any(|p| p.contains(lon, lat))
    }

    /// Index of the polygon containing the point.
    pub fn polygon_index(&self, lon: f64, lat: f64) -> Option<usize> {
        self.polygons.iter().position(|p| p.contains(lon, lat))
    }

    /// Inside the region or within `buffer_km` of an outer ring. The buffer
    /// absorbs coarse borders so a real near-border substation is kept.
    pub fn contains_or_near(&self, lon: f64, lat: f64, buffer_km: f64) -> bool {
        if self.contains(lon, lat) {
            return true;
        }
        if buffer_km <= 0.0 {
            return false;
        }
        for poly in &self.polygons {
            let bb = poly.bbox();
            let margin = buffer_km / 111.0 * 1.5;
            if lon < bb.west - margin / lat.to_radians().cos().abs().max(0.05)
                || lon > bb.east + margin / lat.to_radians().cos().abs().max(0.05)
                || lat < bb.south - margin
                || lat > bb.north + margin
            {
                continue;
            }
            let ring = &poly.rings[0];
            for w in ring.windows(2) {
                if point_segment_km(lon, lat, w[0], w[1]) <= buffer_km {
                    return true;
                }
            }
        }
        false
    }

    /// A point inside the region within `max_km` of a point just outside it:
    /// the nearest point of an outer ring, stepped a few metres inward.
    /// `None` when no outer ring comes that close.
    pub fn inside_near(&self, lon: f64, lat: f64, max_km: f64) -> Option<(f64, f64)> {
        let margin_lat = max_km / 111.0;
        let margin_lon = margin_lat / lat.to_radians().cos().abs().max(0.05);
        // The nearest point and the two ends of its segment.
        let mut best: Option<[(f64, f64); 3]> = None;
        let mut best_km = f64::INFINITY;
        for poly in &self.polygons {
            let bb = poly.bbox();
            if lon < bb.west - margin_lon
                || lon > bb.east + margin_lon
                || lat < bb.south - margin_lat
                || lat > bb.north + margin_lat
            {
                continue;
            }
            for w in poly.rings[0].windows(2) {
                let (d, q) = nearest_on_segment(lon, lat, w[0], w[1]);
                if d <= max_km && d < best_km {
                    best = Some([q, w[0], w[1]]);
                    best_km = d;
                }
            }
        }
        let [q, a, b] = best?;
        // Step off the segment along its normal, on whichever side is inside.
        let k = q.1.to_radians().cos().abs().max(0.05);
        let (dx, dy) = ((b.0 - a.0) * k, b.1 - a.1);
        let len = dx.hypot(dy).max(1e-12);
        let (nx, ny) = (-dy / len / k, dx / len);
        for step in [1e-4, 5e-4, 2e-3, 1e-2] {
            for side in [1.0, -1.0] {
                let p = (q.0 + side * nx * step, q.1 + side * ny * step);
                if self.contains(p.0, p.1) {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Whether a polyline stays inside the region: eight samples per segment,
    /// enough to tag a corridor that leaves the selection.
    pub fn polyline_inside(&self, line: &[(f64, f64)]) -> bool {
        for w in line.windows(2) {
            for k in 0..8 {
                let t = k as f64 / 7.0;
                let lon = w[0].0 + (w[1].0 - w[0].0) * t;
                let lat = w[0].1 + (w[1].1 - w[0].1) * t;
                if !self.contains(lon, lat) {
                    return false;
                }
            }
        }
        true
    }

    /// Spherical area in km² of each polygon (outer minus holes).
    pub fn polygon_areas(&self) -> Vec<f64> {
        self.polygons.iter().map(|p| p.area_km2).collect()
    }

    /// A point inside the region, in the frame. Three tiers: a polygon chosen
    /// with probability proportional to area (16 tries of rejection sampling
    /// in its own bbox), then the whole frame (900 tries), then the first
    /// polygon's centroid. Polygons listed in `skip` are never chosen.
    pub fn sample_point(&self, rng: &mut Stream, weights: &[f64]) -> (f64, f64) {
        for _ in 0..16 {
            let Some(idx) = rng.weighted_index(weights) else {
                break;
            };
            let poly = &self.polygons[idx];
            if let Some(p) = sample_polygon_point(poly, rng) {
                if self.contains(p.0, p.1) {
                    return p;
                }
            }
        }
        let f = &self.frame;
        for _ in 0..900 {
            let lon = f.west + rng.uniform() * (f.east - f.west);
            let lat = f.south + rng.uniform() * (f.north - f.south);
            if self.contains(lon, lat) {
                return (lon, lat);
            }
        }
        self.polygons[0].outer_centroid()
    }
}

fn sample_polygon_point(poly: &Polygon, rng: &mut Stream) -> Option<(f64, f64)> {
    let bb = poly.bbox();
    for _ in 0..300 {
        let lon = bb.west + rng.uniform() * (bb.east - bb.west);
        let lat = bb.south + rng.uniform() * (bb.north - bb.south);
        if poly.contains(lon, lat) {
            return Some((lon, lat));
        }
    }
    Some(poly.outer_centroid())
}

/// Planar point-to-segment distance in km with a cos(lat) longitude
/// correction: adequate for a border buffer of a few km.
pub fn point_segment_km(plon: f64, plat: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    nearest_on_segment(plon, plat, a, b).0
}

/// The point of segment `a`-`b` nearest `(plon, plat)` and its distance in
/// km, with the same planar cos(lat) correction as [`point_segment_km`].
fn nearest_on_segment(plon: f64, plat: f64, a: (f64, f64), b: (f64, f64)) -> (f64, (f64, f64)) {
    let k = plat.to_radians().cos().abs().max(1e-6);
    let (px, py) = (plon * k, plat);
    let (ax, ay) = (a.0 * k, a.1);
    let (bx, by) = (b.0 * k, b.1);
    let (dx, dy) = (bx - ax, by - ay);
    let t = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0)
    };
    let (cx, cy) = (ax + t * dx, ay + t * dy);
    (
        ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() * 111.0,
        (cx / k, cy),
    )
}

/// Great-circle distance between two frame points (longitudes may exceed 180).
pub fn frame_distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    haversine_km(a.0, a.1, b.0, b.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{Stream, StreamTag};

    fn polygon(ring: &[(f64, f64)]) -> serde_json::Value {
        serde_json::json!({ "type": "Polygon", "coordinates": [ring.iter().map(|p| [p.0, p.1]).collect::<Vec<_>>()] })
    }

    #[test]
    fn holes_exclude_points() {
        let geom = serde_json::json!({
            "type": "Polygon",
            "coordinates": [
                [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]],
                [[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0], [4.0, 4.0]],
            ]
        });
        let shape = RegionShape::from_geometry(&geom).unwrap();
        assert!(shape.contains(1.0, 1.0));
        assert!(!shape.contains(5.0, 5.0));
        assert!(!shape.contains(11.0, 5.0));
        let mut rng = Stream::new(1, StreamTag::Site);
        let w = shape.polygon_areas();
        for _ in 0..200 {
            let (lon, lat) = shape.sample_point(&mut rng, &w);
            assert!(shape.contains(lon, lat));
            assert!(!(4.0..=6.0).contains(&lon) || !(4.0..=6.0).contains(&lat));
        }
    }

    #[test]
    fn antimeridian_frame_is_continuous() {
        let geom = serde_json::json!({
            "type": "MultiPolygon",
            "coordinates": [
                [[[160.0, 60.0], [180.0, 60.0], [180.0, 70.0], [160.0, 70.0], [160.0, 60.0]]],
                [[[-180.0, 60.0], [-170.0, 60.0], [-170.0, 70.0], [-180.0, 70.0], [-180.0, 60.0]]],
            ]
        });
        let shape = RegionShape::from_geometry(&geom).unwrap();
        assert!(shape.crosses_antimeridian());
        assert!(shape.frame.east > 180.0);
        assert!(shape.frame.east - shape.frame.west < 60.0);
        assert!(shape.contains(185.0, 65.0));
        assert!(shape.contains(170.0, 65.0));
        assert!(!shape.contains(150.0, 65.0));
    }

    #[test]
    fn buffer_and_polyline_tests() {
        let shape =
            RegionShape::from_geometry(&polygon(&[(0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (0.0, 0.0)]))
                .unwrap();
        assert!(!shape.polyline_inside(&[(0.4, 0.4), (3.6, 3.6)]));
        assert!(shape.polyline_inside(&[(0.4, 0.4), (1.0, 1.0)]));
        assert!(!shape.contains(2.0, -0.1));
        assert!(shape.contains_or_near(2.0, -0.1, 20.0));
        assert!(!shape.contains_or_near(2.0, -0.1, 5.0));
        assert!(!shape.contains_or_near(4.5, 0.5, 20.0));
    }

    #[test]
    fn malformed_geometry_is_rejected() {
        assert!(validate_geometry(&serde_json::json!({"type": "Polygon"})).is_err());
        assert!(validate_geometry(
            &serde_json::json!({"type": "GeometryCollection", "coordinates": []})
        )
        .is_err());
        assert!(validate_geometry(
            &serde_json::json!({"type": "Point", "coordinates": [1.0, 2.0]})
        )
        .is_err());
        assert!(
            validate_geometry(&polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)])).is_ok()
        );
    }
}
