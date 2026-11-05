//! First-level subdivision tables: annual electricity demand and population
//! of every US state (EIA-861 retail sales, Census population, a peak factor
//! from EIA-930 where the state's balancing authorities cover it), and each
//! state's generating capacity mix (EIA-860 nameplate). Bundled as
//! `data/subnational_demand.json` and `data/subnational_fuel_mix.json`, built
//! by `scripts/derive-subnational-demand.py` and
//! `scripts/derive-subnational-fuel-mix.py`; `docs/evidence/synth/subnational-data.md`
//! summarises them. Places carry the ISO 3166-2 code these tables are keyed
//! by ([`crate::places::Place::subdivision`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::request::Fuel;

pub const DEMAND_SCHEMA: &str = "tellegen.synth-subnational-demand/1";

/// Peak factor of a subdivision the table has no measured factor for: the
/// ratio of the lower 48 states' coincident hourly peak to their mean hourly
/// demand in 2025, 760.6 GW over 490.7 GW, from the same EIA-930 files
/// (Demand (MW) (Adjusted), each balancing authority's implausible hours
/// dropped as in `scripts/derive-subnational-demand.py`). A state's own peak
/// is not coincident with the nation's, so this errs low, but it shares the
/// measured factors' basis; the national table's 1.3 is a world default.
pub const UNMEASURED_PEAK_FACTOR: f64 = 1.55;
pub const FUEL_MIX_SCHEMA: &str = "tellegen.synth-subnational-fuel-mix/1";

/// One subdivision's annual demand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubdivisionEntry {
    pub year: u32,
    pub twh: f64,
    pub population: f64,
    /// Annual peak over mean hourly demand, where measured.
    pub peak_factor: Option<f64>,
}

impl SubdivisionEntry {
    pub fn average_mw(&self) -> f64 {
        self.twh * 1e6 / 8760.0
    }
}

#[derive(Deserialize)]
struct DemandFile {
    schema: String,
    year: u32,
    country: String,
    subdivisions: BTreeMap<String, (u32, f64, f64, Option<f64>)>,
}

#[derive(Deserialize)]
struct MixEntry {
    #[allow(dead_code)]
    total_mw: f64,
    shares: BTreeMap<Fuel, f64>,
}

#[derive(Deserialize)]
struct MixFile {
    schema: String,
    subdivisions: BTreeMap<String, MixEntry>,
}

/// The subdivision demand and capacity tables.
#[derive(Clone, Debug)]
pub struct Subnational {
    pub year: u32,
    /// Country (ISO 3166-1 alpha-3) the tables cover.
    pub country: String,
    /// SHA-256 of the demand table and of the fuel mix table.
    pub demand_sha256: String,
    pub fuel_mix_sha256: String,
    demand: BTreeMap<String, SubdivisionEntry>,
    mix: BTreeMap<String, BTreeMap<Fuel, f64>>,
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Subnational {
    pub fn parse(demand: &[u8], mix: &[u8]) -> Result<Self, String> {
        let file: DemandFile =
            serde_json::from_slice(demand).map_err(|e| format!("subnational demand table: {e}"))?;
        if file.schema != DEMAND_SCHEMA {
            return Err(format!(
                "subnational demand table: schema {:?}, expected {DEMAND_SCHEMA:?}",
                file.schema
            ));
        }
        let mut entries = BTreeMap::new();
        for (code, (year, twh, population, peak_factor)) in file.subdivisions {
            if !(twh.is_finite() && twh > 0.0 && population.is_finite() && population > 0.0) {
                return Err(format!(
                    "subnational demand table: {code} has {twh} TWh and {population} people"
                ));
            }
            if peak_factor.is_some_and(|f| !(f.is_finite() && f >= 1.0)) {
                return Err(format!(
                    "subnational demand table: {code} has peak factor {peak_factor:?}"
                ));
            }
            entries.insert(
                code,
                SubdivisionEntry {
                    year,
                    twh,
                    population,
                    peak_factor,
                },
            );
        }
        let mixes: MixFile =
            serde_json::from_slice(mix).map_err(|e| format!("subnational fuel mix table: {e}"))?;
        if mixes.schema != FUEL_MIX_SCHEMA {
            return Err(format!(
                "subnational fuel mix table: schema {:?}, expected {FUEL_MIX_SCHEMA:?}",
                mixes.schema
            ));
        }
        let mut mix_out = BTreeMap::new();
        for (code, entry) in mixes.subdivisions {
            let total: f64 = entry.shares.values().sum();
            if entry.shares.values().any(|s| !(s.is_finite() && *s >= 0.0)) || total <= 0.0 {
                return Err(format!(
                    "subnational fuel mix table: {code} has invalid shares"
                ));
            }
            mix_out.insert(
                code,
                entry
                    .shares
                    .into_iter()
                    .map(|(f, s)| (f, s / total))
                    .collect(),
            );
        }
        Ok(Self {
            year: file.year,
            country: file.country,
            demand_sha256: digest(demand),
            fuel_mix_sha256: digest(mix),
            demand: entries,
            mix: mix_out,
        })
    }

    /// The bundled tables, parsed once per process.
    pub fn bundled() -> &'static Subnational {
        static TABLE: OnceLock<Subnational> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(
                include_bytes!("../data/subnational_demand.json"),
                include_bytes!("../data/subnational_fuel_mix.json"),
            )
            .expect("the bundled subnational tables parse")
        })
    }

    pub fn demand(&self, code: &str) -> Option<&SubdivisionEntry> {
        self.demand.get(code)
    }

    /// The capacity mix of a subdivision, shares summing to 1.
    pub fn fuel_mix(&self, code: &str) -> Option<&BTreeMap<Fuel, f64>> {
        self.mix.get(code)
    }

    pub fn len(&self) -> usize {
        self.demand.len()
    }

    pub fn is_empty(&self) -> bool {
        self.demand.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_tables_cover_every_state() {
        let t = Subnational::bundled();
        assert_eq!(t.len(), 51);
        let ca = t.demand("US-CA").unwrap();
        assert!(ca.twh > 200.0 && ca.population > 3.5e7);
        assert!(ca.peak_factor.is_some_and(|f| (1.5..2.1).contains(&f)));
        let mix = t.fuel_mix("US-CA").unwrap();
        assert!((mix.values().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(mix[&Fuel::Solar] > mix[&Fuel::Coal]);
        assert!(t.demand("US-ZZ").is_none());
    }
}
