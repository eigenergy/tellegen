//! National electricity demand: the yearly demand Ember reports per country
//! (via Our World in Data's energy dataset, CC BY 4.0), bundled as
//! `data/national_demand.json` and derived by
//! `scripts/derive-national-demand.py`, and each country's area (Natural
//! Earth, public domain), bundled as `data/country_areas.json` and derived by
//! `scripts/derive-country-areas.py`.
//!
//! The national demand basis splits a country's peak in two. The share its
//! listed places explain (their population over the national population)
//! follows those places. The rest, the people the places table does not
//! list, is spread over the country's area: a region takes it in proportion
//! to the country area it covers, never more than its share of the country's
//! listed population. A region covering a whole country therefore
//! reproduces that country's peak, and a box holding a country's few listed
//! places no longer takes all of its demand.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::places::Places;

/// Schema tag of the bundled table.
pub const SCHEMA: &str = "tellegen.synth-national-demand/1";

/// Hours in a year, converting annual energy to an average load.
const HOURS_PER_YEAR: f64 = 8760.0;

/// Schema tag of the bundled area table.
pub const AREAS_SCHEMA: &str = "tellegen.synth-country-areas/1";

/// Share of a country's area a region must cover to take all of the
/// country's unlisted demand. A whole-country selection measures a few
/// percent less than the table's area where its outline is more generalised
/// (1:110m against the table's 1:50m) or small islands are left out.
/// Modelling choice, no source.
pub const COUNTRY_AREA_COVERAGE: f64 = 0.95;

/// Peak demand as a multiple of the annual average load (the inverse of the
/// annual load factor). source: annual load factors of 0.58–0.77 in the
/// systems surveyed, a peak factor of 1.3–1.72: Japan 0.60–0.66 over FY2016–
/// FY2023 (OCCTO, *Annual Report FY2024*, Table 1-8); Germany and Spain 0.70,
/// France 0.59, Italy 0.63 in 2024 (ENTSO-E Statistical Factsheet 2024); PJM
/// 0.60 (Monitoring Analytics, *2024 State of the Market Report*, Tables 3-6
/// and 3-8); ERCOT 0.59–0.67 over 2023–2025 (ERCOT hourly native load);
/// China 0.77 (NEA 2024 consumption against the 1,451 GW peak) and India
/// 0.76 (Ministry of Power, FY2023-24). Ember's demand is generation plus net
/// imports, so it counts network losses and self-supplied use that the
/// operators' peak series leave out; the factor applied to it is therefore
/// taken at the low end of the range, China and India's 1.3. That choice is
/// a modelling choice within the sourced range.
pub const DEFAULT_PEAK_FACTOR: f64 = 1.3;

/// One country's (or the world's) annual electricity demand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnualDemand {
    pub year: u32,
    pub twh: f64,
    pub population: f64,
}

impl AnnualDemand {
    /// The annual average load, MW.
    pub fn average_mw(&self) -> f64 {
        self.twh * 1e6 / HOURS_PER_YEAR
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableFile {
    schema: String,
    year: u32,
    source: String,
    source_url: String,
    source_sha256: String,
    licence: String,
    #[allow(dead_code)]
    note: String,
    world: (u32, f64, f64),
    aliases: BTreeMap<String, String>,
    countries: BTreeMap<String, (u32, f64, f64)>,
}

/// The national demand table.
#[derive(Clone, Debug)]
pub struct NationalDemand {
    /// The year most entries report.
    pub year: u32,
    pub source: String,
    pub source_url: String,
    pub licence: String,
    /// SHA-256 of the dataset the table was derived from.
    pub source_sha256: String,
    /// SHA-256 of the table file itself.
    pub sha256: String,
    pub world: AnnualDemand,
    /// Places-table codes that name a table country under another code.
    aliases: BTreeMap<String, String>,
    countries: BTreeMap<String, AnnualDemand>,
}

fn entry(code: &str, (year, twh, population): (u32, f64, f64)) -> Result<AnnualDemand, String> {
    if !(twh.is_finite() && twh > 0.0 && population.is_finite() && population > 0.0) {
        return Err(format!(
            "{code}: demand and population must be positive, got {twh} TWh and {population}"
        ));
    }
    Ok(AnnualDemand {
        year,
        twh,
        population,
    })
}

impl NationalDemand {
    /// Parse and check a table file.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let file: TableFile =
            serde_json::from_slice(bytes).map_err(|e| format!("national demand table: {e}"))?;
        if file.schema != SCHEMA {
            return Err(format!(
                "national demand table: schema {:?}, expected {SCHEMA:?}",
                file.schema
            ));
        }
        let countries = file
            .countries
            .into_iter()
            .map(|(code, row)| entry(&code, row).map(|d| (code, d)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        for (alias, target) in &file.aliases {
            if !countries.contains_key(target) {
                return Err(format!(
                    "national demand table: alias {alias} names {target}, which has no entry"
                ));
            }
        }
        Ok(Self {
            year: file.year,
            source: file.source,
            source_url: file.source_url,
            licence: file.licence,
            source_sha256: file.source_sha256,
            sha256: Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            world: entry("world", file.world)?,
            aliases: file.aliases,
            countries,
        })
    }

    /// The bundled table, parsed once per process.
    pub fn bundled() -> &'static NationalDemand {
        static TABLE: OnceLock<NationalDemand> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(include_bytes!("../data/national_demand.json"))
                .expect("the bundled national demand table parses")
        })
    }

    /// The table code a places-table country code stands for.
    pub fn canonical<'a>(&'a self, code: &'a str) -> &'a str {
        self.aliases.get(code).map_or(code, String::as_str)
    }

    /// The entry a places-table country code resolves to.
    pub fn country(&self, code: &str) -> Option<&AnnualDemand> {
        self.countries.get(self.canonical(code))
    }

    pub fn len(&self) -> usize {
        self.countries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.countries.is_empty()
    }

    /// Peak MW per person of the places table, by table code: the country's
    /// average load × `peak_factor`, divided by the population of every place
    /// of that country in `places`. Each place then carries its country's peak
    /// in proportion to its population, so the places of a country sum to the
    /// national peak. A country without an entry or without places is absent.
    pub fn place_intensity(&self, places: &Places, peak_factor: f64) -> BTreeMap<String, f64> {
        let mut population: BTreeMap<&str, f64> = BTreeMap::new();
        for (code, &total) in places.country_population() {
            *population.entry(self.canonical(code)).or_default() += total;
        }
        population
            .into_iter()
            .filter_map(|(code, total)| {
                let country = self.countries.get(code)?;
                (total > 0.0).then(|| (code.to_owned(), country.average_mw() * peak_factor / total))
            })
            .collect()
    }

    /// Peak MW per person for a place whose country has no entry: the world's
    /// average load per person × `peak_factor`.
    pub fn world_intensity(&self, peak_factor: f64) -> f64 {
        self.world.average_mw() / self.world.population * peak_factor
    }

    /// Population of every place of each country in `places`, by table code.
    pub fn listed_population(&self, places: &Places) -> BTreeMap<String, f64> {
        let mut population: BTreeMap<String, f64> = BTreeMap::new();
        for (code, &total) in places.country_population() {
            *population
                .entry(self.canonical(code).to_owned())
                .or_default() += total;
        }
        population
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AreasFile {
    schema: String,
    source: String,
    source_url: String,
    source_sha256: String,
    #[allow(dead_code)]
    note: String,
    areas_km2: BTreeMap<String, f64>,
}

/// Country areas by places-table code (Natural Earth `ADM0_A3`), km².
#[derive(Clone, Debug)]
pub struct CountryAreas {
    pub source: String,
    pub source_url: String,
    /// SHA-256 of the admin-0 file the areas were derived from.
    pub source_sha256: String,
    /// SHA-256 of the area table itself.
    pub sha256: String,
    areas: BTreeMap<String, f64>,
}

impl CountryAreas {
    /// Parse and check an area table.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let file: AreasFile =
            serde_json::from_slice(bytes).map_err(|e| format!("country area table: {e}"))?;
        if file.schema != AREAS_SCHEMA {
            return Err(format!(
                "country area table: schema {:?}, expected {AREAS_SCHEMA:?}",
                file.schema
            ));
        }
        if let Some((code, area)) = file
            .areas_km2
            .iter()
            .find(|(_, &a)| !(a.is_finite() && a >= 0.0))
        {
            return Err(format!("country area table: {code} has area {area}"));
        }
        Ok(Self {
            source: file.source,
            source_url: file.source_url,
            source_sha256: file.source_sha256,
            sha256: Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            areas: file.areas_km2,
        })
    }

    /// The bundled table, parsed once per process.
    pub fn bundled() -> &'static CountryAreas {
        static TABLE: OnceLock<CountryAreas> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(include_bytes!("../data/country_areas.json"))
                .expect("the bundled country area table parses")
        })
    }

    pub fn len(&self) -> usize {
        self.areas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.areas.is_empty()
    }

    /// Area of a national demand table country, km²: every places-table code
    /// that names it (aliases included). `None` when no code has an area.
    pub fn country_km2(&self, table: &NationalDemand, code: &str) -> Option<f64> {
        let mut found = None;
        for (place_code, &area) in &self.areas {
            if table.canonical(place_code) == code {
                *found.get_or_insert(0.0) += area;
            }
        }
        found.filter(|&a| a > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_table_parses_and_resolves_aliases() {
        let table = NationalDemand::bundled();
        assert!(table.len() > 190, "{}", table.len());
        assert_eq!(table.year, 2024);
        assert_eq!(table.canonical("PSX"), "PSE");
        assert!(table.country("PSX").is_some());
        assert!(table.country("XXX").is_none());
        let usa = table.country("USA").unwrap();
        // About 500 GW of average load.
        assert!(
            (400_000.0..600_000.0).contains(&usa.average_mw()),
            "{usa:?}"
        );
        assert_eq!(table.sha256.len(), 64);
    }

    #[test]
    fn the_bundled_table_is_the_pinned_derivation() {
        // A new derivation (scripts/derive-national-demand.py) changes this
        // digest on purpose; update it with the table.
        let table = NationalDemand::bundled();
        assert_eq!(
            table.sha256,
            "5bff36afafcf42ba82c8bce5843ee54970c1ae7cf49881dc80dafcbdd7ffa677"
        );
        assert_eq!(table.source_url, "https://github.com/owid/energy-data");
        assert_eq!(table.licence, "CC BY 4.0");
        assert_eq!(table.source_sha256.len(), 64);
        assert!(table.world.year == table.year && table.world.twh > 20_000.0);
    }

    #[test]
    fn a_country_s_places_sum_to_its_peak() {
        let table = NationalDemand::bundled();
        let places = Places::parse(
            br#"[{"n":"A","lo":0,"la":0,"p":3000000,"c":"PRT"},
                 {"n":"B","lo":1,"la":0,"p":1000000,"c":"PRT"},
                 {"n":"C","lo":2,"la":0,"p":500000,"c":"ZZZ"}]"#,
        )
        .unwrap();
        let k = table.place_intensity(&places, 1.3);
        let peak = table.country("PRT").unwrap().average_mw() * 1.3;
        assert!((k["PRT"] * 4_000_000.0 - peak).abs() < 1e-6 * peak);
        assert!(!k.contains_key("ZZZ"));
        let world = table.world_intensity(1.3);
        assert!((0.0003..0.001).contains(&world), "{world}");
    }

    #[test]
    fn the_bundled_areas_cover_the_places_and_sum_aliases() {
        let areas = CountryAreas::bundled();
        let table = NationalDemand::bundled();
        assert!(areas.len() > 200, "{}", areas.len());
        assert_eq!(areas.sha256.len(), 64);
        // Natural Earth's outline at 1:50m: about 9.4 million km² with Alaska.
        let usa = areas.country_km2(table, "USA").unwrap();
        assert!((9.0e6..9.9e6).contains(&usa), "{usa}");
        // Finland takes the Åland Islands, which the places table codes ALD.
        let fin = areas.country_km2(table, "FIN").unwrap();
        assert!(fin > areas.areas["FIN"], "{fin}");
        assert!(areas.country_km2(table, "XXX").is_none());
        let bad = r#"{"schema":"tellegen.synth-country-areas/1","source":"s","source_url":"u","source_sha256":"x","note":"n","areas_km2":{"AAA":-1}}"#;
        assert!(CountryAreas::parse(bad.as_bytes()).is_err());
    }

    #[test]
    fn malformed_tables_are_refused() {
        let ok = include_str!("../data/national_demand.json");
        assert!(NationalDemand::parse(ok.replace(SCHEMA, "other/1").as_bytes()).is_err());
        let negative = r#"{"schema":"tellegen.synth-national-demand/1","year":2024,"source":"s","source_url":"u","source_sha256":"x","licence":"l","note":"n","world":[2024,1.0,1.0],"aliases":{},"countries":{"AAA":[2024,-1.0,5.0]}}"#;
        assert!(NationalDemand::parse(negative.as_bytes()).is_err());
        let dangling = r#"{"schema":"tellegen.synth-national-demand/1","year":2024,"source":"s","source_url":"u","source_sha256":"x","licence":"l","note":"n","world":[2024,1.0,1.0],"aliases":{"BBB":"CCC"},"countries":{}}"#;
        assert!(NationalDemand::parse(dangling.as_bytes()).is_err());
    }
}
