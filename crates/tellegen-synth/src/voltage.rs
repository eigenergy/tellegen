//! National AC transmission voltage ladders: the two or three levels a
//! national transmission model of each country uses, highest first, and any
//! higher AC level actually in service there, which the corridor plan may
//! raise buses to. Bundled as `data/voltage_ladders.json`, with the sources
//! of every entry. A request that names no levels takes the ladder of its
//! region's dominant country; a country without an entry takes the ENTSO-E
//! members' ladder when it is one, else IEC 60038's 400/220 kV, and the
//! report says so (`voltage_fallback`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Schema tag of the bundled table.
pub const SCHEMA: &str = "tellegen.synth-voltage-ladders/1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    levels_kv: Vec<f64>,
    #[serde(default)]
    above_kv: Vec<f64>,
    sources: Vec<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Regional {
    name: String,
    members: Vec<String>,
    levels_kv: Vec<f64>,
    sources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Standard {
    name: String,
    levels_kv: Vec<f64>,
    sources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableFile {
    schema: String,
    version: String,
    #[allow(dead_code)]
    note: String,
    regional: Regional,
    standard: Standard,
    countries: BTreeMap<String, Entry>,
}

/// Where a ladder came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LadderSource {
    /// The country's own entry.
    Country,
    /// The ENTSO-E members' ladder, for a member without an entry.
    Regional,
    /// IEC 60038's standard levels, for any other country.
    Standard,
}

/// A resolved ladder.
#[derive(Clone, Debug, PartialEq)]
pub struct Ladder {
    /// Levels a national model uses, kV, highest first.
    pub levels_kv: Vec<f64>,
    /// Higher AC levels in service, kV, highest first; the corridor plan may
    /// raise buses to them. Empty when there are none.
    pub above_kv: Vec<f64>,
    pub source: LadderSource,
}

/// The voltage ladder table.
#[derive(Clone, Debug)]
pub struct VoltageLadders {
    pub version: String,
    /// SHA-256 of the table file.
    pub sha256: String,
    pub regional_name: String,
    pub standard_name: String,
    regional_members: BTreeSet<String>,
    regional_levels: Vec<f64>,
    standard_levels: Vec<f64>,
    countries: BTreeMap<String, (Vec<f64>, Vec<f64>)>,
}

/// Levels sorted highest first; an error for an empty, non-positive or
/// repeated level.
fn ladder(what: &str, levels: &[f64]) -> Result<Vec<f64>, String> {
    if levels.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return Err(format!("voltage table: {what} has a non-positive level"));
    }
    let mut out = levels.to_vec();
    out.sort_by(|a, b| b.total_cmp(a));
    if out.windows(2).any(|w| (w[0] - w[1]).abs() < 1e-9) {
        return Err(format!("voltage table: {what} repeats a level"));
    }
    Ok(out)
}

impl VoltageLadders {
    /// Parse and check a table file.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let file: TableFile =
            serde_json::from_slice(bytes).map_err(|e| format!("voltage table: {e}"))?;
        if file.schema != SCHEMA {
            return Err(format!(
                "voltage table: schema {:?}, expected {SCHEMA:?}",
                file.schema
            ));
        }
        let mut countries = BTreeMap::new();
        for (code, entry) in file.countries {
            let levels = ladder(&code, &entry.levels_kv)?;
            let above = ladder(&code, &entry.above_kv)?;
            if levels.is_empty() || entry.sources.is_empty() {
                return Err(format!("voltage table: {code} needs levels and a source"));
            }
            if above.last().is_some_and(|&a| a <= levels[0]) {
                return Err(format!(
                    "voltage table: {code} lists a higher level at or below its top level"
                ));
            }
            let _ = entry.note;
            countries.insert(code, (levels, above));
        }
        let regional_levels = ladder("regional", &file.regional.levels_kv)?;
        let standard_levels = ladder("standard", &file.standard.levels_kv)?;
        if regional_levels.is_empty()
            || standard_levels.is_empty()
            || file.regional.sources.is_empty()
            || file.standard.sources.is_empty()
        {
            return Err("voltage table: the fallbacks need levels and a source".to_owned());
        }
        Ok(Self {
            version: file.version,
            sha256: Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            regional_name: file.regional.name,
            standard_name: file.standard.name,
            regional_members: file.regional.members.into_iter().collect(),
            regional_levels,
            standard_levels,
            countries,
        })
    }

    /// The bundled table, parsed once per process.
    pub fn bundled() -> &'static VoltageLadders {
        static TABLE: OnceLock<VoltageLadders> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(include_bytes!("../data/voltage_ladders.json"))
                .expect("the bundled voltage ladder table parses")
        })
    }

    pub fn len(&self) -> usize {
        self.countries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.countries.is_empty()
    }

    /// The ladder of a country (a table code), or a fallback.
    pub fn ladder(&self, code: Option<&str>) -> Ladder {
        if let Some((levels, above)) = code.and_then(|c| self.countries.get(c)) {
            return Ladder {
                levels_kv: levels.clone(),
                above_kv: above.clone(),
                source: LadderSource::Country,
            };
        }
        if code.is_some_and(|c| self.regional_members.contains(c)) {
            return Ladder {
                levels_kv: self.regional_levels.clone(),
                above_kv: Vec::new(),
                source: LadderSource::Regional,
            };
        }
        Ladder {
            levels_kv: self.standard_levels.clone(),
            above_kv: Vec::new(),
            source: LadderSource::Standard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_table_parses_and_falls_back() {
        let table = VoltageLadders::bundled();
        assert!(table.len() >= 17, "{}", table.len());
        assert_eq!(table.sha256.len(), 64);
        let deu = table.ladder(Some("DEU"));
        assert_eq!(deu.levels_kv, vec![380.0, 220.0]);
        assert!(deu.above_kv.is_empty());
        assert_eq!(deu.source, LadderSource::Country);
        let usa = table.ladder(Some("USA"));
        assert_eq!(usa.above_kv, vec![765.0]);
        assert!(usa.levels_kv[0] < usa.above_kv[0]);
        // A member of ENTSO-E without its own entry, and any other country.
        let regional = table.ladder(Some("LUX"));
        assert_eq!(regional.source, LadderSource::Regional);
        assert_eq!(table.ladder(Some("XXX")).source, LadderSource::Standard);
        assert_eq!(table.ladder(None).source, LadderSource::Standard);
    }

    #[test]
    fn malformed_tables_are_refused() {
        let ok = include_str!("../data/voltage_ladders.json");
        assert!(VoltageLadders::parse(ok.replace(SCHEMA, "other/1").as_bytes()).is_err());
        let base = |countries: &str| {
            format!(
                r#"{{"schema":"{SCHEMA}","version":"v","note":"n",
                "regional":{{"name":"r","members":["AAA"],"levels_kv":[400,220],"sources":["s"]}},
                "standard":{{"name":"s","levels_kv":[400,220],"sources":["s"]}},
                "countries":{countries}}}"#
            )
        };
        assert!(VoltageLadders::parse(base("{}").as_bytes()).is_ok());
        let below = base(r#"{"AAA":{"levels_kv":[400,220],"above_kv":[380],"sources":["s"]}}"#);
        assert!(VoltageLadders::parse(below.as_bytes()).is_err());
        let unsourced = base(r#"{"AAA":{"levels_kv":[400,220],"sources":[]}}"#);
        assert!(VoltageLadders::parse(unsourced.as_bytes()).is_err());
        let repeated = base(r#"{"AAA":{"levels_kv":[400,400],"sources":["s"]}}"#);
        assert!(VoltageLadders::parse(repeated.as_bytes()).is_err());
    }
}
