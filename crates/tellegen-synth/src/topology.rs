//! Topology: per electrical island, a minimum spanning tree backbone over all
//! buses, a second spanning tree over the highest voltage buses, a Gabriel
//! proximity mesh added shortest first up to the density target, and a few
//! high-voltage loops for redundancy. Everything derives from one Delaunay
//! triangulation per island, so the whole stage is O(n log n). Landmasses in
//! one island are joined by the tree's shortest crossings, tagged
//! `interconnect`; no corridor ever joins two islands. Every corridor leaves
//! this stage with one circuit: how many it needs is decided by the corridor
//! plan once generation is sited ([`crate::electrical::planning`]).

use std::collections::{BTreeMap, BTreeSet};

use crate::context::ContextData;
use crate::geom::delaunay::{
    adjacency, delaunay, is_gabriel, minimum_spanning_forest, Edge, UnionFind,
};
use crate::geom::{haversine_km, Projection};
use crate::rng::Stream;
use crate::site::DraftBus;

/// Where a corridor sits relative to the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorridorRole {
    Internal,
    /// Leaves the region polygon somewhere along its route.
    TieLine,
    /// Joins two landmasses.
    Interconnect,
}

impl CorridorRole {
    pub fn as_str(self) -> &'static str {
        match self {
            CorridorRole::Internal => "internal",
            CorridorRole::TieLine => "tie_line",
            CorridorRole::Interconnect => "interconnect",
        }
    }
}

/// One corridor between two buses; `circuits` parallel branches share it.
#[derive(Clone, Debug, PartialEq)]
pub struct Corridor {
    pub a: usize,
    pub b: usize,
    /// The higher of the two endpoint voltages.
    pub kv: f64,
    pub length_km: f64,
    pub role: CorridorRole,
    pub circuits: u32,
    /// Three-point route in frame longitudes: ends plus a jittered midpoint.
    pub route: Vec<(f64, f64)>,
    pub is_delaunay: bool,
}

/// Maximum nodal degree the mesh and loops may create (the trees are exempt).
/// source: Birchfield et al. 2017 Fig. 3, degree distributions tail off by 7–8.
pub const MAX_DEGREE: usize = 8;

/// Distinct corridors per bus are capped here; the rest of the branch target is
/// met with parallel circuits, as real high-voltage corridors are.
pub const MAX_UNIQUE_RATIO: f64 = 1.55;

pub struct TopologyOutput {
    pub corridors: Vec<Corridor>,
    pub interconnects: usize,
}

pub fn build_topology(
    buses: &[DraftBus],
    data: &ContextData,
    branch_factor: f64,
    rng: &mut Stream,
) -> TopologyOutput {
    let n = buses.len();
    let mut out = TopologyOutput {
        corridors: Vec::new(),
        interconnects: 0,
    };
    if n < 2 {
        return out;
    }
    let proj = Projection::for_frame(&data.shape.frame);
    let points: Vec<(f64, f64)> = buses.iter().map(|b| proj.project(b.lon, b.lat)).collect();
    let unique_ratio = branch_factor.min(MAX_UNIQUE_RATIO);
    let mut edges: BTreeSet<Edge> = BTreeSet::new();
    let mut delaunay_edges: BTreeSet<Edge> = BTreeSet::new();

    let mut islands: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, b) in buses.iter().enumerate() {
        islands.entry(b.island).or_default().push(i);
    }
    for idxs in islands.values() {
        island_topology(
            buses,
            &points,
            idxs,
            unique_ratio,
            &mut edges,
            &mut delaunay_edges,
        );
    }

    for e in &edges {
        let (a, b) = (&buses[e.0], &buses[e.1]);
        let route = line_route(a, b, &data.shape.frame, rng);
        let role = if a.polygon != b.polygon {
            CorridorRole::Interconnect
        } else if data.shape.polyline_inside(&route) {
            CorridorRole::Internal
        } else {
            CorridorRole::TieLine
        };
        if role == CorridorRole::Interconnect {
            out.interconnects += 1;
        }
        out.corridors.push(Corridor {
            a: e.0,
            b: e.1,
            kv: a.base_kv.max(b.base_kv),
            length_km: haversine_km(a.lon, a.lat, b.lon, b.lat).max(3.0),
            role,
            circuits: 1,
            route,
            is_delaunay: delaunay_edges.contains(e),
        });
    }
    out
}

/// Wire one island's buses (indices into `buses`).
fn island_topology(
    buses: &[DraftBus],
    points: &[(f64, f64)],
    idxs: &[usize],
    unique_ratio: f64,
    edges: &mut BTreeSet<Edge>,
    delaunay_edges: &mut BTreeSet<Edge>,
) {
    if idxs.len() < 2 {
        return;
    }
    let local_points: Vec<(f64, f64)> = idxs.iter().map(|&i| points[i]).collect();
    let tri = delaunay(&local_points);
    let weight: Vec<f64> = tri
        .edges
        .iter()
        .map(|e| {
            let (a, b) = (&buses[idxs[e.0]], &buses[idxs[e.1]]);
            haversine_km(a.lon, a.lat, b.lon, b.lat)
        })
        .collect();
    for e in &tri.edges {
        delaunay_edges.insert(Edge::new(idxs[e.0], idxs[e.1]));
    }
    let mut local: BTreeSet<Edge> = BTreeSet::new();
    let mut degree = vec![0usize; idxs.len()];
    let add = |local: &mut BTreeSet<Edge>, degree: &mut Vec<usize>, e: Edge| -> bool {
        if e.0 == e.1 || !local.insert(e) {
            return false;
        }
        degree[e.0] += 1;
        degree[e.1] += 1;
        true
    };

    // 1. Spanning tree backbone.
    for k in minimum_spanning_forest(idxs.len(), &tri.edges, &weight) {
        add(&mut local, &mut degree, tri.edges[k]);
    }
    let target = (unique_ratio * idxs.len() as f64).round() as usize;

    // 2. Spanning tree over the highest voltage buses.
    let max_kv = idxs
        .iter()
        .map(|&i| buses[i].base_kv)
        .fold(f64::NEG_INFINITY, f64::max);
    let hv: Vec<usize> = (0..idxs.len())
        .filter(|&k| buses[idxs[k]].base_kv >= max_kv - 1e-6)
        .collect();
    let hv_tri = if hv.len() >= 2 {
        let hv_points: Vec<(f64, f64)> = hv.iter().map(|&k| local_points[k]).collect();
        let t = delaunay(&hv_points);
        let w: Vec<f64> = t
            .edges
            .iter()
            .map(|e| {
                let (a, b) = (&buses[idxs[hv[e.0]]], &buses[idxs[hv[e.1]]]);
                haversine_km(a.lon, a.lat, b.lon, b.lat)
            })
            .collect();
        for k in minimum_spanning_forest(hv.len(), &t.edges, &w) {
            if local.len() >= target {
                break;
            }
            let e = t.edges[k];
            add(&mut local, &mut degree, Edge::new(hv[e.0], hv[e.1]));
        }
        Some((t, w))
    } else {
        None
    };

    // 3. Gabriel mesh, shortest first, under the degree cap.
    let mut gabriel: Vec<usize> = (0..tri.edges.len())
        .filter(|&k| is_gabriel(&local_points, &tri, k))
        .collect();
    gabriel.sort_by(|&a, &b| {
        weight[a]
            .partial_cmp(&weight[b])
            .unwrap()
            .then(tri.edges[a].cmp(&tri.edges[b]))
    });
    for k in gabriel {
        if local.len() >= target {
            break;
        }
        let e = tri.edges[k];
        if degree[e.0] >= MAX_DEGREE || degree[e.1] >= MAX_DEGREE {
            continue;
        }
        add(&mut local, &mut degree, e);
    }

    // 4. High-voltage loops: each top-tier bus to a near top-tier neighbour it
    //    is not yet tied to, a few per island.
    if let Some((t, w)) = hv_tri {
        if hv.len() >= 3 {
            let cap = ((0.05 * idxs.len() as f64).round() as usize).clamp(2, 6);
            let adj = adjacency(hv.len(), &t.edges);
            let mut added = 0;
            for (h, nbrs) in adj.iter().enumerate() {
                if added >= cap || local.len() >= target {
                    break;
                }
                let mut sorted: Vec<usize> = nbrs.clone();
                sorted.sort_by(|&a, &b| {
                    let wa = w[t.edges.iter().position(|e| *e == Edge::new(h, a)).unwrap()];
                    let wb = w[t.edges.iter().position(|e| *e == Edge::new(h, b)).unwrap()];
                    wa.partial_cmp(&wb).unwrap().then(a.cmp(&b))
                });
                for &j in sorted.iter().take(3) {
                    let e = Edge::new(hv[h], hv[j]);
                    if local.contains(&e) || degree[e.0] >= MAX_DEGREE || degree[e.1] >= MAX_DEGREE
                    {
                        continue;
                    }
                    add(&mut local, &mut degree, e);
                    added += 1;
                    break;
                }
            }
        }
    }

    // Guarantee: one component per island (the tree already does this; the
    // union-find check is cheap insurance against a degenerate triangulation).
    let mut uf = UnionFind::new(idxs.len());
    for e in &local {
        uf.union(e.0, e.1);
    }
    if uf.components(idxs.len()) > 1 {
        // Attach each extra component to its nearest bus in the first one.
        let root0 = uf.find(0);
        for k in 1..idxs.len() {
            if uf.find(k) != root0 {
                let mut best = (f64::INFINITY, 0usize);
                for m in 0..idxs.len() {
                    if uf.find(m) == root0 {
                        let d = crate::geom::delaunay::dist2(local_points[k], local_points[m]);
                        if d < best.0 {
                            best = (d, m);
                        }
                    }
                }
                add(&mut local, &mut degree, Edge::new(k, best.1));
                uf.union(k, best.1);
            }
        }
    }

    for e in local {
        edges.insert(Edge::new(idxs[e.0], idxs[e.1]));
    }
}

/// A three-point route: the two ends and a midpoint jittered by up to ±0.03°
/// so parallel corridors read as separate lines on the map.
fn line_route(
    a: &DraftBus,
    b: &DraftBus,
    frame: &crate::report::BBox,
    rng: &mut Stream,
) -> Vec<(f64, f64)> {
    let mid_lon = (a.lon + b.lon) / 2.0 + 0.06 * (rng.uniform() - 0.5);
    let mid_lat = (a.lat + b.lat) / 2.0 + 0.06 * (rng.uniform() - 0.5);
    let margin_lon = 0.01 * (frame.east - frame.west);
    let margin_lat = 0.01 * (frame.north - frame.south);
    let mid_lon = mid_lon.clamp(frame.west + margin_lon, frame.east - margin_lon);
    let mid_lat = mid_lat.clamp(frame.south + margin_lat, frame.north - margin_lat);
    vec![(a.lon, a.lat), (mid_lon, mid_lat), (b.lon, b.lat)]
}

/// Nodal degree counting parallel circuits once per corridor.
pub fn corridor_degrees(n: usize, corridors: &[Corridor]) -> Vec<usize> {
    let mut deg = vec![0usize; n];
    for c in corridors {
        deg[c.a] += 1;
        deg[c.b] += 1;
    }
    deg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::resolve_context;
    use crate::places::Places;
    use crate::request::{Region, SynthRequest};
    use crate::rng::StreamTag;
    use crate::site::site_buses;

    fn generate(req: &SynthRequest) -> (Vec<DraftBus>, TopologyOutput, ContextData) {
        let places = Places::parse(include_bytes!("../tests/fixtures/places_small.json")).unwrap();
        let (ctx, data) = resolve_context(req, &places, &mut Vec::new()).unwrap();
        let mut rng = Stream::new(req.seed, StreamTag::Site);
        let buses = site_buses(&ctx, &data, &mut rng);
        let mut rng = Stream::new(req.seed, StreamTag::Topology);
        let topo = build_topology(&buses, &data, ctx.branch_factor, &mut rng);
        (buses, topo, data)
    }

    #[test]
    fn one_component_realistic_degree_and_branch_count() {
        for &n in &[30usize, 100, 300, 1000] {
            let mut req = SynthRequest::new(Region::bbox(42.1, 31.1, 48.5, 36.8, "Iraq box"));
            req.target_bus_count = Some(n as u32);
            req.voltage_levels_kv = vec![500.0, 230.0, 115.0];
            let (buses, topo, _) = generate(&req);
            let mut uf = UnionFind::new(n);
            for c in &topo.corridors {
                uf.union(c.a, c.b);
            }
            assert_eq!(uf.components(n), 1, "n={n}");
            let deg = corridor_degrees(n, &topo.corridors);
            let avg = 2.0 * topo.corridors.len() as f64 / n as f64;
            assert!((2.2..=3.1).contains(&avg), "n={n} avg degree {avg}");
            assert!(*deg.iter().max().unwrap() <= MAX_DEGREE, "n={n}");
            assert_eq!(
                topo.corridors.len(),
                (1.45 * n as f64).round() as usize,
                "n={n}"
            );
            assert!(topo.corridors.iter().all(|c| c.circuits == 1));
            assert!(topo
                .corridors
                .iter()
                .all(|c| c.role != CorridorRole::Interconnect));
            assert!(buses.len() == n);
        }
    }

    #[test]
    fn distinct_corridors_follow_the_branch_factor_up_to_the_cap() {
        for &(n, f) in &[(20usize, 1.0), (24, 2.0), (60, 1.3)] {
            let mut req = SynthRequest::new(Region::bbox(-2.0, 5.0, 1.0, 8.0, "box"));
            req.target_bus_count = Some(n as u32);
            req.branch_factor = Some(f);
            let (_, topo, _) = generate(&req);
            let unique = (f.min(MAX_UNIQUE_RATIO) * n as f64).round() as usize;
            assert_eq!(topo.corridors.len(), unique, "n={n} f={f}");
            assert!(topo.corridors.iter().all(|c| c.circuits == 1));
        }
    }

    #[test]
    fn landmasses_within_reach_are_joined_and_far_ones_are_not() {
        let geom = serde_json::json!({"type":"MultiPolygon","coordinates":[
            [[[0.0,0.0],[2.0,0.0],[2.0,2.0],[0.0,2.0],[0.0,0.0]]],
            [[[2.5,0.0],[4.0,0.0],[4.0,1.5],[2.5,1.5],[2.5,0.0]]]]});
        let mut req = SynthRequest::new(Region {
            geometry: geom,
            label: "two".into(),
        });
        req.target_bus_count = Some(40);
        let (buses, topo, _) = generate(&req);
        assert!(topo.interconnects >= 1);
        assert!(topo
            .corridors
            .iter()
            .all(|c| buses[c.a].island == buses[c.b].island));

        let far = serde_json::json!({"type":"MultiPolygon","coordinates":[
            [[[0.0,0.0],[2.0,0.0],[2.0,2.0],[0.0,2.0],[0.0,0.0]]],
            [[[9.0,0.0],[11.0,0.0],[11.0,2.0],[9.0,2.0],[9.0,0.0]]]]});
        let mut req = SynthRequest::new(Region {
            geometry: far,
            label: "far".into(),
        });
        req.target_bus_count = Some(40);
        req.islands.beyond = crate::request::FarIslandPolicy::Keep;
        let (buses, topo, _) = generate(&req);
        assert_eq!(topo.interconnects, 0);
        assert!(buses.iter().any(|b| b.island == 1));
        let mut uf = UnionFind::new(40);
        for c in &topo.corridors {
            uf.union(c.a, c.b);
        }
        assert_eq!(uf.components(40), 2);
    }
}
