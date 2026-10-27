//! Planar neighbourhood structure over projected bus points: the Delaunay
//! triangulation (O(n log n)), its edges, the Euclidean minimum spanning tree
//! (Kruskal over Delaunay edges, which contain the EMST) and the Gabriel
//! subgraph (each Delaunay edge tested against only its two opposite
//! vertices). Degenerate inputs (fewer than three points, all collinear) fall
//! back to a chain along the sorted hull so every point is reachable.

use delaunator::{next_halfedge, prev_halfedge, triangulate, Point};

/// One undirected edge between point indices `a < b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge(pub usize, pub usize);

impl Edge {
    pub fn new(a: usize, b: usize) -> Self {
        if a < b {
            Edge(a, b)
        } else {
            Edge(b, a)
        }
    }
}

/// A triangulation plus the per-edge opposite vertices the Gabriel test needs.
#[derive(Clone, Debug, Default)]
pub struct Triangulation {
    /// Unique undirected edges.
    pub edges: Vec<Edge>,
    /// For each edge, the third vertex of each adjacent triangle (one or two).
    pub opposite: Vec<[Option<usize>; 2]>,
}

/// Triangulate projected points. Coincident points are perturbed by the caller;
/// a point that still ends up in no triangle (exactly coincident) is attached to
/// its nearest neighbour so the edge set spans every index.
pub fn delaunay(points: &[(f64, f64)]) -> Triangulation {
    let n = points.len();
    let mut out = Triangulation::default();
    if n < 2 {
        return out;
    }
    if n == 2 {
        out.edges.push(Edge(0, 1));
        out.opposite.push([None, None]);
        return out;
    }
    let pts: Vec<Point> = points.iter().map(|&(x, y)| Point { x, y }).collect();
    let tri = triangulate(&pts);
    if tri.triangles.is_empty() {
        // Collinear (or nearly): the hull lists the points in order along the line.
        let mut order: Vec<usize> = if tri.hull.len() == n {
            tri.hull.clone()
        } else {
            let mut idx: Vec<usize> = (0..n).collect();
            idx.sort_by(|&a, &b| {
                points[a]
                    .partial_cmp(&points[b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            idx
        };
        order.dedup();
        for w in order.windows(2) {
            out.edges.push(Edge::new(w[0], w[1]));
            out.opposite.push([None, None]);
        }
        attach_isolated(points, &mut out);
        return out;
    }
    let mut seen = std::collections::HashMap::<Edge, usize>::new();
    for e in 0..tri.triangles.len() {
        let a = tri.triangles[e];
        let b = tri.triangles[next_halfedge(e)];
        let c = tri.triangles[prev_halfedge(e)];
        let edge = Edge::new(a, b);
        match seen.get(&edge) {
            Some(&k) => {
                if out.opposite[k][0] != Some(c) {
                    out.opposite[k][1] = Some(c);
                }
            }
            None => {
                seen.insert(edge, out.edges.len());
                out.edges.push(edge);
                out.opposite.push([Some(c), None]);
            }
        }
    }
    attach_isolated(points, &mut out);
    out
}

/// Connect any index absent from every edge to its nearest neighbour (brute
/// force; this only happens for exactly coincident points).
fn attach_isolated(points: &[(f64, f64)], tri: &mut Triangulation) {
    let n = points.len();
    let mut present = vec![false; n];
    for e in &tri.edges {
        present[e.0] = true;
        present[e.1] = true;
    }
    for i in 0..n {
        if present[i] || n < 2 {
            continue;
        }
        let mut best = None;
        let mut best_d = f64::INFINITY;
        for j in 0..n {
            if j == i {
                continue;
            }
            let d = (points[i].0 - points[j].0).powi(2) + (points[i].1 - points[j].1).powi(2);
            if d < best_d || (d == best_d && best.is_none_or(|b| j < b)) {
                best_d = d;
                best = Some(j);
            }
        }
        if let Some(j) = best {
            tri.edges.push(Edge::new(i, j));
            tri.opposite.push([None, None]);
            present[i] = true;
        }
    }
}

/// Union-find with path halving.
pub struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    pub fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// True when the two were in different sets.
    pub fn union(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        self.parent[ra] = rb;
        true
    }

    pub fn components(&mut self, n: usize) -> usize {
        (0..n).filter(|&i| self.find(i) == i).count()
    }
}

/// Kruskal's minimum spanning forest over `edges` with the given weights;
/// returns the chosen edge indices. Ties break on the edge order, which is
/// deterministic.
pub fn minimum_spanning_forest(n: usize, edges: &[Edge], weight: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by(|&a, &b| {
        weight[a]
            .partial_cmp(&weight[b])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(edges[a].cmp(&edges[b]))
    });
    let mut uf = UnionFind::new(n);
    let mut chosen = Vec::with_capacity(n.saturating_sub(1));
    for k in order {
        let e = edges[k];
        if uf.union(e.0, e.1) {
            chosen.push(k);
            if chosen.len() + 1 >= n {
                break;
            }
        }
    }
    chosen
}

/// Whether Delaunay edge `k` is a Gabriel edge: no opposite vertex lies inside
/// the circle with the edge as diameter (equivalently, the angle it subtends
/// at each opposite vertex is at most a right angle).
pub fn is_gabriel(points: &[(f64, f64)], tri: &Triangulation, k: usize) -> bool {
    let Edge(i, j) = tri.edges[k];
    let d2 = dist2(points[i], points[j]);
    tri.opposite[k]
        .iter()
        .flatten()
        .all(|&c| dist2(points[i], points[c]) + dist2(points[j], points[c]) >= d2 - 1e-9 * d2)
}

pub fn dist2(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

/// Adjacency lists from an edge set.
pub fn adjacency(n: usize, edges: &[Edge]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for e in edges {
        adj[e.0].push(e.1);
        adj[e.1].push(e.0);
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_points(w: usize, h: usize) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        for r in 0..h {
            for c in 0..w {
                // A slight shear avoids degenerate co-circular squares.
                pts.push((c as f64 + 0.05 * r as f64, r as f64 + 0.013 * c as f64));
            }
        }
        pts
    }

    #[test]
    fn edges_span_all_points_and_mst_has_n_minus_one_edges() {
        let pts = grid_points(7, 5);
        let tri = delaunay(&pts);
        let mut present = vec![false; pts.len()];
        for e in &tri.edges {
            present[e.0] = true;
            present[e.1] = true;
        }
        assert!(present.iter().all(|p| *p));
        let w: Vec<f64> = tri
            .edges
            .iter()
            .map(|e| dist2(pts[e.0], pts[e.1]).sqrt())
            .collect();
        let mst = minimum_spanning_forest(pts.len(), &tri.edges, &w);
        assert_eq!(mst.len(), pts.len() - 1);
        let gabriel = (0..tri.edges.len())
            .filter(|&k| is_gabriel(&pts, &tri, k))
            .count();
        assert!(gabriel >= mst.len());
        assert!(gabriel <= tri.edges.len());
    }

    #[test]
    fn collinear_and_tiny_inputs_are_chained() {
        let line: Vec<(f64, f64)> = (0..6).map(|i| (i as f64, 2.0 * i as f64)).collect();
        let tri = delaunay(&line);
        assert_eq!(tri.edges.len(), 5);
        assert_eq!(delaunay(&[(0.0, 0.0)]).edges.len(), 0);
        assert_eq!(delaunay(&[(0.0, 0.0), (1.0, 1.0)]).edges.len(), 1);
        let dup = vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)];
        let tri = delaunay(&dup);
        let mut present = [false; 4];
        for e in &tri.edges {
            present[e.0] = true;
            present[e.1] = true;
        }
        assert!(present.iter().all(|p| *p));
    }

    #[test]
    fn mst_of_a_square_with_centre_is_a_star() {
        let pts = vec![(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (1.0, 1.0)];
        let tri = delaunay(&pts);
        let w: Vec<f64> = tri
            .edges
            .iter()
            .map(|e| dist2(pts[e.0], pts[e.1]).sqrt())
            .collect();
        let mst = minimum_spanning_forest(5, &tri.edges, &w);
        assert_eq!(mst.len(), 4);
        assert!(mst
            .iter()
            .all(|&k| tri.edges[k].0 == 4 || tri.edges[k].1 == 4));
    }

    #[test]
    fn triangulates_three_thousand_points_quickly() {
        let mut pts = Vec::new();
        let mut s = 12345u64;
        for _ in 0..3000 {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let x = (s >> 11) as f64 / (1u64 << 53) as f64;
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let y = (s >> 11) as f64 / (1u64 << 53) as f64;
            pts.push((x * 1000.0, y * 1000.0));
        }
        let t0 = std::time::Instant::now();
        let tri = delaunay(&pts);
        let elapsed = t0.elapsed();
        assert!(tri.edges.len() > 8000);
        // Debug builds are slow; the release figure is well under 10 ms.
        assert!(elapsed.as_millis() < 2000, "{elapsed:?}");
    }
}
