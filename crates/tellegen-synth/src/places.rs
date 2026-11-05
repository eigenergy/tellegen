//! Populated places (GeoNames `cities5000`, CC BY 4.0: every place of 5,000
//! people or more), trimmed by `scripts/derive-populated-places.py` to rows
//! of `[name, lon, lat, population, country]` and, in the United States, the
//! ISO 3166-2 state code. They anchor buses on real towns and carry the
//! population signal that sizes and allocates load (through each country's
//! or state's demand, see [`crate::demand`]). GeoNames populations are those
//! of each place itself, so a region's places sum to about its population.
//! Parsed once per process and shared by sessions. The earlier table layout,
//! `{n, lo, la, p, c}` objects (Natural Earth places), still parses.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::SynthError;
use crate::geom::haversine_km;
use crate::geom::region::RegionShape;
use crate::report::BBox;

/// How far outside a region's polygons a place may lie and still belong to
/// it, km, when the region already holds places of the same country. The
/// place moves onto the region's edge. source: Natural Earth's 1:10m places
/// measured against its own admin-0 polygons, which treat harbours and
/// estuaries narrower than their generalization as water: at 1:50m, New York
/// lies 1.9 km outside the United States, Lagos 1.6 km outside Nigeria and
/// Lisbon 1.6 km outside Portugal; at 1:110m (the MapToGrid selection layer),
/// Durban lies 5.3 km, São Luís 10.1 km and Dalian 12.4 km outside. 15 km keeps
/// such cities in their country and stays below the 25 km a transmission
/// substation serves (`LOAD_IDW_RADIUS_KM`).
pub const PLACE_SNAP_KM: f64 = 15.0;

/// Share of a country's place population a region must hold, counting the
/// places within [`PLACE_SNAP_KM`] outside it, before those places snap in.
/// The snap repairs a country outline's generalised coast; a region holding
/// less of the country is a drawn box or a part of it, whose edge is a real
/// cut through land. Modelling choice, no source: a whole-country selection
/// holds nearly all of its country (Portugal at 1:50m holds 48% inside its
/// polygon and 95% with Lisbon, 1.6 km outside), a box around one
/// metropolitan area a few percent.
pub const PLACE_SNAP_COUNTRY_SHARE: f64 = 0.5;

/// Schema of the bundled table: the rows under `places`, beside the dataset
/// and the SHA-256 of the file they were derived from.
pub const SCHEMA: &str = "tellegen.synth-populated-places/2";
/// The earlier layout (object rows), still accepted.
pub const SCHEMA_V1: &str = "tellegen.synth-populated-places/1";

/// One populated place.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub lon: f64,
    pub lat: f64,
    /// Population; a negative unknown sentinel is clamped to 0.
    pub population: f64,
    pub iso_a3: String,
    /// ISO 3166-2 code of the first-level subdivision ("US-CA"), where the
    /// table states one.
    pub subdivision: Option<String>,
}

/// A row as an object (`{n, lo, la, p, c, s?}`) or a positional array
/// (`[n, lo, la, p, c, s?]`).
#[derive(Deserialize)]
#[serde(untagged)]
enum Row {
    Object {
        n: String,
        lo: f64,
        la: f64,
        p: f64,
        #[serde(default)]
        c: String,
        #[serde(default)]
        s: Option<String>,
    },
    Array(Vec<serde_json::Value>),
}

/// A row's fields: name, lon, lat, population, country, subdivision.
type Fields = (String, f64, f64, f64, String, Option<String>);

impl Row {
    fn fields(self, i: usize) -> Result<Fields, SynthError> {
        match self {
            Row::Object { n, lo, la, p, c, s } => Ok((n, lo, la, p, c, s)),
            Row::Array(v) => {
                let bad = || {
                    SynthError::Places(format!(
                        "row {i} is not [name, lon, lat, population, country, subdivision?]"
                    ))
                };
                if !(5..=6).contains(&v.len()) {
                    return Err(bad());
                }
                let text = |k: usize| v[k].as_str().map(str::to_owned).ok_or_else(bad);
                let num = |k: usize| v[k].as_f64().ok_or_else(bad);
                let s = match v.get(5) {
                    Some(serde_json::Value::Null) | None => None,
                    Some(_) => Some(text(5)?),
                };
                Ok((text(0)?, num(1)?, num(2)?, num(3)?, text(4)?, s))
            }
        }
    }
}

/// The bundled table's layout.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    schema: String,
    source: String,
    #[allow(dead_code)]
    source_url: String,
    source_sha256: String,
    #[allow(dead_code)]
    #[serde(default)]
    country_info_sha256: Option<String>,
    #[allow(dead_code)]
    note: String,
    places: Vec<Row>,
}

/// The places table: rows sorted by population descending, plus a latitude
/// sorted index for window queries.
#[derive(Clone, Debug)]
pub struct Places {
    rows: Vec<Place>,
    /// Row indices sorted by latitude.
    by_lat: Vec<usize>,
    /// Population of every place of a country, by country code.
    country_population: BTreeMap<String, f64>,
    /// Population of every place of a subdivision, by ISO 3166-2 code.
    subdivision_population: BTreeMap<String, f64>,
    sha256: String,
    /// The dataset the rows were derived from and its SHA-256, when the table
    /// names them; a bare array of rows names neither.
    source: Option<(String, String)>,
}

/// A place positioned in a region's continuous longitude frame. `lon` may
/// differ from the place's stored longitude on a seam-crossing frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCity {
    pub lon: f64,
    pub lat: f64,
    pub place: Place,
    /// The place's share of demand: its peak MW once the context stage has
    /// sized demand, its population before. Load allocation reads only ratios.
    pub demand_weight: f64,
    /// The place lay just outside the region and was moved onto its edge
    /// ([`PLACE_SNAP_KM`]).
    pub snapped: bool,
}

impl Places {
    /// Parse the bundled table (`{schema, source, ..., places: [rows]}`) or a
    /// bare JSON array of rows.
    pub fn parse(bytes: &[u8]) -> Result<Self, SynthError> {
        let unreadable =
            |e: serde_json::Error| SynthError::Places(format!("cannot parse places table: {e}"));
        let bare = bytes
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .is_some_and(|b| *b == b'[');
        let (rows, source) = if bare {
            let rows: Vec<Row> = serde_json::from_slice(bytes).map_err(unreadable)?;
            (rows, None)
        } else {
            let table: Table = serde_json::from_slice(bytes).map_err(unreadable)?;
            if table.schema != SCHEMA && table.schema != SCHEMA_V1 {
                return Err(SynthError::Places(format!(
                    "places table schema {:?}, expected {SCHEMA:?}",
                    table.schema
                )));
            }
            (table.places, Some((table.source, table.source_sha256)))
        };
        let mut places = Vec::with_capacity(rows.len());
        for (i, row) in rows.into_iter().enumerate() {
            let (n, lo, la, p, c, sub) = row.fields(i)?;
            if !(-180.0..=180.0).contains(&lo) || !(-90.0..=90.0).contains(&la) {
                return Err(SynthError::Places(format!(
                    "row {i} has coordinates out of range (lon={lo}, lat={la})"
                )));
            }
            places.push(Place {
                name: n,
                lon: lo,
                lat: la,
                population: if p.is_finite() { p.max(0.0) } else { 0.0 },
                iso_a3: c,
                subdivision: sub.filter(|code| !code.is_empty()),
            });
        }
        places.sort_by(|a, b| {
            b.population
                .partial_cmp(&a.population)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| {
                    a.lon
                        .partial_cmp(&b.lon)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        let mut by_lat: Vec<usize> = (0..places.len()).collect();
        by_lat.sort_by(|&a, &b| {
            places[a]
                .lat
                .partial_cmp(&places[b].lat)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut country_population: BTreeMap<String, f64> = BTreeMap::new();
        let mut subdivision_population: BTreeMap<String, f64> = BTreeMap::new();
        for place in &places {
            *country_population.entry(place.iso_a3.clone()).or_default() += place.population;
            if let Some(code) = &place.subdivision {
                *subdivision_population.entry(code.clone()).or_default() += place.population;
            }
        }
        let digest = Sha256::digest(bytes);
        Ok(Self {
            rows: places,
            by_lat,
            country_population,
            subdivision_population,
            sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
            source,
        })
    }

    /// The bundled table (`data/populated_places.json`), for the CLI and
    /// offline tests. The wasm build fetches the same file beside the binary
    /// instead of embedding it.
    #[cfg(feature = "embedded-places")]
    pub fn embedded() -> Result<Self, SynthError> {
        Self::parse(include_bytes!("../data/populated_places.json"))
    }

    /// An empty table: every bus falls back to blue-noise siting.
    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            by_lat: Vec::new(),
            country_population: BTreeMap::new(),
            subdivision_population: BTreeMap::new(),
            sha256: Sha256::digest(b"")
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            source: None,
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// The dataset the rows were derived from, as the table names it.
    pub fn source(&self) -> Option<&str> {
        self.source.as_ref().map(|(source, _)| source.as_str())
    }

    /// SHA-256 of the dataset the rows were derived from.
    pub fn source_sha256(&self) -> Option<&str> {
        self.source.as_ref().map(|(_, sha256)| sha256.as_str())
    }

    pub fn rows(&self) -> &[Place] {
        &self.rows
    }

    /// Population of every place of each country, by the table's country
    /// code (places without one under the empty code).
    pub fn country_population(&self) -> &BTreeMap<String, f64> {
        &self.country_population
    }

    /// Population of every place of each subdivision, by ISO 3166-2 code.
    pub fn subdivision_population(&self) -> &BTreeMap<String, f64> {
        &self.subdivision_population
    }

    /// Row indices with latitude in `[south, north]`.
    fn lat_window(&self, south: f64, north: f64) -> &[usize] {
        let lo = self.by_lat.partition_point(|&i| self.rows[i].lat < south);
        let hi = self.by_lat.partition_point(|&i| self.rows[i].lat <= north);
        &self.by_lat[lo..hi.max(lo)]
    }

    /// Places inside the shape, in its frame, sorted by population descending.
    /// A place within [`PLACE_SNAP_KM`] outside it joins too, moved just
    /// inside its edge, when the shape holds places of its country and,
    /// with the places that snap, at least [`PLACE_SNAP_COUNTRY_SHARE`] of
    /// that country's place population.
    pub fn in_region(&self, shape: &RegionShape) -> Vec<PlacedCity> {
        let frame = &shape.frame;
        let seam = frame.east > 180.0;
        let margin_lat = PLACE_SNAP_KM / 111.0;
        let mut out = Vec::new();
        let mut near = Vec::new();
        for &i in self.lat_window(frame.south - margin_lat, frame.north + margin_lat) {
            let place = &self.rows[i];
            let margin_lon = margin_lat / place.lat.to_radians().cos().abs().max(0.05);
            let mut lon = place.lon;
            if seam && lon < frame.west - margin_lon {
                lon += 360.0;
            }
            if lon < frame.west - margin_lon || lon > frame.east + margin_lon {
                continue;
            }
            if shape.contains(lon, place.lat) {
                out.push(PlacedCity {
                    lon,
                    lat: place.lat,
                    demand_weight: place.population,
                    snapped: false,
                    place: place.clone(),
                });
            } else {
                near.push((i, lon));
            }
        }
        let mut held: BTreeMap<String, f64> = BTreeMap::new();
        for c in &out {
            *held.entry(c.place.iso_a3.clone()).or_default() += c.place.population;
        }
        let mut snaps = Vec::new();
        for (i, lon) in near {
            let place = &self.rows[i];
            if place.iso_a3.is_empty() || !held.contains_key(place.iso_a3.as_str()) {
                continue;
            }
            if let Some(at) = shape.inside_near(lon, place.lat, PLACE_SNAP_KM) {
                snaps.push((i, at));
            }
        }
        for &(i, _) in &snaps {
            let place = &self.rows[i];
            *held.entry(place.iso_a3.clone()).or_default() += place.population;
        }
        let countries: BTreeSet<String> = held
            .into_iter()
            .filter(|(code, pop)| {
                self.country_population
                    .get(code)
                    .is_some_and(|&total| *pop >= PLACE_SNAP_COUNTRY_SHARE * total)
            })
            .map(|(code, _)| code)
            .collect();
        for (i, (lon, lat)) in snaps {
            let place = &self.rows[i];
            if countries.contains(&place.iso_a3) {
                out.push(PlacedCity {
                    lon,
                    lat,
                    demand_weight: place.population,
                    snapped: true,
                    place: place.clone(),
                });
            }
        }
        out.sort_by(|a, b| {
            b.place
                .population
                .partial_cmp(&a.place.population)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.place.name.cmp(&b.place.name))
        });
        out
    }

    /// Places within `frame` (no polygon test), in its frame.
    pub fn in_frame(&self, frame: &BBox) -> Vec<PlacedCity> {
        let seam = frame.east > 180.0;
        let mut out = Vec::new();
        for &i in self.lat_window(frame.south, frame.north) {
            let place = &self.rows[i];
            let mut lon = place.lon;
            if seam && lon < frame.west {
                lon += 360.0;
            }
            if lon < frame.west || lon > frame.east {
                continue;
            }
            out.push(PlacedCity {
                lon,
                lat: place.lat,
                demand_weight: place.population,
                snapped: false,
                place: place.clone(),
            });
        }
        out
    }
}

/// Inverse-distance-weighted demand within `radius_km` of each point:
/// `Σ demand_weight / (1 + d)` over the cities in range. Cities are bucketed
/// on a `radius`-sized grid so the cost is O(points + cities).
/// Each place's demand split among the points within `radius_km` of it, by
/// inverse distance `1 / (1 + d)` normalised over those points, or given to
/// its nearest point when none is that close. Unlike [`demand_idw`], a place
/// counts once however many points surround it: a city ringed by many
/// substations shares its demand among them instead of lending all of it to
/// each, so totals follow where people live and not where buses are dense.
pub fn demand_partition(points: &[(f64, f64)], cities: &[PlacedCity], radius_km: f64) -> Vec<f64> {
    let mut out = vec![0.0; points.len()];
    if cities.is_empty() || points.is_empty() {
        return out;
    }
    let cell_lat = radius_km.max(1e-3) / 111.0;
    let key = |lon: f64, lat: f64| -> (i64, i64) {
        let cell_lon = cell_lat / lat.to_radians().cos().abs().max(0.05);
        (
            (lon / cell_lon).floor() as i64,
            (lat / cell_lat).floor() as i64,
        )
    };
    let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (k, &(lon, lat)) in points.iter().enumerate() {
        buckets.entry(key(lon, lat)).or_default().push(k);
    }
    for c in cities {
        let (cx, cy) = key(c.lon, c.lat);
        let mut near: Vec<(usize, f64)> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(list) = buckets.get(&(cx + dx, cy + dy)) {
                    for &k in list {
                        let d = haversine_km(c.lon, c.lat, points[k].0, points[k].1);
                        if d <= radius_km {
                            near.push((k, 1.0 / (1.0 + d)));
                        }
                    }
                }
            }
        }
        if near.is_empty() {
            let nearest = (0..points.len())
                .min_by(|&a, &b| {
                    haversine_km(c.lon, c.lat, points[a].0, points[a].1)
                        .total_cmp(&haversine_km(c.lon, c.lat, points[b].0, points[b].1))
                        .then(a.cmp(&b))
                })
                .expect("a point");
            out[nearest] += c.demand_weight;
            continue;
        }
        let total: f64 = near.iter().map(|(_, w)| w).sum();
        for (k, w) in near {
            out[k] += c.demand_weight * w / total;
        }
    }
    out
}

pub fn demand_idw(points: &[(f64, f64)], cities: &[PlacedCity], radius_km: f64) -> Vec<f64> {
    let mut out = vec![0.0; points.len()];
    if cities.is_empty() || points.is_empty() {
        return out;
    }
    let cell_lat = radius_km / 111.0;
    let key = |lon: f64, lat: f64| -> (i64, i64) {
        let cell_lon = cell_lat / lat.to_radians().cos().abs().max(0.05);
        (
            (lon / cell_lon).floor() as i64,
            (lat / cell_lat).floor() as i64,
        )
    };
    let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, c) in cities.iter().enumerate() {
        buckets.entry(key(c.lon, c.lat)).or_default().push(i);
    }
    for (k, &(lon, lat)) in points.iter().enumerate() {
        let (cx, cy) = key(lon, lat);
        let mut w = 0.0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(list) = buckets.get(&(cx + dx, cy + dy)) {
                    for &i in list {
                        let c = &cities[i];
                        let d = haversine_km(lon, lat, c.lon, c.lat);
                        if d <= radius_km {
                            w += c.demand_weight / (1.0 + d);
                        }
                    }
                }
            }
        }
        out[k] = w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/places_small.json");

    #[test]
    fn parses_and_sorts_by_population() {
        let places = Places::parse(FIXTURE).unwrap();
        assert!(places.len() > 100);
        assert_eq!(places.rows()[0].name, "Paris");
        assert!(places
            .rows()
            .windows(2)
            .all(|w| w[0].population >= w[1].population));
        assert_eq!(places.sha256().len(), 64);
    }

    #[test]
    fn the_bundled_table_names_its_source_and_bare_rows_name_none() {
        let bundled = Places::parse(include_bytes!("../data/populated_places.json")).unwrap();
        assert_eq!(bundled.len(), 69_999);
        assert!(bundled.source().unwrap().starts_with("GeoNames cities5000"));
        assert_eq!(bundled.source_sha256().unwrap().len(), 64);
        let bare = Places::parse(FIXTURE).unwrap();
        assert_eq!((bare.source(), bare.source_sha256()), (None, None));
        let table = |schema: &str, extra: &str| {
            format!(
                r#"{{"schema":"{schema}","source":"s","source_url":"u","source_sha256":"x","note":"n","places":[]{extra}}}"#
            )
        };
        assert!(Places::parse(table(SCHEMA, "").as_bytes()).is_ok());
        for refused in [table("other/1", ""), table(SCHEMA, r#","extra":1"#)] {
            assert!(
                matches!(
                    Places::parse(refused.as_bytes()),
                    Err(SynthError::Places(_))
                ),
                "{refused}"
            );
        }
    }

    #[test]
    fn places_just_outside_a_country_outline_snap_in_and_a_cut_through_land_does_not() {
        // AAA's largest place lies 1.1 km past the east edge (a harbour the
        // outline leaves out) and a small one 33 km past it. BBB's place
        // 5.5 km past the west edge belongs to a country the square holds
        // almost none of, so the edge is a cut through land.
        let places = Places::parse(
            br#"[{"n":"Inland","lo":0.5,"la":0.5,"p":1000000,"c":"AAA"},
                 {"n":"Harbour","lo":1.01,"la":0.5,"p":2000000,"c":"AAA"},
                 {"n":"Far","lo":1.3,"la":0.5,"p":100,"c":"AAA"},
                 {"n":"Border","lo":0.2,"la":0.2,"p":100,"c":"BBB"},
                 {"n":"Across","lo":-0.05,"la":0.2,"p":100,"c":"BBB"},
                 {"n":"Capital","lo":10.0,"la":10.0,"p":1000000,"c":"BBB"}]"#,
        )
        .unwrap();
        let geom = serde_json::json!({"type":"Polygon","coordinates":[[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0]]]});
        let shape = RegionShape::from_geometry(&geom).unwrap();
        let cities = places.in_region(&shape);
        let names: Vec<&str> = cities.iter().map(|c| c.place.name.as_str()).collect();
        assert_eq!(names, ["Harbour", "Inland", "Border"]);
        let harbour = &cities[0];
        assert!(harbour.snapped && !cities[1].snapped && !cities[2].snapped);
        assert!(shape.contains(harbour.lon, harbour.lat));
        assert!(haversine_km(harbour.lon, harbour.lat, 1.01, 0.5) < PLACE_SNAP_KM);
        assert_eq!(places.country_population()["AAA"], 3_000_100.0);
    }

    #[test]
    fn region_query_finds_baghdad_and_respects_the_frame() {
        let places = Places::parse(FIXTURE).unwrap();
        let geom = serde_json::json!({"type":"Polygon","coordinates":[[[42.1,31.1],[48.5,31.1],[48.5,36.8],[42.1,36.8],[42.1,31.1]]]});
        let shape = RegionShape::from_geometry(&geom).unwrap();
        let cities = places.in_region(&shape);
        assert_eq!(cities[0].place.name, "Baghdad");
        assert!(cities.iter().all(|c| shape.contains(c.lon, c.lat)));

        let seam = serde_json::json!({"type":"MultiPolygon","coordinates":[
            [[[170.0,-20.0],[180.0,-20.0],[180.0,-15.0],[170.0,-15.0],[170.0,-20.0]]],
            [[[-180.0,-20.0],[-178.0,-20.0],[-178.0,-15.0],[-180.0,-15.0],[-180.0,-20.0]]]]});
        let shape = RegionShape::from_geometry(&seam).unwrap();
        let cities = places.in_region(&shape);
        assert!(cities.iter().any(|c| c.place.name == "Labasa"));
        assert!(cities.iter().all(|c| c.lon >= 170.0 && c.lon <= 182.0));
    }

    #[test]
    fn idw_concentrates_near_cities() {
        let places = Places::parse(FIXTURE).unwrap();
        let frame = BBox {
            west: 2.0,
            south: 48.0,
            east: 12.0,
            north: 53.0,
        };
        let cities = places.in_frame(&frame);
        let big = cities
            .iter()
            .max_by(|a, b| a.place.population.partial_cmp(&b.place.population).unwrap())
            .unwrap();
        let w = demand_idw(
            &[(big.lon, big.lat), (big.lon, big.lat + 0.2)],
            &cities,
            25.0,
        );
        assert!(w[0] > w[1]);
        assert!(w[0] > 0.0);
        let none = demand_idw(&[(-20.0, -40.0)], &cities, 25.0);
        assert_eq!(none[0], 0.0);
    }

    #[test]
    fn rejects_out_of_range_rows() {
        let bad = br#"[{"n":"x","lo":200.0,"la":0.0,"p":1,"c":"XXX"}]"#;
        assert!(Places::parse(bad).is_err());
        assert!(Places::parse(b"not json").is_err());
        assert!(Places::empty().is_empty());
    }

    #[cfg(feature = "embedded-places")]
    #[test]
    fn the_bundled_table_parses() {
        let places = Places::embedded().unwrap();
        assert_eq!(places.len(), 69_999);
        assert!(places.rows()[0].population >= places.rows()[1].population);
        // Positional rows carry the state code in the United States only.
        let la = places
            .rows()
            .iter()
            .find(|p| p.name == "Los Angeles" && p.iso_a3 == "USA")
            .unwrap();
        assert_eq!(la.subdivision.as_deref(), Some("US-CA"));
        assert!(places
            .rows()
            .iter()
            .filter(|p| p.iso_a3 != "USA")
            .all(|p| p.subdivision.is_none()));
    }

    #[test]
    fn rows_parse_as_objects_or_arrays() {
        let objects =
            Places::parse(br#"[{"n":"A","lo":1.0,"la":2.0,"p":10,"c":"USA","s":"US-CA"}]"#)
                .unwrap();
        let arrays = Places::parse(br#"[["A",1.0,2.0,10,"USA","US-CA"]]"#).unwrap();
        assert_eq!(objects.rows(), arrays.rows());
        assert_eq!(arrays.rows()[0].subdivision.as_deref(), Some("US-CA"));
        let bare = Places::parse(br#"[["B",1.0,2.0,10,"PRT"]]"#).unwrap();
        assert_eq!(bare.rows()[0].subdivision, None);
        for bad in [
            &br#"[["B",1.0,2.0]]"#[..],
            br#"[["B","x",2.0,10,"PRT"]]"#,
            br#"[[1,1.0,2.0,10,"PRT"]]"#,
        ] {
            assert!(Places::parse(bad).is_err());
        }
    }
}
