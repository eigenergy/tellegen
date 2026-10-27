//! Bus siting: the most populous places inside the region anchor buses in
//! population order, blue-noise fill supplies the rest, and population rank
//! decides each bus's voltage tier and role.

use crate::context::ContextData;
use crate::geom::bluenoise::blue_noise_points;
use crate::report::{BusKind, ElementSource, RegionContext};
use crate::rng::Stream;

/// A sited bus before electrification. Longitudes are in the region's
/// continuous frame until output.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftBus {
    pub lon: f64,
    pub lat: f64,
    pub name: String,
    /// The anchoring place, when any.
    pub place: Option<String>,
    /// 0 = highest voltage level.
    pub tier: usize,
    pub base_kv: f64,
    pub kind: BusKind,
    pub confidence: f64,
    /// Containing polygon (an index into the shape's polygons).
    pub polygon: usize,
    /// Electrical island (polygon group) index.
    pub island: usize,
    pub source: ElementSource,
    pub population_weight: f64,
    pub load_mw: f64,
    pub load_mvar: f64,
    pub annotation: String,
}

/// Rank thresholds for voltage tiers: the top 12% of buses by population
/// rank anchor the highest level, the next 28% the second, the rest the
/// lowest. With two levels the split is 12/88. source: the ACTIVSg2000 case
/// has 8% of buses at 500 kV, 29% at 230 kV and the rest at 115/161 kV
/// (Birchfield et al. 2017, Table II).
pub fn tier_for_rank(levels: usize, rank: f64) -> usize {
    if levels <= 1 || rank < 0.12 {
        0
    } else if rank < 0.40 || levels == 2 {
        1
    } else {
        levels - 1
    }
}

/// Site `ctx.target_bus_count` buses. Deterministic for a seed.
pub fn site_buses(ctx: &RegionContext, data: &ContextData, rng: &mut Stream) -> Vec<DraftBus> {
    let n = ctx.target_bus_count as usize;
    let levels = &ctx.voltage_levels_kv;
    let ncity = data.cities.len().min(n);
    let anchors: Vec<(f64, f64)> = data.cities[..ncity]
        .iter()
        .map(|c| (c.lon, c.lat))
        .collect();
    let fill = blue_noise_points(&data.shape, &data.polygon_weights, n - ncity, &anchors, rng);

    let mut coords: Vec<(f64, f64, Option<String>)> = Vec::with_capacity(n);
    for c in &data.cities[..ncity] {
        coords.push((c.lon, c.lat, Some(c.place.name.clone())));
    }
    for (lon, lat) in fill {
        coords.push((lon, lat, None));
    }
    // Coincident coordinates (two places with one position) are nudged so the
    // triangulation sees distinct points.
    let mut seen = std::collections::HashSet::<(i64, i64)>::new();
    for c in coords.iter_mut() {
        let mut key = ((c.0 * 1e6).round() as i64, (c.1 * 1e6).round() as i64);
        let mut bumps = 0;
        while !seen.insert(key) && bumps < 100 {
            c.0 += 1e-4;
            c.1 += 7e-5;
            key = ((c.0 * 1e6).round() as i64, (c.1 * 1e6).round() as i64);
            bumps += 1;
        }
    }

    let total = coords.len();
    let mut buses = Vec::with_capacity(total);
    for (i, (mut lon, mut lat, name)) in coords.into_iter().enumerate() {
        if !data.shape.contains(lon, lat) {
            let p = data.shape.sample_point(rng, &data.polygon_weights);
            lon = p.0;
            lat = p.1;
        }
        let polygon = data.shape.polygon_index(lon, lat).unwrap_or(0);
        let island = data.polygon_group[polygon].unwrap_or(0);
        let rank = if total <= 1 {
            0.0
        } else {
            i as f64 / (total - 1) as f64
        };
        let tier = tier_for_rank(levels.len(), rank);
        let kind = if rank < 0.16 {
            BusKind::Substation
        } else {
            BusKind::Load
        };
        let (confidence, annotation) = match &name {
            Some(place) => (
                0.82 + 0.12 * rng.uniform(),
                format!("Bus anchored on {place}"),
            ),
            None => (
                0.58 + 0.18 * rng.uniform(),
                "Synthetic substation placed by blue-noise siting".to_owned(),
            ),
        };
        buses.push(DraftBus {
            lon,
            lat,
            name: name
                .clone()
                .unwrap_or_else(|| format!("Substation {:03}", i + 1)),
            place: name,
            tier,
            base_kv: levels[tier],
            kind,
            confidence,
            polygon,
            island,
            source: ElementSource::Synthetic,
            population_weight: 0.0,
            load_mw: 0.0,
            load_mvar: 0.0,
            annotation,
        });
    }
    buses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::resolve_context;
    use crate::places::Places;
    use crate::request::{Region, SynthRequest};
    use crate::rng::StreamTag;

    #[test]
    fn tiers_follow_rank() {
        assert_eq!(tier_for_rank(3, 0.0), 0);
        assert_eq!(tier_for_rank(3, 0.2), 1);
        assert_eq!(tier_for_rank(3, 0.9), 2);
        assert_eq!(tier_for_rank(2, 0.9), 1);
        assert_eq!(tier_for_rank(1, 0.9), 0);
    }

    #[test]
    fn cities_anchor_first_and_fill_completes_the_count() {
        let places = Places::parse(include_bytes!("../tests/fixtures/places_small.json")).unwrap();
        let req = SynthRequest::new(Region::bbox(42.1, 31.1, 48.5, 36.8, "Iraq box"));
        let (ctx, data) = resolve_context(&req, &places, &mut Vec::new()).unwrap();
        let mut rng = Stream::new(req.seed, StreamTag::Site);
        let buses = site_buses(&ctx, &data, &mut rng);
        assert_eq!(buses.len(), ctx.target_bus_count as usize);
        assert_eq!(buses[0].name, "Baghdad");
        assert_eq!(buses[0].tier, 0);
        assert!(buses.iter().any(|b| b.place.is_none()));
        assert!(buses.iter().all(|b| data.shape.contains(b.lon, b.lat)));
        let mut rng2 = Stream::new(req.seed, StreamTag::Site);
        assert_eq!(buses, site_buses(&ctx, &data, &mut rng2));
    }
}
