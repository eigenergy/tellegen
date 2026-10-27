//! Structural statistics of a balanced network: degree distribution,
//! clustering, components, algebraic connectivity by sparse power iteration,
//! corridor lengths against the Euclidean minimum spanning tree, voltage
//! shares, parallel corridors and capacity margin. The same code measures
//! generated grids and staged reference cases, so comparisons are like for
//! like.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use powerio::BalancedNetwork;

use crate::geom::delaunay::{delaunay, minimum_spanning_forest, Edge, UnionFind};
use crate::geom::haversine_km;
use crate::report::{BBox, Quantiles, ReferenceComparison, TopologyStats};
use crate::request::CalibrationTargets;

/// Graphs above this size skip the eigenvalue estimate.
pub const LAMBDA2_MAX_BUSES: usize = 2000;
const LAMBDA2_MAX_ITERATIONS: usize = 600;

/// Measure `net`. `max_loading` is the DC OPF figure when one exists.
pub fn measure(net: &BalancedNetwork, max_loading: Option<f64>) -> TopologyStats {
    let buses = net.buses();
    let n = buses.len();
    let index: HashMap<usize, usize> = buses.iter().enumerate().map(|(i, b)| (b.id.0, i)).collect();
    let mut corridor_count: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut corridor_length: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let mut lengths = Vec::new();
    let mut n_transformers = 0;
    let mut uf = UnionFind::new(n);
    let coords: Vec<Option<(f64, f64)>> = buses
        .iter()
        .map(|b| b.location.map(|l| (l.x, l.y)))
        .collect();
    for br in net.branches().iter().filter(|b| b.in_service) {
        let (Some(&a), Some(&b)) = (index.get(&br.from.0), index.get(&br.to.0)) else {
            continue;
        };
        if a == b {
            continue;
        }
        let kv_differs = (buses[a].base_kv - buses[b].base_kv).abs() > 1e-6;
        if br.tap != 0.0 || kv_differs {
            n_transformers += 1;
        }
        let key = (a.min(b), a.max(b));
        *corridor_count.entry(key).or_insert(0) += 1;
        uf.union(a, b);
        let len = branch_length_km(br, coords[a], coords[b]);
        if let Some(len) = len {
            lengths.push(len);
            corridor_length.entry(key).or_insert(len);
        }
    }
    let mut degree = vec![0usize; n];
    let mut adj: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    for &(a, b) in corridor_count.keys() {
        degree[a] += 1;
        degree[b] += 1;
        adj[a].insert(b);
        adj[b].insert(a);
    }
    let max_degree = degree.iter().copied().max().unwrap_or(0);
    let mut degree_histogram = vec![0u32; max_degree + 1];
    for &d in &degree {
        degree_histogram[d] += 1;
    }
    let clustering = {
        let mut sum = 0.0;
        let mut count = 0usize;
        for i in 0..n {
            let d = adj[i].len();
            if d < 2 {
                continue;
            }
            let mut tri = 0usize;
            let nb: Vec<usize> = adj[i].iter().copied().collect();
            for x in 0..nb.len() {
                for y in x + 1..nb.len() {
                    if adj[nb[x]].contains(&nb[y]) {
                        tri += 1;
                    }
                }
            }
            sum += tri as f64 / (d * (d - 1) / 2) as f64;
            count += 1;
        }
        if count > 0 {
            sum / count as f64
        } else {
            0.0
        }
    };
    let connected_components = if n == 0 { 0 } else { uf.components(n) };
    let m_branches = net.branches().iter().filter(|b| b.in_service).count();

    // Spanning tree ratio and Delaunay fraction over located buses.
    let located: Vec<usize> = (0..n).filter(|&i| coords[i].is_some()).collect();
    let (mst_length_ratio, delaunay_edge_fraction) = if located.len() >= 2 {
        let lons: Vec<f64> = located.iter().map(|&i| coords[i].unwrap().0).collect();
        let lats: Vec<f64> = located.iter().map(|&i| coords[i].unwrap().1).collect();
        let frame = BBox {
            west: lons.iter().cloned().fold(f64::INFINITY, f64::min),
            east: lons.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            south: lats.iter().cloned().fold(f64::INFINITY, f64::min),
            north: lats.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        };
        let proj = crate::geom::Projection::for_frame(&frame);
        let pts: Vec<(f64, f64)> = located
            .iter()
            .map(|&i| {
                let (lon, lat) = coords[i].unwrap();
                proj.project(lon, lat)
            })
            .collect();
        let tri = delaunay(&pts);
        let w: Vec<f64> = tri
            .edges
            .iter()
            .map(|e| {
                let (a, b) = (coords[located[e.0]].unwrap(), coords[located[e.1]].unwrap());
                haversine_km(a.0, a.1, b.0, b.1)
            })
            .collect();
        let mst = minimum_spanning_forest(located.len(), &tri.edges, &w);
        let mst_len: f64 = mst.iter().map(|&k| w[k]).sum();
        let total_corridor: f64 = corridor_length.values().sum();
        let ratio = if mst_len > 0.0 {
            total_corridor / mst_len
        } else {
            0.0
        };
        let dset: BTreeSet<Edge> = tri
            .edges
            .iter()
            .map(|e| Edge::new(located[e.0], located[e.1]))
            .collect();
        let hits = corridor_count
            .keys()
            .filter(|&&(a, b)| dset.contains(&Edge::new(a, b)))
            .count();
        let frac = if corridor_count.is_empty() {
            0.0
        } else {
            hits as f64 / corridor_count.len() as f64
        };
        (ratio, frac)
    } else {
        (0.0, 0.0)
    };

    let (lambda2, converged) = if (2..=LAMBDA2_MAX_BUSES).contains(&n) {
        let (l, c) = algebraic_connectivity(n, &adj);
        (Some(l), Some(c))
    } else {
        (None, None)
    };

    let mut voltage_counts: BTreeMap<String, u32> = BTreeMap::new();
    let max_kv = buses
        .iter()
        .map(|b| b.base_kv)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut hv = 0usize;
    for b in buses {
        *voltage_counts.entry(kv_key(b.base_kv)).or_insert(0) += 1;
        if (b.base_kv - max_kv).abs() < 1e-6 {
            hv += 1;
        }
    }
    let total_load: f64 = net
        .loads()
        .iter()
        .filter(|l| l.in_service)
        .map(|l| l.p)
        .sum();
    let total_cap: f64 = net
        .generators()
        .iter()
        .filter(|g| g.in_service)
        .map(|g| g.pmax)
        .sum();
    TopologyStats {
        n_buses: n,
        n_branches: m_branches,
        n_generators: net.generators().iter().filter(|g| g.in_service).count(),
        n_loads: net.loads().iter().filter(|l| l.in_service).count(),
        n_transformers,
        n_hvdc: net.hvdc().len(),
        connected_components,
        algebraic_connectivity: lambda2.map(|x| (x * 1e6).round() / 1e6),
        algebraic_connectivity_converged: converged,
        degree_histogram,
        avg_degree: if n > 0 {
            2.0 * corridor_count.len() as f64 / n as f64
        } else {
            0.0
        },
        max_degree,
        clustering_coefficient: clustering,
        branch_bus_ratio: if n > 0 {
            m_branches as f64 / n as f64
        } else {
            0.0
        },
        line_length_km: quantiles(&lengths),
        total_line_length_km: lengths.iter().sum(),
        mst_length_ratio,
        delaunay_edge_fraction,
        parallel_corridors: corridor_count.values().filter(|&&c| c > 1).count(),
        parallel_branches: corridor_count.values().map(|&c| c.saturating_sub(1)).sum(),
        voltage_counts,
        hv_share: if n > 0 { hv as f64 / n as f64 } else { 0.0 },
        total_load_mw: total_load,
        total_generation_capacity_mw: total_cap,
        gen_load_margin: if total_load > 0.0 {
            total_cap / total_load
        } else {
            0.0
        },
        max_line_loading: max_loading,
        reference: Vec::new(),
    }
}

/// Compare the structural statistics with the reference ranges.
pub fn compare(stats: &mut TopologyStats, targets: &CalibrationTargets) {
    let within = |v: f64, r: [f64; 2]| v >= r[0] && v <= r[1];
    stats.reference = vec![
        ReferenceComparison {
            metric: "avg_degree".into(),
            value: stats.avg_degree,
            reference_range: targets.avg_degree,
            within: within(stats.avg_degree, targets.avg_degree),
        },
        ReferenceComparison {
            metric: "hv_share".into(),
            value: stats.hv_share,
            reference_range: targets.hv_share,
            within: within(stats.hv_share, targets.hv_share),
        },
    ];
}

fn kv_key(kv: f64) -> String {
    if (kv - kv.round()).abs() < 1e-9 {
        format!("{}", kv.round() as i64)
    } else {
        format!("{}", (kv * 1000.0).round() / 1000.0)
    }
}

fn branch_length_km(
    br: &powerio::Branch,
    a: Option<(f64, f64)>,
    b: Option<(f64, f64)>,
) -> Option<f64> {
    if let Some(len) = br
        .extras
        .get(crate::build::EXTRAS_KEY)
        .and_then(|v| v.get("length_km"))
        .and_then(|v| v.as_f64())
    {
        return Some(len);
    }
    if let Some(route) = br.route.as_ref().filter(|r| r.len() >= 2) {
        return Some(
            route
                .windows(2)
                .map(|w| haversine_km(w[0].x, w[0].y, w[1].x, w[1].y))
                .sum(),
        );
    }
    match (a, b) {
        (Some(a), Some(b)) => Some(haversine_km(a.0, a.1, b.0, b.1)),
        _ => None,
    }
}

fn quantiles(values: &[f64]) -> Quantiles {
    if values.is_empty() {
        return Quantiles::default();
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| {
        let pos = p * (v.len() - 1) as f64;
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        v[lo] + (v[hi] - v[lo]) * (pos - lo as f64)
    };
    Quantiles {
        min: v[0],
        p10: q(0.1),
        p50: q(0.5),
        mean: v.iter().sum::<f64>() / v.len() as f64,
        p90: q(0.9),
        max: v[v.len() - 1],
    }
}

/// Second smallest Laplacian eigenvalue by power iteration on
/// `(d_max + 1) I − L` restricted to the complement of the constant vector.
/// Returns the Rayleigh quotient and whether the iteration converged.
pub fn algebraic_connectivity(n: usize, adj: &[BTreeSet<usize>]) -> (f64, bool) {
    let dmax = adj.iter().map(|a| a.len()).max().unwrap_or(0) as f64;
    let shift = dmax + 1.0;
    let mut v: Vec<f64> = (0..n).map(|i| ((i + 1) as f64 * 0.7391).sin()).collect();
    orthonormalise(&mut v);
    let apply = |v: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let lv = adj[i].len() as f64 * v[i] - adj[i].iter().map(|&j| v[j]).sum::<f64>();
                shift * v[i] - lv
            })
            .collect()
    };
    let mut converged = false;
    for _ in 0..LAMBDA2_MAX_ITERATIONS {
        let mut w = apply(&v);
        orthonormalise(&mut w);
        let diff: f64 = v
            .iter()
            .zip(&w)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        v = w;
        if diff < 1e-7 {
            converged = true;
            break;
        }
    }
    let lv: f64 = (0..n)
        .map(|i| v[i] * (adj[i].len() as f64 * v[i] - adj[i].iter().map(|&j| v[j]).sum::<f64>()))
        .sum();
    (lv.max(0.0), converged)
}

fn orthonormalise(v: &mut [f64]) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    for x in v.iter_mut() {
        *x -= mean;
    }
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use powerio::{Branch, Bus, BusId, BusType};

    fn graph(n: usize, edges: &[(usize, usize)]) -> BalancedNetwork {
        let coords: Vec<(f64, f64)> = (0..n)
            .map(|i| (i as f64, (i * i % 7) as f64 * 0.1))
            .collect();
        graph_at(&coords, edges)
    }

    fn graph_at(coords: &[(f64, f64)], edges: &[(usize, usize)]) -> BalancedNetwork {
        let buses = coords
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                let mut b = Bus::new(
                    BusId(i + 1),
                    if i == 0 { BusType::Ref } else { BusType::Pq },
                    230.0,
                );
                b.location = Some(powerio::geo::Location { x, y, kind: None });
                b
            })
            .collect();
        let branches = edges
            .iter()
            .map(|&(a, b)| Branch::new(BusId(a + 1), BusId(b + 1), 0.01, 0.1))
            .collect();
        BalancedNetwork::in_memory("g", 100.0, buses, branches)
    }

    #[test]
    fn triangle_and_star_clustering() {
        let tri = measure(&graph(3, &[(0, 1), (1, 2), (0, 2)]), None);
        assert!((tri.clustering_coefficient - 1.0).abs() < 1e-12);
        assert_eq!(tri.connected_components, 1);
        assert_eq!(tri.degree_histogram, vec![0, 0, 3]);
        let star = measure(
            &graph_at(
                &[(1.0, 1.0), (0.0, 0.0), (2.0, 0.0), (1.0, 2.0)],
                &[(0, 1), (0, 2), (0, 3)],
            ),
            None,
        );
        assert_eq!(star.clustering_coefficient, 0.0);
        assert_eq!(star.max_degree, 3);
        assert!((star.mst_length_ratio - 1.0).abs() < 1e-9);
        assert!((star.delaunay_edge_fraction - 1.0).abs() < 1e-12);
    }

    #[test]
    fn lambda2_and_components() {
        let path = measure(&graph(5, &[(0, 1), (1, 2), (2, 3), (3, 4)]), None);
        // Path on 5 nodes: λ2 = 2 − 2 cos(π/5) ≈ 0.382.
        assert!((path.algebraic_connectivity.unwrap() - 0.382).abs() < 0.01);
        assert_eq!(path.algebraic_connectivity_converged, Some(true));
        let two = measure(&graph(4, &[(0, 1), (2, 3)]), None);
        assert_eq!(two.connected_components, 2);
        assert!(two.algebraic_connectivity.unwrap() < 1e-6);
        let parallel = measure(&graph(2, &[(0, 1), (0, 1)]), None);
        assert_eq!(parallel.parallel_corridors, 1);
        assert_eq!(parallel.parallel_branches, 1);
        assert_eq!(parallel.n_branches, 2);
    }
}
