//! Overpass JSON documents and the OSM power tags the grid build reads. The
//! tag parsers are written by hand (no regular expressions) to keep the wasm
//! small, and accept the spellings real OpenStreetMap data carries.

use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::SynthError;
use crate::report::OsmProvenance;
use crate::request::Fuel;

#[derive(Deserialize)]
struct Document {
    #[serde(default)]
    osm3s: Option<Osm3s>,
    #[serde(default)]
    elements: Vec<Element>,
}

#[derive(Deserialize)]
struct Osm3s {
    #[serde(default)]
    timestamp_osm_base: Option<String>,
    #[serde(default)]
    copyright: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
struct LatLon {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: i64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    center: Option<LatLon>,
    /// `out geom` geometry of a way; Overpass writes `null` for a node it
    /// could not resolve.
    #[serde(default)]
    geometry: Vec<Option<LatLon>>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

impl Element {
    /// A node's own position, or the centre `out center` gives a way or
    /// relation: `(lon, lat)`.
    fn point(&self) -> Option<(f64, f64)> {
        match (self.lon, self.lat, self.center) {
            (Some(lon), Some(lat), _) => Some((lon, lat)),
            (_, _, Some(c)) => Some((c.lon, c.lat)),
            _ => None,
        }
    }

    fn tag(&self, key: &str) -> &str {
        self.tags.get(key).map(String::as_str).unwrap_or("")
    }
}

/// A substation site with every voltage level its tags list, in kV, highest first.
#[derive(Clone, Debug, PartialEq)]
pub struct OsmSubstation {
    pub id: i64,
    pub lon: f64,
    pub lat: f64,
    pub kvs: Vec<f64>,
}

/// A transmission line or cable way.
#[derive(Clone, Debug, PartialEq)]
pub struct OsmLine {
    /// The way ids stitched into this line.
    pub ids: Vec<i64>,
    /// `(lon, lat)` polyline.
    pub geometry: Vec<(f64, f64)>,
    /// Highest tagged voltage, kV; 0 when untagged (inferred later).
    pub kv: f64,
    pub circuits: u32,
    pub is_cable: bool,
    /// Tagged `frequency=0`: a DC line.
    pub is_dc: bool,
}

/// A power plant or generator.
#[derive(Clone, Debug, PartialEq)]
pub struct OsmPlant {
    pub id: i64,
    pub lon: f64,
    pub lat: f64,
    pub fuel: Fuel,
    /// Tagged electrical output, MW; `None` when untagged.
    pub capacity_mw: Option<f64>,
    /// `power=generator`: one unit, often inside a mapped `power=plant`.
    pub is_generator: bool,
}

/// An HVDC converter station.
#[derive(Clone, Debug, PartialEq)]
pub struct OsmConverter {
    pub id: i64,
    pub lon: f64,
    pub lat: f64,
    pub kv: f64,
}

/// What a set of Overpass documents holds for the grid build.
pub struct ParsedDocument {
    pub provenance: OsmProvenance,
    pub substations: Vec<OsmSubstation>,
    pub lines: Vec<OsmLine>,
    pub plants: Vec<OsmPlant>,
    pub converters: Vec<OsmConverter>,
}

/// Parse one Overpass JSON document or the two documents from an
/// antimeridian-crossing request. Elements seen twice are kept once. Reject
/// oversized inputs before creating the mapped graph.
pub fn parse_document(text: &str) -> Result<ParsedDocument, SynthError> {
    parse_document_with_limits(
        text,
        crate::osm::MAX_OSM_DOCUMENT_BYTES,
        crate::osm::MAX_OSM_ELEMENTS,
    )
}

/// Parse an Overpass document under the host's explicit resource limits.
pub fn parse_document_with_limits(
    text: &str,
    max_document_bytes: usize,
    max_elements: usize,
) -> Result<ParsedDocument, SynthError> {
    if text.len() > max_document_bytes {
        return Err(SynthError::Osm(format!(
            "document is {} bytes, over the {} byte safety limit",
            text.len(),
            max_document_bytes
        )));
    }
    let trimmed = text.trim_start();
    let documents: Vec<Document> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)
    } else {
        serde_json::from_str(trimmed).map(|d| vec![d])
    }
    .map_err(|e| SynthError::Osm(format!("cannot parse the Overpass JSON: {e}")))?;
    let element_count: usize = documents.iter().map(|d| d.elements.len()).sum();
    if element_count > max_elements {
        return Err(SynthError::Osm(format!(
            "response contains {element_count} elements, over the {max_elements} element safety limit"
        )));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut out = ParsedDocument {
        provenance: OsmProvenance {
            generator: "overpass".to_owned(),
            timestamp_osm_base: documents
                .iter()
                .find_map(|d| d.osm3s.as_ref().and_then(|o| o.timestamp_osm_base.clone())),
            copyright: documents
                .iter()
                .find_map(|d| d.osm3s.as_ref().and_then(|o| o.copyright.clone())),
            document_sha256: Sha256::digest(text.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            elements: 0,
            counts: BTreeMap::new(),
        },
        substations: Vec::new(),
        lines: Vec::new(),
        plants: Vec::new(),
        converters: Vec::new(),
    };
    for el in documents.iter().flat_map(|d| &d.elements) {
        if !seen.insert((el.kind.clone(), el.id)) {
            continue;
        }
        out.provenance.elements += 1;
        match el.tag("power") {
            "substation" => {
                let (Some((lon, lat)), kvs) = (el.point(), parse_voltages(el.tag("voltage")))
                else {
                    continue;
                };
                if !kvs.is_empty() {
                    out.substations.push(OsmSubstation {
                        id: el.id,
                        lon,
                        lat,
                        kvs,
                    });
                }
            }
            power @ ("line" | "cable") => {
                let geometry: Vec<(f64, f64)> = el
                    .geometry
                    .iter()
                    .flatten()
                    .map(|p| (p.lon, p.lat))
                    .collect();
                if geometry.len() < 2 {
                    continue;
                }
                out.lines.push(OsmLine {
                    ids: vec![el.id],
                    geometry,
                    kv: parse_voltage(el.tag("voltage")),
                    circuits: parse_circuits(el.tag("circuits"), el.tag("cables")),
                    is_cable: power == "cable",
                    is_dc: el.tag("frequency").trim() == "0",
                });
            }
            power @ ("plant" | "generator") => {
                let Some((lon, lat)) = el.point() else {
                    continue;
                };
                let output = match el.tag("plant:output:electricity") {
                    "" => el.tag("generator:output:electricity"),
                    s => s,
                };
                let source = match el.tag("plant:source") {
                    "" => el.tag("generator:source"),
                    s => s,
                };
                out.plants.push(OsmPlant {
                    id: el.id,
                    lon,
                    lat,
                    fuel: parse_fuel(source),
                    capacity_mw: parse_power_mw(output),
                    is_generator: power == "generator",
                });
            }
            "converter" => {
                let Some((lon, lat)) = el.point() else {
                    continue;
                };
                out.converters.push(OsmConverter {
                    id: el.id,
                    lon,
                    lat,
                    kv: parse_voltage(el.tag("voltage")),
                });
            }
            _ => {}
        }
    }
    let counts = &mut out.provenance.counts;
    counts.insert(
        "substations_tagged".to_owned(),
        out.substations.len() as u64,
    );
    counts.insert("lines_tagged".to_owned(), out.lines.len() as u64);
    counts.insert("plants_tagged".to_owned(), out.plants.len() as u64);
    counts.insert("converters_tagged".to_owned(), out.converters.len() as u64);
    Ok(out)
}

/// The numbers in a voltage tag, in volts: `"380000;220000"`, `"110000"`.
fn voltage_values(tag: &str) -> impl Iterator<Item = f64> + '_ {
    tag.split([';', ',', '/', ' '])
        .filter_map(|part| part.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// The highest voltage a tag lists, kV; 0 when nothing parses.
pub fn parse_voltage(tag: &str) -> f64 {
    voltage_values(tag).fold(0.0, f64::max) / 1000.0
}

/// Every distinct voltage level a tag lists, kV, highest first. The caller
/// applies its voltage floor.
pub fn parse_voltages(tag: &str) -> Vec<f64> {
    let mut kvs: Vec<f64> = voltage_values(tag).map(|v| v / 1000.0).collect();
    kvs.sort_by(|a, b| b.partial_cmp(a).unwrap());
    kvs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    kvs
}

/// Most circuits one mapped way, or one corridor of joined ways, may carry.
/// Real lines carry a handful; anything above this is a tagging error, and
/// every circuit becomes a branch row.
pub const MAX_OSM_CIRCUITS: u32 = 12;

/// Circuits on a line: the `circuits` tag, else `cables / 3` (three
/// conductors per AC circuit), between one and [`MAX_OSM_CIRCUITS`].
pub fn parse_circuits(circuits: &str, cables: &str) -> u32 {
    if let Ok(c) = circuits.trim().parse::<u32>() {
        return c.clamp(1, MAX_OSM_CIRCUITS);
    }
    if let Ok(c) = cables.trim().parse::<f64>() {
        if c.is_finite() {
            return ((c / 3.0).round() as u32).clamp(1, MAX_OSM_CIRCUITS);
        }
    }
    1
}

/// A plant or generator source tag as a fuel family; the first listed source
/// counts. Oil, geothermal, biomass, waste and unknown sources map to gas,
/// the dispatchable thermal family they price like.
pub fn parse_fuel(source: &str) -> Fuel {
    let s = source
        .split([';', ','])
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if s.is_empty() {
        Fuel::Gas
    } else if s.contains("coal") || s == "lignite" {
        Fuel::Coal
    } else if s == "solar" || s == "photovoltaic" {
        Fuel::Solar
    } else if s == "wind" {
        Fuel::Wind
    } else if matches!(s.as_str(), "hydro" | "water" | "tidal" | "wave" | "stream") {
        Fuel::Hydro
    } else if s == "nuclear" {
        Fuel::Nuclear
    } else if s == "battery" || s.contains("storage") {
        Fuel::Storage
    } else {
        Fuel::Gas
    }
}

/// An OSM power value in MW: `"1300 MW"`, `"1.3 GW"`, `"600 kW"`, `"1300
/// megawatt"`, `"1300 MVA"` (read as MW for a nameplate estimate), `"1,300
/// MW"`, `"1.5e3 W"`, and `"2x600 MW"` for identical units. A bare number is
/// watts, OSM's base unit. `None` for `"yes"`, blank or unparseable values.
pub fn parse_power_mw(value: &str) -> Option<f64> {
    let s: String = value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| *c != ',')
        .collect();
    if s.is_empty() || s == "yes" {
        return None;
    }
    let mut rest = s.as_str();
    let mut multiplier = 1.0;
    // "NxM": N identical units of M.
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let after = rest[digits..].trim_start();
        if let Some(tail) = after.strip_prefix('x').or_else(|| after.strip_prefix('×')) {
            multiplier = rest[..digits].parse::<f64>().ok()?;
            rest = tail.trim_start();
        }
    }
    let (number, tail) = leading_number(rest)?;
    let tail = tail.trim_start();
    let (factor, unit_len) = [
        ("giga", 1000.0),
        ("mega", 1.0),
        ("kilo", 0.001),
        ("g", 1000.0),
        ("m", 1.0),
        ("k", 0.001),
        ("", 1e-6),
    ]
    .iter()
    .find_map(|&(prefix, factor)| {
        let after = tail.strip_prefix(prefix)?;
        ["watt", "va", "w"]
            .iter()
            .find(|unit| after.starts_with(*unit))
            .map(|unit| (factor, prefix.len() + unit.len()))
    })
    .unwrap_or((1e-6, 0));
    // The unit must end the word ("mw" yes, "mwh" no).
    if tail[unit_len..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let mw = multiplier * number * factor;
    mw.is_finite().then_some(mw)
}

/// A leading decimal number with an optional sign, fraction and exponent,
/// and the text after it.
fn leading_number(s: &str) -> Option<(f64, &str)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        i += 1;
    }
    let start_digits = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i == start_digits || (i == start_digits + 1 && bytes[start_digits] == b'.') {
        return None;
    }
    if i < bytes.len() && bytes[i] == b'e' {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'-' || bytes[j] == b'+') {
            j += 1;
        }
        let exp_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            i = j;
        }
    }
    s[..i].parse::<f64>().ok().map(|v| (v, &s[i..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voltage_tags() {
        assert_eq!(parse_voltage("220000;110000"), 220.0);
        assert_eq!(parse_voltage("400000"), 400.0);
        assert_eq!(parse_voltage(""), 0.0);
        assert_eq!(parse_voltage("high"), 0.0);
        assert_eq!(parse_voltages("380000;220000;220000"), vec![380.0, 220.0]);
        assert_eq!(parse_voltages("110000 / 20000"), vec![110.0, 20.0]);
    }

    #[test]
    fn circuits_and_fuels() {
        assert_eq!(parse_circuits("2", ""), 2);
        assert_eq!(parse_circuits("", "6"), 2);
        assert_eq!(parse_circuits("", "3"), 1);
        assert_eq!(parse_circuits("0", ""), 1);
        assert_eq!(parse_circuits("", ""), 1);
        assert_eq!(parse_fuel("coal;gas"), Fuel::Coal);
        assert_eq!(parse_fuel("lignite"), Fuel::Coal);
        assert_eq!(parse_fuel("photovoltaic"), Fuel::Solar);
        assert_eq!(parse_fuel("Wind"), Fuel::Wind);
        assert_eq!(parse_fuel("water"), Fuel::Hydro);
        assert_eq!(parse_fuel("nuclear"), Fuel::Nuclear);
        assert_eq!(parse_fuel("battery"), Fuel::Storage);
        assert_eq!(parse_fuel("biomass"), Fuel::Gas);
        assert_eq!(parse_fuel(""), Fuel::Gas);
    }

    #[test]
    fn power_values() {
        let close = |v: Option<f64>, x: f64| v.is_some_and(|v| (v - x).abs() < 1e-9);
        assert!(close(parse_power_mw("1300 MW"), 1300.0));
        assert!(close(parse_power_mw("1.3 GW"), 1300.0));
        assert!(close(parse_power_mw("600 kW"), 0.6));
        assert!(close(parse_power_mw("1300 megawatt"), 1300.0));
        assert!(close(parse_power_mw("1300 MVA"), 1300.0));
        assert!(close(parse_power_mw("1,300 MW"), 1300.0));
        assert!(close(parse_power_mw("1.5e3 W"), 0.0015));
        assert!(close(parse_power_mw("2x600 MW"), 1200.0));
        assert!(close(parse_power_mw("2 × 600 MW"), 1200.0));
        assert!(close(parse_power_mw("50000000"), 50.0));
        assert!(close(parse_power_mw("-5 MW"), -5.0));
        assert_eq!(parse_power_mw("yes"), None);
        assert_eq!(parse_power_mw(""), None);
        assert_eq!(parse_power_mw("lots"), None);
        assert_eq!(parse_power_mw("5 MWh"), None);
    }

    #[test]
    fn oversized_raw_document_is_refused_before_json_parsing() {
        let oversized = " ".repeat(crate::osm::MAX_OSM_DOCUMENT_BYTES + 1);
        let err = match parse_document(&oversized) {
            Err(err) => err,
            Ok(_) => panic!("oversized document was accepted"),
        };
        assert!(err.to_string().contains("safety limit"), "{err}");
    }

    #[test]
    fn host_resource_limits_are_enforced() {
        let document = r#"{"elements":[]}"#;
        assert!(parse_document_with_limits(document, 1, 10)
            .err()
            .expect("oversized document should fail")
            .to_string()
            .contains("byte safety limit"));

        let document = r#"{"elements":[{"type":"node","id":1},{"type":"node","id":2}]}"#;
        assert!(parse_document_with_limits(document, 1024, 1)
            .err()
            .expect("too many elements should fail")
            .to_string()
            .contains("element safety limit"));
    }

    #[test]
    fn documents_and_arrays_of_documents() {
        let doc = r#"{"osm3s":{"timestamp_osm_base":"2026-10-01T00:00:00Z","copyright":"The data included in this document is from www.openstreetmap.org. The data is made available under ODbL."},"elements":[
            {"type":"node","id":1,"lat":40.0,"lon":1.0,"tags":{"power":"substation","voltage":"220000;110000"}},
            {"type":"way","id":2,"center":{"lat":40.5,"lon":1.5},"tags":{"power":"plant","plant:source":"wind","plant:output:electricity":"2x50 MW"}},
            {"type":"way","id":3,"geometry":[{"lat":40.0,"lon":1.0},null,{"lat":40.5,"lon":1.5}],"tags":{"power":"line","voltage":"220000","circuits":"2"}},
            {"type":"node","id":4,"lat":40.2,"lon":1.2,"tags":{"power":"converter","voltage":"320000"}},
            {"type":"node","id":5,"lat":40.2,"lon":1.2,"tags":{"amenity":"cafe"}}
        ]}"#;
        let parsed = parse_document(doc).unwrap();
        assert_eq!(
            parsed.provenance.timestamp_osm_base.as_deref(),
            Some("2026-10-01T00:00:00Z")
        );
        assert!(parsed
            .provenance
            .copyright
            .as_deref()
            .is_some_and(|c| c.ends_with("under ODbL.")));
        assert_eq!(parsed.provenance.elements, 5);
        assert_eq!(parsed.substations[0].kvs, vec![220.0, 110.0]);
        assert_eq!(parsed.plants[0].fuel, Fuel::Wind);
        assert_eq!(parsed.plants[0].capacity_mw, Some(100.0));
        assert_eq!(parsed.lines[0].geometry.len(), 2);
        assert_eq!(parsed.lines[0].circuits, 2);
        assert_eq!(parsed.converters[0].kv, 320.0);
        let both = format!("[{doc},{doc}]");
        let parsed = parse_document(&both).unwrap();
        assert_eq!(
            parsed.provenance.elements, 5,
            "duplicates across halves count once"
        );
        assert!(parse_document("{not json").is_err());
    }
}
