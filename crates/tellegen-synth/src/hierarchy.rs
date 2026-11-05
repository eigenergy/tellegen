//! Hierarchical networks, for ladders that reach subtransmission (a level
//! below [`SUBTRANSMISSION_KV`]). A transmission-only model can stand one bus
//! for a whole substation and join buses of different voltage with a line
//! and a step-down transformer in one branch. Once subtransmission is
//! modelled the levels have to be kept apart, as they are in a real grid:
//!
//! - **Substations hold several levels.** A share of the buses of each level
//!   also steps down at its site to the next level (or the one below it),
//!   through a transformer between co-located buses.
//! - **Lines join buses of one level.** The top level is a meshed network over
//!   all of its buses ([`crate::topology`]'s spanning trees, Gabriel mesh and
//!   loops).
//! - **Lower levels form pockets.** Every lower-level bus belongs to the
//!   pocket of the nearest bus fed from above (its source), by distance along
//!   the level's Delaunay graph. Each pocket is a spanning tree with a few
//!   extra corridors, and a share of neighbouring pockets are tied. This is
//!   how subtransmission is laid out: radial or lightly looped feeders hanging
//!   off step-down substations, not a second meshed grid in parallel with the
//!   transmission one (Birchfield et al. 2017, Section IV-B: lines per
//!   substation stay near 1.1–1.4 at every level).
//! - **Taps.** Where a pocket tree bends sharply (below 120°), the bend is
//!   replaced by a tap junction at the Fermat point of the three buses. This
//!   is the classic Steiner improvement of a spanning tree. It gives the
//!   T-junctions real subtransmission is full of: a substation hangs off a
//!   passing line on a short spur.
//!
//! Sites, levels, pockets and taps all derive from Delaunay triangulations
//! and shortest paths on them, so the stage stays O(n log n).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::context::ContextData;
use crate::geom::bluenoise::weighted_blue_noise_points;
use crate::geom::delaunay::{delaunay, minimum_spanning_forest, Edge, UnionFind};
use crate::geom::{haversine_km, Projection};
use crate::report::{BusKind, ElementSource, RegionContext};
use crate::rng::Stream;
use crate::site::DraftBus;
use crate::topology::{corridor_between, Corridor, CorridorRole, TopologyOutput};
use crate::tuning::{level_shares, per_level, tuning};

/// Nominal voltage below which a level is subtransmission, kV. source: IEC
/// 60038 and ANSI C84.1 place subtransmission at 34.5–69 kV (up to 115/138
/// kV in some utilities); every national transmission model of the voltage
/// table stops at 110 kV or above.
pub const SUBTRANSMISSION_KV: f64 = 100.0;

/// Length given to the short bus-to-bus run inside a substation that carries
/// a transformer, km (the same as mapped OpenStreetMap sites,
/// `crate::osm::build`).
pub const SITE_TRANSFORMER_KM: f64 = 0.3;

/// Shortest line between two sites, km.
const MIN_LINE_KM: f64 = 0.5;

/// Whether a ladder reaches subtransmission.
pub fn is_hierarchical(levels: &[f64]) -> bool {
    levels.len() >= 2 && levels.iter().any(|&kv| kv < SUBTRANSMISSION_KV)
}

/// Whether a region is built hierarchically: its ladder comes from a
/// subdivision's entry ([`crate::voltage`]) and reaches subtransmission. A
/// request that states its own levels keeps the transmission design, one bus
/// per substation, whatever the levels.
pub fn layered(ctx: &RegionContext) -> bool {
    ctx.voltage_basis == crate::report::VoltageBasis::Subdivision
        && is_hierarchical(&ctx.voltage_levels_kv)
}

/// Bus counts per level: the tuned shares of the ladder depth, summing to `n`.
fn level_counts(n: usize, depth: usize) -> Vec<usize> {
    let shares = level_shares(depth);
    let mut counts: Vec<usize> = shares
        .iter()
        .map(|s| (s * n as f64).floor() as usize)
        .collect();
    // Largest remainders first, deepest level on ties.
    let mut rest: Vec<(f64, usize)> = shares
        .iter()
        .enumerate()
        .map(|(k, s)| (s * n as f64 - counts[k] as f64, k))
        .collect();
    rest.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut missing = n - counts.iter().sum::<usize>();
    for (_, k) in rest {
        if missing == 0 {
            break;
        }
        counts[k] += 1;
        missing -= 1;
    }
    // Every level the ladder names gets at least one bus when there is room.
    for k in 0..depth {
        if counts[k] == 0 {
            if let Some(donor) = (0..depth)
                .max_by_key(|&j| counts[j])
                .filter(|&j| counts[j] > 1)
            {
                counts[donor] -= 1;
                counts[k] += 1;
            }
        }
    }
    counts
}

/// Fisher-Yates shuffle on the session's stream.
fn shuffle<T>(v: &mut [T], rng: &mut Stream) {
    for i in (1..v.len()).rev() {
        let j = rng.index(i + 1);
        v.swap(i, j);
    }
}

/// Reorder a ranked list loosely: each item's rank is stretched by a random
/// factor in [1, 3), so the head stays mostly ranked without being a strict
/// cut.
fn rank_jitter(v: &mut Vec<usize>, rng: &mut Stream) {
    let mut keyed: Vec<(f64, usize)> = v
        .iter()
        .enumerate()
        .map(|(r, &i)| ((r as f64 + 1.0) * (1.0 + 2.0 * rng.uniform()), i))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    *v = keyed.into_iter().map(|(_, i)| i).collect();
}

/// A sited bus slot before it becomes a [`DraftBus`].
struct Slot {
    lon: f64,
    lat: f64,
    place: Option<String>,
    tier: usize,
    site: usize,
}

/// Site `ctx.target_bus_count` buses over the levels of a hierarchical
/// ladder. Deterministic for a seed.
pub fn site_hierarchical(
    ctx: &RegionContext,
    data: &ContextData,
    rng: &mut Stream,
) -> Vec<DraftBus> {
    let t = tuning();
    let levels = &ctx.voltage_levels_kv;
    let depth = levels.len();
    let n = ctx.target_bus_count as usize;
    let want = level_counts(n, depth);

    // Co-located step-down buses each level receives from the levels above.
    let mut colocated = vec![0usize; depth];
    let mut step_down = vec![0usize; depth];
    let mut step_skip = vec![0usize; depth];
    for k in 0..depth {
        if k + 1 < depth {
            let room = want[k + 1].saturating_sub(colocated[k + 1]);
            step_down[k] = ((per_level(&t.step_down, k) * want[k] as f64).round() as usize)
                .min(want[k])
                .min(room);
            colocated[k + 1] += step_down[k];
        }
        if k + 2 < depth {
            let room = want[k + 2].saturating_sub(colocated[k + 2]);
            step_skip[k] = ((per_level(&t.step_skip, k) * want[k] as f64).round() as usize)
                .min(want[k] - step_down[k])
                .min(room);
            colocated[k + 2] += step_skip[k];
        }
    }
    let standalone: Vec<usize> = (0..depth).map(|k| want[k] - colocated[k]).collect();
    let sites_total: usize = standalone.iter().sum();

    // Candidate site positions: places by population, then fill weighted by
    // the population density around places.
    let ncity = data.cities.len().min(sites_total);
    let anchors: Vec<(f64, f64)> = data.cities[..ncity]
        .iter()
        .map(|c| (c.lon, c.lat))
        .collect();
    let fill = weighted_blue_noise_points(
        &data.shape,
        &data.polygon_weights,
        sites_total - ncity,
        &anchors,
        &data.cities,
        t.fill_bandwidth_km,
        t.fill_floor,
        rng,
    );
    let mut candidates: Vec<(f64, f64, Option<String>)> = data.cities[..ncity]
        .iter()
        .map(|c| (c.lon, c.lat, Some(c.place.name.clone())))
        .collect();
    candidates.extend(fill.into_iter().map(|(lon, lat)| (lon, lat, None)));

    // Levels of the standalone sites: the top levels spread out in rank
    // order at their spacing, the rest drawn without replacement.
    let proj = Projection::for_frame(&data.shape.frame);
    let xy: Vec<(f64, f64)> = candidates.iter().map(|c| proj.project(c.0, c.1)).collect();
    let mut level_of: Vec<Option<usize>> = vec![None; candidates.len()];
    let area = data.shape.area_km2.max(1.0);
    for (k, &count) in standalone.iter().enumerate().take(t.level_spacing.len()) {
        if count == 0 {
            continue;
        }
        let spacing = t.level_spacing[k] * (area / count as f64).sqrt();
        let mut chosen: Vec<(f64, f64)> = Vec::new();
        let mut taken = 0;
        for pass in 0..2 {
            for i in 0..candidates.len() {
                if taken == count {
                    break;
                }
                if level_of[i].is_some() {
                    continue;
                }
                let far = pass == 1
                    || chosen
                        .iter()
                        .all(|p| (p.0 - xy[i].0).hypot(p.1 - xy[i].1) >= spacing);
                if far {
                    level_of[i] = Some(k);
                    chosen.push(xy[i]);
                    taken += 1;
                }
            }
        }
    }
    let mut pool: Vec<usize> = Vec::new();
    for (k, &count) in standalone
        .iter()
        .enumerate()
        .skip(t.level_spacing.len().min(depth))
    {
        pool.extend(std::iter::repeat_n(k, count));
    }
    shuffle(&mut pool, rng);
    let mut next = pool.into_iter();
    for slot in level_of.iter_mut().filter(|l| l.is_none()) {
        *slot = Some(next.next().unwrap_or(depth - 1));
    }

    let mut slots: Vec<Slot> = candidates
        .into_iter()
        .zip(&level_of)
        .enumerate()
        .map(|(i, ((lon, lat, place), level))| Slot {
            lon,
            lat,
            place,
            tier: level.unwrap_or(depth - 1),
            site: i,
        })
        .collect();

    // Step-down buses at the sites of each level, highest ranked first, a
    // little apart so the triangulation sees distinct points.
    let offset_deg = t.site_offset_km / 111.0;
    for k in 0..depth {
        let mut at_level: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].tier == k).collect();
        // Ranked sites (populous places) get transformation first; a light
        // shuffle keeps it from being a strict population cut.
        rank_jitter(&mut at_level, rng);
        let mut steps: Vec<(usize, usize)> = Vec::new();
        for (j, &i) in at_level.iter().enumerate() {
            if j < step_down[k] {
                steps.push((i, k + 1));
            } else if j < step_down[k] + step_skip[k] {
                steps.push((i, k + 2));
            }
        }
        for (i, lower) in steps {
            let (lon, lat, site) = (slots[i].lon, slots[i].lat, slots[i].site);
            let nth = slots.iter().filter(|s| s.site == site).count() as f64;
            slots.push(Slot {
                lon: lon + offset_deg * nth / lat.to_radians().cos().abs().max(0.05),
                lat,
                place: slots[i].place.clone(),
                tier: lower,
                site,
            });
        }
    }

    let mut buses = Vec::with_capacity(slots.len());
    let site_size: BTreeMap<usize, usize> = slots.iter().fold(BTreeMap::new(), |mut m, s| {
        *m.entry(s.site).or_default() += 1;
        m
    });
    for (i, slot) in slots.into_iter().enumerate() {
        let (mut lon, mut lat) = (slot.lon, slot.lat);
        if !data.shape.contains(lon, lat) {
            let p = data.shape.sample_point(rng, &data.polygon_weights);
            lon = p.0;
            lat = p.1;
        }
        let polygon = data.shape.polygon_index(lon, lat).unwrap_or(0);
        let island = data.polygon_group[polygon].unwrap_or(0);
        let kv = levels[slot.tier];
        let multi = site_size[&slot.site] > 1;
        let kind = if multi || slot.tier == 0 {
            BusKind::Substation
        } else {
            BusKind::Load
        };
        let (confidence, annotation) = match &slot.place {
            Some(place) => (
                0.8 + 0.12 * rng.uniform(),
                format!("{kv} kV bus anchored on {place}"),
            ),
            None => (
                0.55 + 0.18 * rng.uniform(),
                format!("Synthetic {kv} kV substation placed by population-weighted siting"),
            ),
        };
        let name = match &slot.place {
            Some(place) if multi => format!("{place} {kv} kV"),
            Some(place) => place.clone(),
            None => format!("Substation {:04} {kv} kV", slot.site + 1),
        };
        buses.push(DraftBus {
            lon,
            lat,
            name,
            place: slot.place,
            tier: slot.tier,
            base_kv: kv,
            kind,
            confidence,
            polygon,
            island,
            source: ElementSource::Synthetic,
            population_weight: 0.0,
            load_mw: 0.0,
            load_mvar: 0.0,
            annotation,
            site: slot.site,
        });
        let _ = i;
    }
    buses
}

/// Blend nearest-bus demand into the loads `assign_loads` spread by inverse
/// distance: each place's peak goes to the bus nearest it, for
/// [`crate::tuning::Tuning::nearest_load_share`] of the region's demand. With
/// buses already sited by population, inverse-distance weights alone count a
/// city twice (more buses there, each weighted by the same people).
pub fn blend_place_loads(buses: &mut [DraftBus], ctx: &RegionContext, data: &ContextData) {
    let share = tuning().nearest_load_share.clamp(0.0, 1.0);
    let total: f64 = data.cities.iter().map(|c| c.demand_weight.max(0.0)).sum();
    if share <= 0.0 || total <= 0.0 || buses.is_empty() {
        return;
    }
    let proj = Projection::for_frame(&data.shape.frame);
    let xy: Vec<(f64, f64)> = buses.iter().map(|b| proj.project(b.lon, b.lat)).collect();
    let mut nearest = vec![0.0; buses.len()];
    for c in &data.cities {
        let p = proj.project(c.lon, c.lat);
        let best = (0..buses.len())
            .filter(|&i| buses[i].kind != BusKind::Converter && buses[i].kind != BusKind::Tap)
            .min_by(|&i, &j| dist(xy[i], p).total_cmp(&dist(xy[j], p)).then(i.cmp(&j)));
        if let Some(i) = best {
            nearest[i] += ctx.peak_demand_mw * c.demand_weight.max(0.0) / total;
        }
    }
    for (b, n) in buses.iter_mut().zip(nearest) {
        b.load_mw = (1.0 - share) * b.load_mw + share * n;
    }
}

/// Voltage regulation at every step-down secondary of a hierarchical
/// network: the lower bus of each site transformer below the bulk levels
/// holds its voltage, as a load tap changer with the substation's capacitor
/// banks does, modelled as a synchronous condenser with ±
/// [`crate::electrical::params::VOLTAGE_SUPPORT_Q_SHARE`] MVAr per MW of the
/// load nearest it (along corridors). source: subtransmission and
/// distribution step-down transformers carry load tap changers that regulate
/// the secondary voltage (IEEE C57.131; Kundur, *Power System Stability and
/// Control*, Section 3.7); the reactive plan's decoupled model would miss the
/// P·R drop of 60–115 kV lines, whose X/R is 2–5. The California Test System
/// carries 1,743 synchronous condensers to the same end.
///
/// A designed network keeps every subtransmission bus within
/// [`crate::tuning::Tuning::pocket_reach_km`] of its step-down by splitting
/// pockets. A mapped one (`mapped`) is taken as it is, so there the same
/// reach is kept by regulation instead: while a loaded bus below the bulk
/// levels lies farther along corridors than that reach (scaled with its
/// voltage) from every regulated bus, the farthest is regulated too, as the
/// substation capacitor banks and tap changers of a long rural feeder are.
pub fn regulate_secondaries(
    buses: &mut [DraftBus],
    corridors: &[Corridor],
    mapped: bool,
) -> Vec<crate::electrical::generators::DraftGenerator> {
    let n = buses.len();
    let mut sources: Vec<usize> = corridors
        .iter()
        .filter(|c| {
            (c.length_km - SITE_TRANSFORMER_KM).abs() < 1e-9
                && buses[c.a].base_kv != buses[c.b].base_kv
        })
        .map(|c| {
            if buses[c.a].base_kv < buses[c.b].base_kv {
                c.a
            } else {
                c.b
            }
        })
        .filter(|&i| buses[i].base_kv < 200.0)
        .collect();
    sources.sort_unstable();
    sources.dedup();
    if sources.is_empty() {
        return Vec::new();
    }
    if mapped {
        let reach = |i: usize| tuning().pocket_reach_km * buses[i].base_kv / 66.0;
        loop {
            let (dist, _) =
                crate::electrical::reactive::distance_to_sources(n, corridors, &sources);
            let farthest = (0..n)
                .filter(|&i| {
                    buses[i].load_mw > 0.0
                        && buses[i].base_kv < 200.0
                        && dist[i].is_finite()
                        && dist[i] > reach(i)
                })
                .max_by(|&a, &b| {
                    (dist[a] / reach(a))
                        .total_cmp(&(dist[b] / reach(b)))
                        .then(b.cmp(&a))
                });
            match farthest {
                Some(i) => sources.push(i),
                None => break,
            }
        }
    }
    let (_, from) = crate::electrical::reactive::distance_to_sources(n, corridors, &sources);
    let mut served = vec![0.0; n];
    for i in 0..n {
        if from[i] != usize::MAX {
            served[from[i]] += buses[i].load_mw;
        }
    }
    sources
        .into_iter()
        .filter(|&b| served[b] > 0.0)
        .map(|bus| {
            let q = (crate::electrical::params::VOLTAGE_SUPPORT_Q_SHARE * served[bus])
                .max(crate::electrical::params::MIN_SHUNT_BANK_MVAR);
            buses[bus].annotation = format!(
                "{} (secondary voltage regulated, modelled as a condenser)",
                buses[bus].annotation
            );
            crate::electrical::generators::DraftGenerator {
                bus,
                fuel: crate::request::Fuel::Other,
                pmax_mw: 0.0,
                pmin_mw: 0.0,
                qmax_mvar: q,
                qmin_mvar: -q,
                c2: 0.0,
                c1: 0.0,
                c0: 0.0,
                makeup: false,
                slack: false,
                condenser: true,
            }
        })
        .collect()
}

/// Farthest a new step-down added to a mapped network reaches for its higher
/// level, km.
const RELIEF_REACH_KM: f64 = 30.0;

/// New step-downs where a mapped network cannot carry its planning case: for
/// every corridor still overloaded after the plan, the bus it feeds (below
/// the top level) gets a line and transformer to the nearest bus of a higher
/// level within [`RELIEF_REACH_KM`] it is not yet joined to, as a
/// planner adds a substation where load outgrows its feed. Mapped data often
/// misses the step-downs that feed an urban subtransmission mesh; this puts
/// back the capacity, not the exact equipment. Returns the corridors added.
pub fn relieve_mapped(
    buses: &[DraftBus],
    corridors: &mut Vec<Corridor>,
    designs: &[crate::electrical::branches::CircuitDesign],
    plan: &crate::electrical::planning::CorridorPlan,
) -> usize {
    let mut existing: BTreeSet<(usize, usize)> = corridors
        .iter()
        .map(|c| (c.a.min(c.b), c.a.max(c.b)))
        .collect();
    let top = buses.iter().map(|b| b.base_kv).fold(0.0, f64::max);
    let mut receivers: BTreeSet<usize> = BTreeSet::new();
    for (k, d) in designs.iter().enumerate() {
        let circuits = plan.circuits.get(k).copied().unwrap_or(1).max(1) as f64;
        let flow = plan.corridor_flow.get(k).copied().unwrap_or(0.0);
        if flow.abs() <= circuits * d.rate_a {
            continue;
        }
        // The end power flows into: the lower bus of a step-down, the
        // receiving end of a line. The top level has nothing above it.
        let r = if d.is_transformer || flow > 0.0 {
            d.to
        } else {
            d.from
        };
        if buses[r].base_kv < top - 1.0 {
            receivers.insert(r);
        }
    }
    let mut added = 0;
    for r in receivers {
        let (lon, lat, kv, island) = (
            buses[r].lon,
            buses[r].lat,
            buses[r].base_kv,
            buses[r].island,
        );
        let best = (0..buses.len())
            .filter(|&j| {
                buses[j].island == island
                    && buses[j].base_kv > kv + 1.0
                    && buses[j].kind != BusKind::Converter
                    && !existing.contains(&(r.min(j), r.max(j)))
            })
            .map(|j| (haversine_km(lon, lat, buses[j].lon, buses[j].lat), j))
            .filter(|(d, _)| *d <= RELIEF_REACH_KM)
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let Some((d, j)) = best else { continue };
        let (a, b) = (r.min(j), r.max(j));
        if !existing.insert((a, b)) {
            continue;
        }
        corridors.push(Corridor {
            a,
            b,
            kv: buses[a].base_kv.max(buses[b].base_kv),
            length_km: d.max(SITE_TRANSFORMER_KM),
            role: CorridorRole::Internal,
            circuits: 1,
            route: vec![(buses[a].lon, buses[a].lat), (buses[b].lon, buses[b].lat)],
            is_delaunay: false,
        });
        added += 1;
    }
    added
}

/// Move the load of every bus that has a lower-level bus at its site onto
/// the lowest one: a substation serves its load from its lowest voltage.
pub fn settle_site_loads(buses: &mut [DraftBus]) {
    let mut lowest: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, b) in buses.iter().enumerate() {
        let e = lowest.entry(b.site).or_insert(i);
        if b.tier > buses[*e].tier {
            *e = i;
        }
    }
    for i in 0..buses.len() {
        let low = lowest[&buses[i].site];
        if low != i {
            let (mw, mvar, w) = (
                buses[i].load_mw,
                buses[i].load_mvar,
                buses[i].population_weight,
            );
            buses[low].load_mw += mw;
            buses[low].load_mvar += mvar;
            buses[low].population_weight += w;
            buses[i].load_mw = 0.0;
            buses[i].load_mvar = 0.0;
            buses[i].population_weight = 0.0;
        }
    }
}

/// A min-heap entry for the multi-source shortest paths.
#[derive(PartialEq)]
struct Visit(f64, usize, usize);

impl Eq for Visit {}

impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .total_cmp(&self.0)
            .then(other.1.cmp(&self.1))
            .then(other.2.cmp(&self.2))
    }
}

/// The Fermat point of a triangle with no angle of 120° or more
/// (Weiszfeld's iteration from the centroid).
fn fermat_point(p: [(f64, f64); 3]) -> (f64, f64) {
    let mut x = (
        (p[0].0 + p[1].0 + p[2].0) / 3.0,
        (p[0].1 + p[1].1 + p[2].1) / 3.0,
    );
    for _ in 0..40 {
        let (mut nx, mut ny, mut w) = (0.0, 0.0, 0.0);
        for q in p {
            let d = (q.0 - x.0).hypot(q.1 - x.1).max(1e-9);
            nx += q.0 / d;
            ny += q.1 / d;
            w += 1.0 / d;
        }
        x = (nx / w, ny / w);
    }
    x
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Replace sharp bends of a forest by tap junctions. `points` holds every
/// bus's projected position and grows by the taps; `edges` are undirected
/// global index pairs. Returns the taps as (position, level member it was
/// made for).
fn steinerize(
    points: &mut Vec<(f64, f64)>,
    edges: &mut BTreeSet<Edge>,
    nodes: &[usize],
    rng: &mut Stream,
) -> Vec<usize> {
    let t = tuning();
    let mut taps = Vec::new();
    let mut adj: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for e in edges.iter() {
        adj.entry(e.0).or_default().insert(e.1);
        adj.entry(e.1).or_default().insert(e.0);
    }
    for _pass in 0..2 {
        for &v in nodes {
            let Some(nbrs) = adj.get(&v) else { continue };
            if nbrs.len() < 2 {
                continue;
            }
            let nb: Vec<usize> = nbrs.iter().copied().collect();
            let pv = points[v];
            let mut best: Option<(f64, usize, usize, (f64, f64))> = None;
            for i in 0..nb.len() {
                for j in i + 1..nb.len() {
                    let (a, b) = (nb[i], nb[j]);
                    let (pa, pb) = (points[a], points[b]);
                    let (ax, ay) = (pa.0 - pv.0, pa.1 - pv.1);
                    let (bx, by) = (pb.0 - pv.0, pb.1 - pv.1);
                    let (la, lb) = (ax.hypot(ay), bx.hypot(by));
                    if la <= 0.0 || lb <= 0.0 {
                        continue;
                    }
                    let cos = (ax * bx + ay * by) / (la * lb);
                    // Every angle of the triangle must be below 120° for an
                    // interior Fermat point; the one at v is checked here and
                    // the others by the saving.
                    if cos <= -0.5 {
                        continue;
                    }
                    let s = fermat_point([pv, pa, pb]);
                    let saving = la + lb - (dist(s, pv) + dist(s, pa) + dist(s, pb));
                    if saving > t.tap_min_saving_km && best.is_none_or(|(bs, ..)| saving > bs) {
                        best = Some((saving, a, b, s));
                    }
                }
            }
            let Some((_, a, b, fermat)) = best else {
                continue;
            };
            if rng.uniform() >= t.tap_share {
                continue;
            }
            // At the substation fence, or where the three sections meet.
            let s = if rng.uniform() < t.tap_at_site_share {
                let d = dist(fermat, pv).max(1e-9);
                let k = (t.tap_spur_km / d).min(1.0);
                (pv.0 + k * (fermat.0 - pv.0), pv.1 + k * (fermat.1 - pv.1))
            } else {
                fermat
            };
            let tap = points.len();
            points.push(s);
            for w in [a, b] {
                edges.remove(&Edge::new(v, w));
                adj.get_mut(&v).unwrap().remove(&w);
                adj.get_mut(&w).unwrap().remove(&v);
            }
            for w in [v, a, b] {
                edges.insert(Edge::new(tap, w));
                adj.entry(tap).or_default().insert(w);
                adj.entry(w).or_default().insert(tap);
            }
            taps.push(v);
        }
    }
    taps
}

/// A tap junction bus at (lon, lat) on the level of bus `v`.
fn tap_bus(buses: &[DraftBus], v: usize, lon: f64, lat: f64, data: &ContextData) -> DraftBus {
    let src = &buses[v];
    let n = buses.iter().filter(|b| b.kind == BusKind::Tap).count();
    DraftBus {
        lon,
        lat,
        name: format!("Tap {:04} {} kV", n + 1, src.base_kv),
        place: None,
        tier: src.tier,
        base_kv: src.base_kv,
        kind: BusKind::Tap,
        confidence: 0.5,
        polygon: data.shape.polygon_index(lon, lat).unwrap_or(src.polygon),
        island: src.island,
        source: ElementSource::Synthetic,
        population_weight: 0.0,
        load_mw: 0.0,
        load_mvar: 0.0,
        annotation: "Tap junction where a line branches to a substation".to_owned(),
        site: usize::MAX - buses.len(),
    }
}

/// Load each member of a level serves: its own and that of every bus of its
/// level or below whose nearest member it is (the lower networks hang off
/// the level near where they are).
fn catchment_loads(
    buses: &[DraftBus],
    members: &[usize],
    points: &[(f64, f64)],
    tier: usize,
) -> Vec<f64> {
    let mut host = vec![0.0; members.len()];
    if members.is_empty() {
        return host;
    }
    let island = buses[members[0]].island;
    // A coarse grid over the members keeps the nearest search local.
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for &i in members {
        let p = points[i];
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    let cell = (((x1 - x0) * (y1 - y0)).max(1.0) / members.len() as f64)
        .sqrt()
        .max(1.0)
        * 2.0;
    let cols = (((x1 - x0) / cell).ceil() as usize + 1).min(4096);
    let rows = (((y1 - y0) / cell).ceil() as usize + 1).min(4096);
    let key = |p: (f64, f64)| {
        let c = (((p.0 - x0) / cell).floor().max(0.0) as usize).min(cols - 1);
        let r = (((p.1 - y0) / cell).floor().max(0.0) as usize).min(rows - 1);
        (c, r)
    };
    let mut grid: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (l, &i) in members.iter().enumerate() {
        grid.entry(key(points[i])).or_default().push(l);
    }
    for (i, b) in buses.iter().enumerate() {
        if b.island != island || b.tier < tier || b.load_mw <= 0.0 {
            continue;
        }
        let (c, r) = key(points[i]);
        let mut best: Option<(f64, usize)> = None;
        let mut ring = 0usize;
        while ring <= cols.max(rows) {
            for rr in r.saturating_sub(ring)..=(r + ring).min(rows - 1) {
                for cc in c.saturating_sub(ring)..=(c + ring).min(cols - 1) {
                    if rr.abs_diff(r) != ring && cc.abs_diff(c) != ring {
                        continue;
                    }
                    for &l in grid.get(&(cc, rr)).map(Vec::as_slice).unwrap_or(&[]) {
                        let d = dist(points[members[l]], points[i]);
                        if best.is_none_or(|(bd, bl)| d < bd || (d == bd && l < bl)) {
                            best = Some((d, l));
                        }
                    }
                }
            }
            // Anything beyond the next ring is farther than a hit in this one.
            if best.is_some_and(|(d, _)| d <= ring as f64 * cell) {
                break;
            }
            ring += 1;
        }
        if let Some((_, l)) = best {
            host[l] += b.load_mw;
        }
    }
    host
}

/// Largest load a pocket at `kv` serves, MW: [`crate::tuning::Tuning::pocket_feeders`]
/// corridors at the circuit cap, at the planning utilisation.
fn pocket_capacity_mw(kv: f64) -> f64 {
    let (class, ratio) = crate::electrical::params::class_ratio(kv);
    tuning().pocket_feeders
        * crate::electrical::params::MAX_CIRCUITS_PER_CORRIDOR as f64
        * class.thermal_mva
        * ratio
        * crate::electrical::params::PLANNING_UTILISATION
}

/// Pocket of every member, the index of its nearest source by distance along
/// the level's Delaunay graph (multi-source shortest paths), and that
/// distance, km.
fn pockets_with_distance(
    tri: &crate::geom::delaunay::Triangulation,
    weight: &[f64],
    members: &[usize],
    sources: &[usize],
) -> (Vec<usize>, Vec<f64>) {
    let m = members.len();
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); m];
    for (e, &w) in tri.edges.iter().zip(weight) {
        adj[e.0].push((e.1, w));
        adj[e.1].push((e.0, w));
    }
    let local_of: BTreeMap<usize, usize> =
        members.iter().enumerate().map(|(l, &g)| (g, l)).collect();
    let mut owner = vec![usize::MAX; m];
    let mut best = vec![f64::INFINITY; m];
    let mut heap = BinaryHeap::new();
    for (p, &s) in sources.iter().enumerate() {
        let l = local_of[&s];
        best[l] = 0.0;
        heap.push(Visit(0.0, l, p));
    }
    while let Some(Visit(d, u, p)) = heap.pop() {
        if owner[u] != usize::MAX || d > best[u] {
            continue;
        }
        owner[u] = p;
        for &(v, w) in &adj[u] {
            let nd = d + w;
            if owner[v] == usize::MAX && nd < best[v] {
                best[v] = nd;
                heap.push(Visit(nd, v, p));
            }
        }
    }
    // A member the graph does not reach (a degenerate triangulation) joins
    // the first pocket.
    owner
        .iter_mut()
        .filter(|o| **o == usize::MAX)
        .for_each(|o| *o = 0);
    (owner, best)
}

/// Candidate corridors of one level: its Delaunay edges and the edges to
/// second Delaunay neighbours, each with its straight length and a noisy
/// length (a log-normal factor of σ `route_noise`) by which trees and meshes
/// choose. Local indices.
fn candidate_edges(
    tri: &crate::geom::delaunay::Triangulation,
    local: &[(f64, f64)],
    m: usize,
    rng: &mut Stream,
) -> Vec<(Edge, f64, f64)> {
    let sigma = tuning().route_noise.max(0.0);
    let adj = crate::geom::delaunay::adjacency(m, &tri.edges);
    let mut set: BTreeSet<Edge> = tri.edges.iter().copied().collect();
    for (u, nbrs) in adj.iter().enumerate() {
        for &v in nbrs {
            for &w in &adj[v] {
                if w != u {
                    set.insert(Edge::new(u, w));
                }
            }
        }
    }
    set.into_iter()
        .map(|e| {
            let len = dist(local[e.0], local[e.1]);
            let noisy = len * (sigma * rng.normal().clamp(-3.0, 3.0)).exp();
            (e, len, noisy)
        })
        .collect()
}

/// Tap substations a tree line passes straight through: with probability
/// [`crate::tuning::Tuning::tap_straight_share`], a degree-2 member `v` (not
/// in `protected`) between `a` and `b` is replaced on the line by a junction
/// at its fence, `tap_spur_km` to the side, which keeps `v` on a spur.
fn straight_taps(
    points: &mut Vec<(f64, f64)>,
    edges: &mut BTreeSet<Edge>,
    nodes: &[usize],
    protected: &BTreeSet<usize>,
    rng: &mut Stream,
) -> Vec<usize> {
    let t = tuning();
    let mut adj: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for e in edges.iter() {
        adj.entry(e.0).or_default().push(e.1);
        adj.entry(e.1).or_default().push(e.0);
    }
    let mut taps = Vec::new();
    for &v in nodes {
        if protected.contains(&v) {
            continue;
        }
        let Some(nbrs) = adj.get(&v).cloned() else {
            continue;
        };
        if nbrs.len() != 2 || rng.uniform() >= t.tap_straight_share {
            continue;
        }
        let (a, b) = (nbrs[0], nbrs[1]);
        let (pa, pb, pv) = (points[a], points[b], points[v]);
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = dx.hypot(dy).max(1e-9);
        let side = if rng.uniform() < 0.5 { 1.0 } else { -1.0 };
        let s = (
            pv.0 - dy / len * t.tap_spur_km * side,
            pv.1 + dx / len * t.tap_spur_km * side,
        );
        let tap = points.len();
        points.push(s);
        for w in [a, b] {
            edges.remove(&Edge::new(v, w));
            if let Some(l) = adj.get_mut(&w) {
                l.retain(|&x| x != v);
                l.push(tap);
            }
        }
        adj.insert(v, vec![tap]);
        adj.insert(tap, vec![a, b, v]);
        for w in [a, b, v] {
            edges.insert(Edge::new(tap, w));
        }
        taps.push(v);
    }
    taps
}

/// Corridors of a hierarchical network: site transformers, the top level's
/// mesh, and the pockets of every lower level with their taps. Taps are
/// appended to `buses`.
pub fn hierarchical_topology(
    buses: &mut Vec<DraftBus>,
    data: &ContextData,
    rng: &mut Stream,
) -> TopologyOutput {
    let t = tuning();
    let mut out = TopologyOutput {
        corridors: Vec::new(),
        interconnects: 0,
    };
    if buses.len() < 2 {
        return out;
    }
    let proj = Projection::for_frame(&data.shape.frame);
    let mut points: Vec<(f64, f64)> = buses.iter().map(|b| proj.project(b.lon, b.lat)).collect();

    // 1. Transformers inside multi-level sites: each bus to the nearest
    //    higher level at its site.
    let mut by_site: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, b) in buses.iter().enumerate() {
        by_site.entry(b.site).or_default().push(i);
    }
    let mut transformers: Vec<Edge> = Vec::new();
    let mut fed = vec![false; buses.len()];
    for members in by_site.values() {
        for &i in members {
            let upper = members
                .iter()
                .copied()
                .filter(|&j| buses[j].tier < buses[i].tier)
                .max_by_key(|&j| (buses[j].tier, std::cmp::Reverse(j)));
            if let Some(j) = upper {
                transformers.push(Edge::new(i, j));
                fed[i] = true;
            }
        }
    }

    // 2. Lines, island by island and level by level.
    let mut edges: BTreeSet<Edge> = BTreeSet::new();
    let mut delaunay_edges: BTreeSet<Edge> = BTreeSet::new();
    let mut islands: BTreeMap<usize, BTreeMap<usize, Vec<usize>>> = BTreeMap::new();
    for (i, b) in buses.iter().enumerate() {
        islands
            .entry(b.island)
            .or_default()
            .entry(b.tier)
            .or_default()
            .push(i);
    }
    for tiers in islands.values() {
        let top = *tiers.keys().next().expect("an island has a level");
        for (&tier, members) in tiers {
            if members.len() < 2 {
                continue;
            }
            let m = members.len();
            let sources: Vec<usize> = members.iter().copied().filter(|&i| fed[i]).collect();
            let meshed = tier == top || sources.is_empty();
            let local: Vec<(f64, f64)> = members.iter().map(|&i| points[i]).collect();
            let tri = delaunay(&local);
            let weight: Vec<f64> = tri
                .edges
                .iter()
                .map(|e| dist(local[e.0], local[e.1]))
                .collect();
            for e in &tri.edges {
                delaunay_edges.insert(Edge::new(members[e.0], members[e.1]));
            }
            let owner = if meshed {
                vec![0usize; m]
            } else {
                // Pockets heavier than their feeders can serve get another
                // source: a bus of the level above at their heaviest, farthest
                // site, tapping that level's network.
                let mut sources = sources.clone();
                let host = catchment_loads(buses, members, &points, tier);
                let cap = pocket_capacity_mw(buses[members[0]].base_kv);
                let upper = tiers
                    .range(..tier)
                    .next_back()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let reach = t.pocket_reach_km * buses[members[0]].base_kv / 66.0;
                let (mut owner, mut far) = pockets_with_distance(&tri, &weight, members, &sources);
                for _round in 0..24 {
                    let mut load: BTreeMap<usize, f64> = BTreeMap::new();
                    let mut span: BTreeMap<usize, f64> = BTreeMap::new();
                    for (l, &o) in owner.iter().enumerate() {
                        *load.entry(o).or_default() += host[l];
                        let r = span.entry(o).or_default();
                        *r = r.max(far[l]);
                    }
                    // Too heavy for its feeders, or reaching too far for its
                    // voltage to hold.
                    let heavy: Vec<(usize, bool)> = load
                        .iter()
                        .filter_map(|(&o, &w)| {
                            if w > cap {
                                Some((o, true))
                            } else if span[&o] > reach {
                                Some((o, false))
                            } else {
                                None
                            }
                        })
                        .collect();
                    if heavy.is_empty() || upper.is_empty() {
                        break;
                    }
                    for (o, by_load) in heavy {
                        let src = points[sources[o]];
                        let pick = (0..m)
                            .filter(|&l| owner[l] == o && !fed[members[l]])
                            .max_by(|&a, &b| {
                                let (ka, kb) = if by_load {
                                    (
                                        host[a] * dist(points[members[a]], src),
                                        host[b] * dist(points[members[b]], src),
                                    )
                                } else {
                                    (far[a], far[b])
                                };
                                ka.total_cmp(&kb).then(b.cmp(&a))
                            });
                        let Some(l) = pick else { continue };
                        let v = members[l];
                        let near = upper
                            .iter()
                            .copied()
                            .min_by(|&a, &b| {
                                dist(points[a], points[v])
                                    .total_cmp(&dist(points[b], points[v]))
                                    .then(a.cmp(&b))
                            })
                            .expect("an upper level");
                        let up = buses.len();
                        let mut bus = buses[near].clone();
                        bus.lon = buses[v].lon;
                        bus.lat = buses[v].lat + t.site_offset_km / 111.0;
                        bus.site = buses[v].site;
                        bus.place = buses[v].place.clone();
                        bus.name = format!("{} {} kV", buses[v].name, bus.base_kv);
                        bus.kind = BusKind::Substation;
                        bus.load_mw = 0.0;
                        bus.load_mvar = 0.0;
                        bus.population_weight = 0.0;
                        bus.annotation = format!(
                            "{} kV step-down added where the {} kV network's load outgrew its feeders",
                            bus.base_kv, buses[v].base_kv
                        );
                        buses.push(bus);
                        points.push(proj.project(buses[up].lon, buses[up].lat));
                        fed.push(false);
                        transformers.push(Edge::new(v, up));
                        edges.insert(Edge::new(up, near));
                        fed[v] = true;
                        sources.push(v);
                    }
                    (owner, far) = pockets_with_distance(&tri, &weight, members, &sources);
                }
                owner
            };
            let sources: Vec<usize> = members.iter().copied().filter(|&i| fed[i]).collect();
            let cand = candidate_edges(&tri, &local, m, rng);
            // Spanning tree of each pocket (the whole level when meshed) over
            // the candidate corridors at their noisy lengths.
            let inside: Vec<usize> = (0..cand.len())
                .filter(|&k| owner[cand[k].0 .0] == owner[cand[k].0 .1])
                .collect();
            let sub: Vec<Edge> = inside.iter().map(|&k| cand[k].0).collect();
            let sub_w: Vec<f64> = inside.iter().map(|&k| cand[k].2).collect();
            let mut tree: BTreeSet<Edge> = minimum_spanning_forest(m, &sub, &sub_w)
                .into_iter()
                .map(|k| Edge::new(members[sub[k].0], members[sub[k].1]))
                .collect();
            if !meshed {
                let before = points.len();
                let mut made = steinerize(&mut points, &mut tree, members, rng);
                let protected: BTreeSet<usize> = sources.iter().copied().collect();
                made.extend(straight_taps(
                    &mut points,
                    &mut tree,
                    members,
                    &protected,
                    rng,
                ));
                // Taps become buses at once, so bus and point indices agree.
                for (n, &v) in made.iter().enumerate() {
                    let (lon, lat) = proj.unproject(points[before + n].0, points[before + n].1);
                    let tap = tap_bus(buses, v, lon, lat, data);
                    buses.push(tap);
                    fed.push(false);
                }
                debug_assert_eq!(buses.len(), points.len());
            }
            // Corridors beyond the trees, shortest noisy length first, up to
            // the level's ratio in each pocket.
            let ratio = if tier == top {
                t.mesh_ratio - 1.0
            } else {
                per_level(&t.pocket_extra, tier - top - 1)
            };
            let mut left: BTreeMap<usize, usize> = BTreeMap::new();
            for &o in &owner {
                *left.entry(o).or_default() += 1;
            }
            for v in left.values_mut() {
                *v = (ratio.max(0.0) * *v as f64).round() as usize;
            }
            let mut degree: BTreeMap<usize, usize> = BTreeMap::new();
            for e in &tree {
                *degree.entry(e.0).or_default() += 1;
                *degree.entry(e.1).or_default() += 1;
            }
            let mut order = inside.clone();
            order.sort_by(|&a, &b| cand[a].2.total_cmp(&cand[b].2).then(a.cmp(&b)));
            for k in order {
                let e = cand[k].0;
                let g = Edge::new(members[e.0], members[e.1]);
                let slot = left.get_mut(&owner[e.0]).unwrap();
                if *slot == 0 || tree.contains(&g) {
                    continue;
                }
                if degree.get(&g.0).copied().unwrap_or(0) >= crate::topology::MAX_DEGREE
                    || degree.get(&g.1).copied().unwrap_or(0) >= crate::topology::MAX_DEGREE
                {
                    continue;
                }
                tree.insert(g);
                *slot -= 1;
                *degree.entry(g.0).or_default() += 1;
                *degree.entry(g.1).or_default() += 1;
            }
            if !meshed {
                // Ties between neighbouring pockets: the shortest Delaunay edge
                // of each adjacent pair, a share of them.
                let mut tie: BTreeMap<(usize, usize), (f64, Edge)> = BTreeMap::new();
                for (kk, e) in tri.edges.iter().enumerate() {
                    let (a, b) = (owner[e.0], owner[e.1]);
                    if a == b {
                        continue;
                    }
                    let key = (a.min(b), a.max(b));
                    let cand = (weight[kk], Edge::new(members[e.0], members[e.1]));
                    tie.entry(key)
                        .and_modify(|c| {
                            if cand.0 < c.0 {
                                *c = cand;
                            }
                        })
                        .or_insert(cand);
                }
                let share = per_level(&t.pocket_ties, tier - top - 1);
                for (_, (_, g)) in tie {
                    if rng.uniform() < share {
                        tree.insert(g);
                    }
                }
            }
            edges.extend(tree);
        }
    }

    debug_assert_eq!(buses.len(), points.len());

    // 4. Every component reaches the top level: join any that does not to
    //    its nearest bus of a higher level (a line and a transformer in one
    //    corridor, as the flat design does).
    let n = buses.len();
    let mut uf = UnionFind::new(n);
    for e in edges.iter().chain(&transformers) {
        uf.union(e.0, e.1);
    }
    let mut by_island: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, b) in buses.iter().enumerate() {
        by_island.entry(b.island).or_default().push(i);
    }
    for members in by_island.values() {
        let top = members.iter().map(|&i| buses[i].tier).min().unwrap_or(0);
        loop {
            let main = members
                .iter()
                .copied()
                .find(|&i| buses[i].tier == top)
                .map(|i| uf.find(i));
            let Some(main) = main else { break };
            let stray: Vec<usize> = members
                .iter()
                .copied()
                .filter(|&i| uf.find(i) != main)
                .collect();
            if stray.is_empty() {
                break;
            }
            let mut best: Option<(f64, usize, usize)> = None;
            for &s in &stray {
                for &m in members {
                    if uf.find(m) != main {
                        continue;
                    }
                    let d = dist(points[s], points[m]);
                    if best.is_none_or(|(bd, ..)| d < bd) {
                        best = Some((d, s, m));
                    }
                }
            }
            let (_, s, m) = best.expect("a main component member");
            edges.insert(Edge::new(s, m));
            uf.union(s, m);
        }
    }

    for e in transformers {
        out.corridors.push(Corridor {
            a: e.0,
            b: e.1,
            kv: buses[e.0].base_kv.max(buses[e.1].base_kv),
            length_km: SITE_TRANSFORMER_KM,
            role: CorridorRole::Internal,
            circuits: 1,
            route: vec![
                (buses[e.0].lon, buses[e.0].lat),
                (buses[e.1].lon, buses[e.1].lat),
            ],
            is_delaunay: false,
        });
    }
    for e in &edges {
        let c = corridor_between(
            buses,
            e.0,
            e.1,
            data,
            rng,
            MIN_LINE_KM,
            t.route_jitter,
            delaunay_edges.contains(e),
        );
        if c.role == CorridorRole::Interconnect {
            out.interconnects += 1;
        }
        out.corridors.push(c);
    }
    let _ = haversine_km;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::resolve_context;
    use crate::places::Places;
    use crate::request::{Region, SynthRequest};
    use crate::rng::StreamTag;

    fn run(n: u32) -> (Vec<DraftBus>, TopologyOutput) {
        let places = Places::parse(include_bytes!("../tests/fixtures/places_small.json")).unwrap();
        let mut req = SynthRequest::new(Region::bbox(42.1, 31.1, 48.5, 36.8, "Iraq box"));
        req.target_bus_count = Some(n);
        req.voltage_levels_kv = vec![400.0, 132.0, 66.0, 33.0];
        let (ctx, data) = resolve_context(&req, &places, &mut Vec::new()).unwrap();
        let mut rng = Stream::new(req.seed, StreamTag::Site);
        let mut buses = site_hierarchical(&ctx, &data, &mut rng);
        assert_eq!(buses.len(), n as usize);
        let mut rng = Stream::new(req.seed, StreamTag::Topology);
        let topo = hierarchical_topology(&mut buses, &data, &mut rng);
        (buses, topo)
    }

    #[test]
    fn levels_are_kept_apart_and_the_network_is_connected() {
        let (buses, topo) = run(600);
        let n = buses.len();
        assert!(n > 600, "taps are added");
        let mut uf = UnionFind::new(n);
        for c in &topo.corridors {
            uf.union(c.a, c.b);
            let (a, b) = (&buses[c.a], &buses[c.b]);
            if (a.base_kv - b.base_kv).abs() > 1e-9 {
                // Between levels: a site transformer, or a fallback join.
                assert!(a.site == b.site || c.length_km > SITE_TRANSFORMER_KM);
            }
        }
        assert_eq!(uf.components(n), 1);
        let taps = buses.iter().filter(|b| b.kind == BusKind::Tap).count();
        assert!(taps > 0);
        // Every level is present and the deepest holds the most buses.
        let count = |kv: f64| buses.iter().filter(|b| b.base_kv == kv).count();
        assert!(count(33.0) > count(66.0) && count(66.0) > count(400.0) && count(400.0) > 0);
        // Sparse: corridors per bus near a tree's.
        let lines = topo.corridors.len() as f64 / n as f64;
        assert!((1.0..1.4).contains(&lines), "{lines}");
        // Deterministic.
        let (again, topo2) = run(600);
        assert_eq!(buses, again);
        assert_eq!(topo.corridors, topo2.corridors);
    }

    #[test]
    fn mapped_subtransmission_is_regulated_within_reach() {
        // A 230/66 kV site feeding a 66 kV chain of five loaded buses 30 km
        // apart. Designed, only the step-down's secondary is regulated; mapped,
        // the bus 150 km out (beyond the 60 km reach) is too, which brings
        // every other bus within reach.
        let bus = |kv: f64, load_mw: f64| DraftBus {
            lon: 0.0,
            lat: 0.0,
            name: String::new(),
            place: None,
            tier: 0,
            base_kv: kv,
            kind: BusKind::Load,
            confidence: 1.0,
            polygon: 0,
            island: 0,
            source: ElementSource::Synthetic,
            population_weight: 0.0,
            load_mw,
            load_mvar: 0.0,
            annotation: String::new(),
            site: 0,
        };
        let mut buses = vec![bus(230.0, 0.0), bus(66.0, 5.0)];
        buses.extend((0..5).map(|_| bus(66.0, 5.0)));
        let corridor = |a: usize, b: usize, km: f64| Corridor {
            a,
            b,
            kv: 66.0,
            length_km: km,
            role: CorridorRole::Internal,
            circuits: 1,
            route: Vec::new(),
            is_delaunay: false,
        };
        let mut corridors = vec![corridor(0, 1, SITE_TRANSFORMER_KM)];
        corridors.extend((1..6).map(|i| corridor(i, i + 1, 30.0)));
        let designed = regulate_secondaries(&mut buses.clone(), &corridors, false);
        assert_eq!(designed.iter().map(|g| g.bus).collect::<Vec<_>>(), vec![1]);
        let mapped = regulate_secondaries(&mut buses, &corridors, true);
        let mut at: Vec<usize> = mapped.iter().map(|g| g.bus).collect();
        at.sort_unstable();
        assert_eq!(at, vec![1, 6]);
        assert!(mapped.iter().all(|g| g.condenser && g.qmax_mvar > 0.0));
    }

    #[test]
    fn fermat_point_of_an_equilateral_triangle_is_its_centre() {
        let s = fermat_point([(0.0, 0.0), (1.0, 0.0), (0.5, 3f64.sqrt() / 2.0)]);
        assert!((s.0 - 0.5).abs() < 1e-6 && (s.1 - 3f64.sqrt() / 6.0).abs() < 1e-6);
    }

    #[test]
    fn subtransmission_makes_a_ladder_hierarchical() {
        assert!(is_hierarchical(&[500.0, 230.0, 115.0, 66.0]));
        assert!(!is_hierarchical(&[500.0, 345.0, 230.0]));
        assert!(!is_hierarchical(&[66.0]));
    }
}
