//! OpenStreetMap (Overpass) mapping: the query builder (so the QL text
//! exists once, in the engine), the document and tag parsers, and the grid
//! build the session drives stage by stage.

pub mod build;
pub mod parse;

pub use build::{OsmBuild, OsmCounts};
pub use parse::{parse_document, ParsedDocument};

use crate::error::SynthError;
use crate::geom::region::RegionShape;
use crate::report::BBox;
use crate::request::SynthRequest;

/// Maximum raw response size. Applied before JSON parsing to reject global extracts.
pub const MAX_OSM_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;

/// Maximum element count accepted from Overpass.
pub const MAX_OSM_ELEMENTS: usize = 500_000;

/// Server-side time limit stated in every query, seconds. A regional query
/// that needs longer is better split than waited on.
pub const OVERPASS_TIMEOUT_S: u32 = 60;

/// Kilometres per degree of latitude (mean Earth radius 6371 km).
const KM_PER_DEGREE: f64 = 111.195;

/// Overpass QL for the request's region: one query per bounding box, two when
/// the region crosses the antimeridian (split at ±180°). Each query fetches
/// substations, lines and cables, plants, generators rated in MW or GW (the
/// rooftop units, most of them, are left out), and converters, with centres
/// for areas and full geometry for lines. The box is the
/// region's frame widened by the OSM clip buffer, so infrastructure the clip
/// keeps is fetched.
pub fn overpass_queries(request: &SynthRequest) -> Result<Vec<String>, SynthError> {
    let shape = RegionShape::from_geometry(&request.region.geometry).map_err(SynthError::Region)?;
    let buffer_km = request
        .osm
        .as_ref()
        .map(|o| o.clip_buffer_km)
        .unwrap_or_else(|| crate::request::OsmOptions::default().clip_buffer_km);
    let f = shape.frame;
    let dlat = buffer_km / KM_PER_DEGREE;
    let mid = (0.5 * (f.south + f.north)).to_radians().cos().max(0.05);
    let dlon = buffer_km / (KM_PER_DEGREE * mid);
    let south = (f.south - dlat).max(-90.0);
    let north = (f.north + dlat).min(90.0);
    let (west, east) = (f.west - dlon, f.east + dlon);
    let boxes = if east - west >= 360.0 {
        vec![BBox {
            west: -180.0,
            south,
            east: 180.0,
            north,
        }]
    } else if east > 180.0 {
        vec![
            BBox {
                west,
                south,
                east: 180.0,
                north,
            },
            BBox {
                west: -180.0,
                south,
                east: east - 360.0,
                north,
            },
        ]
    } else if west < -180.0 {
        vec![
            BBox {
                west: west + 360.0,
                south,
                east: 180.0,
                north,
            },
            BBox {
                west: -180.0,
                south,
                east,
                north,
            },
        ]
    } else {
        vec![BBox {
            west,
            south,
            east,
            north,
        }]
    };
    Ok(boxes.iter().map(overpass_ql).collect())
}

/// One Overpass QL query for `bbox` (`south,west,north,east`, six decimals).
fn overpass_ql(bbox: &BBox) -> String {
    let b = format!(
        "({:.6},{:.6},{:.6},{:.6})",
        bbox.south, bbox.west, bbox.north, bbox.east
    );
    format!(
        "[out:json][timeout:{OVERPASS_TIMEOUT_S}];\n\
         (\n  node[\"power\"=\"substation\"]{b};\n  way[\"power\"=\"substation\"]{b};\n  relation[\"power\"=\"substation\"]{b};\n)->.subs;\n\
         (\n  way[\"power\"=\"line\"]{b};\n  way[\"power\"=\"cable\"]{b};\n)->.lines;\n\
         (\n  node[\"power\"=\"plant\"]{b};\n  way[\"power\"=\"plant\"]{b};\n  relation[\"power\"=\"plant\"]{b};\n  node[\"power\"=\"generator\"][\"generator:output:electricity\"~\"[0-9] *[mg]w\",i]{b};\n  way[\"power\"=\"generator\"][\"generator:output:electricity\"~\"[0-9] *[mg]w\",i]{b};\n)->.plants;\n\
         (\n  node[\"power\"=\"converter\"]{b};\n  way[\"power\"=\"converter\"]{b};\n  relation[\"power\"=\"converter\"]{b};\n)->.converters;\n\
         .subs out center tags;\n.lines out geom tags;\n.plants out center tags;\n.converters out center tags;\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Region;

    #[test]
    fn one_query_per_box_and_two_across_the_antimeridian() {
        let req = SynthRequest::new(Region::bbox(-9.6, 36.9, -6.2, 42.2, "Portugal"));
        let q = overpass_queries(&req).unwrap();
        assert_eq!(q.len(), 1);
        assert!(q[0].starts_with("[out:json][timeout:60];"));
        assert!(q[0].contains("way[\"power\"=\"line\"](36.85"));
        assert!(q[0].contains(".lines out geom tags;"));
        assert!(q[0].contains(
            "node[\"power\"=\"generator\"][\"generator:output:electricity\"~\"[0-9] *[mg]w\",i](36.85"
        ));
        let req = SynthRequest::new(Region::bbox(176.0, -19.5, 182.0, -15.5, "Fiji"));
        let q = overpass_queries(&req).unwrap();
        assert_eq!(q.len(), 2);
        assert!(q[0].contains(",180.000000)"));
        assert!(q[1].contains("(-19.544966,-180.000000,"));
    }
}
