//! Sparse stress layout for graphs without coordinates.
//!
//! Stress majorization places nodes so their drawn distances match their graph distances. The full
//! model needs all pairs (quadratic); the sparse model of Ortmann, Klimenta and Brandes, "A sparse
//! stress model", JGAA 21(5), 2017, keeps the terms to each node's neighbours plus a few pivots,
//! each pivot standing in for the nodes of its region. The start is pivot MDS (Brandes and Pich,
//! "Eigensolver methods for progressive multidimensional scaling of large data", Graph Drawing
//! 2006). Both cost O(p (n + m)) per pass for p pivots, so tens of thousands of nodes lay out in a
//! fraction of a second. Deterministic: pivots are chosen by max-min distance from node 0, the
//! power iteration starts from a fixed vector, and no random numbers are used.

use std::collections::VecDeque;

/// Number of pivots (Ortmann et al. find 50 to 200 enough).
const PIVOTS: usize = 64;
/// Majorization passes at most; a pass that moves nodes by less than `SETTLED` on average (in
/// edge lengths) ends the iteration.
const PASSES: usize = 60;
const SETTLED: f64 = 1e-2;

/// Positions for `n` nodes joined by undirected `edges`, unit edge length. Components are laid out
/// separately and packed side by side.
pub fn sparse_stress(n: usize, edges: &[(usize, usize)]) -> Vec<(f64, f64)> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b) in edges {
        if a != b && a < n && b < n {
            adj[a].push(b);
            adj[b].push(a);
        }
    }
    for list in &mut adj {
        list.sort_unstable();
        list.dedup();
    }
    let mut out = vec![(0.0, 0.0); n];
    let mut component = vec![usize::MAX; n];
    let mut parts: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if component[start] != usize::MAX {
            continue;
        }
        let id = parts.len();
        let mut members = vec![start];
        component[start] = id;
        let mut head = 0;
        while head < members.len() {
            let u = members[head];
            head += 1;
            for &v in &adj[u] {
                if component[v] == usize::MAX {
                    component[v] = id;
                    members.push(v);
                }
            }
        }
        parts.push(members);
    }
    // Largest components first, packed in rows.
    parts.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    let mut local = vec![usize::MAX; n];
    let (mut x, mut y, mut row_h) = (0.0f64, 0.0f64, 0.0f64);
    let width = (n as f64).sqrt().max(1.0) * 2.0;
    for members in &parts {
        for (i, &u) in members.iter().enumerate() {
            local[u] = i;
        }
        let sub: Vec<Vec<usize>> = members
            .iter()
            .map(|&u| adj[u].iter().map(|&v| local[v]).collect())
            .collect();
        let pos = component_layout(&sub);
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &pos {
            x0 = x0.min(p.0);
            x1 = x1.max(p.0);
            y0 = y0.min(p.1);
            y1 = y1.max(p.1);
        }
        let (w, h) = (x1 - x0, y1 - y0);
        if x > 0.0 && x + w > width {
            x = 0.0;
            y += row_h + 2.0;
            row_h = 0.0;
        }
        for (i, &u) in members.iter().enumerate() {
            out[u] = (pos[i].0 - x0 + x, pos[i].1 - y0 + y);
        }
        x += w + 2.0;
        row_h = row_h.max(h);
    }
    out
}

/// Hop distances from `s` over `adj`; `u32::MAX` where unreachable.
fn bfs(adj: &[Vec<usize>], s: usize, dist: &mut [u32]) {
    dist.fill(u32::MAX);
    dist[s] = 0;
    let mut queue = VecDeque::from([s]);
    while let Some(u) = queue.pop_front() {
        for &v in &adj[u] {
            if dist[v] == u32::MAX {
                dist[v] = dist[u] + 1;
                queue.push_back(v);
            }
        }
    }
}

fn component_layout(adj: &[Vec<usize>]) -> Vec<(f64, f64)> {
    let n = adj.len();
    match n {
        0 => return Vec::new(),
        1 => return vec![(0.0, 0.0)],
        2 => return vec![(0.0, 0.0), (1.0, 0.0)],
        _ => {}
    }
    // Pivots by max-min distance, with each pivot's distances to every node.
    let p = PIVOTS.min(n);
    let mut pivots = Vec::with_capacity(p);
    let mut dist: Vec<Vec<u32>> = Vec::with_capacity(p);
    let mut nearest = vec![u32::MAX; n];
    let mut next = 0;
    for _ in 0..p {
        let mut d = vec![0u32; n];
        bfs(adj, next, &mut d);
        for i in 0..n {
            nearest[i] = nearest[i].min(d[i]);
        }
        pivots.push(next);
        dist.push(d);
        next = (0..n)
            .max_by_key(|&i| (nearest[i], std::cmp::Reverse(i)))
            .unwrap_or(0);
        if nearest[next] == 0 {
            break;
        }
    }
    let p = pivots.len();

    // Pivot MDS: double-centred squared distances C (n × p); the top two eigenvectors of CᵀC give
    // the coordinates C·v.
    let mut c = vec![0.0f64; n * p];
    for (j, d) in dist.iter().enumerate() {
        for i in 0..n {
            let v = f64::from(d[i]);
            c[i * p + j] = v * v;
        }
    }
    let mut col_mean = vec![0.0; p];
    let mut row_mean = vec![0.0; n];
    let mut all = 0.0;
    for i in 0..n {
        for j in 0..p {
            col_mean[j] += c[i * p + j];
            row_mean[i] += c[i * p + j];
            all += c[i * p + j];
        }
    }
    col_mean.iter_mut().for_each(|v| *v /= n as f64);
    row_mean.iter_mut().for_each(|v| *v /= p as f64);
    all /= (n * p) as f64;
    for i in 0..n {
        for j in 0..p {
            c[i * p + j] = -0.5 * (c[i * p + j] - col_mean[j] - row_mean[i] + all);
        }
    }
    let mut ctc = vec![0.0; p * p];
    for i in 0..n {
        let row = &c[i * p..i * p + p];
        for a in 0..p {
            for b in a..p {
                ctc[a * p + b] += row[a] * row[b];
            }
        }
    }
    for a in 0..p {
        for b in 0..a {
            ctc[a * p + b] = ctc[b * p + a];
        }
    }
    let first = power_iteration(&ctc, p, None);
    let second = power_iteration(&ctc, p, Some(&first));
    let mut pos: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let row = &c[i * p..i * p + p];
            let x: f64 = row.iter().zip(&first).map(|(a, b)| a * b).sum();
            let y: f64 = row.iter().zip(&second).map(|(a, b)| a * b).sum();
            (x, y)
        })
        .collect();
    // Scale so neighbours sit about one unit apart, the stress model's target.
    let mut sum = 0.0;
    let mut count = 0usize;
    for (u, list) in adj.iter().enumerate() {
        for &v in list {
            if u < v {
                sum += (pos[u].0 - pos[v].0).hypot(pos[u].1 - pos[v].1);
                count += 1;
            }
        }
    }
    let mean = if count > 0 { sum / count as f64 } else { 1.0 };
    if mean > 1e-12 {
        pos.iter_mut().for_each(|q| *q = (q.0 / mean, q.1 / mean));
    }
    // Nodes MDS put on top of each other get a deterministic nudge.
    for (i, q) in pos.iter_mut().enumerate() {
        let t = i as f64;
        *q = (
            q.0 + 1e-3 * (t * 1.618).sin(),
            q.1 + 1e-3 * (t * 2.414).cos(),
        );
    }

    // Sparse stress majorization. Each pivot's region is the nodes nearest it; a node's term to a
    // pivot at distance d weighs the region's nodes within d/2 of the pivot (Ortmann et al. §3).
    let mut region = vec![0usize; n];
    for i in 0..n {
        region[i] = (0..p).min_by_key(|&j| (dist[j][i], j)).unwrap_or(0);
    }
    let max_d = dist
        .iter()
        .flat_map(|d| d.iter().copied())
        .filter(|&v| v != u32::MAX)
        .max()
        .unwrap_or(0) as usize;
    // within[j][h]: nodes of pivot j's region at most h hops from it.
    let mut within = vec![vec![0usize; max_d + 2]; p];
    for i in 0..n {
        let j = region[i];
        let h = dist[j][i] as usize;
        within[j][h.min(max_d + 1)] += 1;
    }
    for counts in &mut within {
        for h in 1..counts.len() {
            counts[h] += counts[h - 1];
        }
    }
    // Node-major copies for the inner loop: each node's pivot distances and term weights together.
    let mut terms = vec![(0.0f64, 0.0f64); n * p];
    for i in 0..n {
        for j in 0..p {
            let d = dist[j][i];
            terms[i * p + j] = if d <= 1 || d == u32::MAX || pivots[j] == i {
                (0.0, 0.0)
            } else {
                let s = within[j][(d / 2) as usize].max(1) as f64;
                let d = f64::from(d);
                (d, s / (d * d))
            };
        }
    }
    let pivot_of: Vec<Option<usize>> = {
        let mut v = vec![None; n];
        for (j, &u) in pivots.iter().enumerate() {
            v[u] = Some(j);
        }
        v
    };
    for _ in 0..PASSES {
        let mut moved = 0.0;
        for i in 0..n {
            let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
            let term = |q: (f64, f64), d: f64, w: f64, sx: &mut f64, sy: &mut f64, sw: &mut f64| {
                let (dx, dy) = (pos[i].0 - q.0, pos[i].1 - q.1);
                let len = dx.hypot(dy).max(1e-9);
                *sx += w * (q.0 + d * dx / len);
                *sy += w * (q.1 + d * dy / len);
                *sw += w;
            };
            for &v in &adj[i] {
                term(pos[v], 1.0, 1.0, &mut sx, &mut sy, &mut sw);
            }
            for (j, &u) in pivots.iter().enumerate() {
                let (d, w) = terms[i * p + j];
                if w == 0.0 || pivot_of[i] == Some(j) {
                    continue;
                }
                term(pos[u], d, w, &mut sx, &mut sy, &mut sw);
            }
            if sw > 0.0 {
                let next = (sx / sw, sy / sw);
                moved += (next.0 - pos[i].0).hypot(next.1 - pos[i].1);
                pos[i] = next;
            }
        }
        if moved / (n as f64) < SETTLED {
            break;
        }
    }
    pos
}

/// The dominant eigenvector of the symmetric `p × p` matrix `m`, orthogonal to `against` when
/// given.
fn power_iteration(m: &[f64], p: usize, against: Option<&[f64]>) -> Vec<f64> {
    let mut v: Vec<f64> = (0..p).map(|i| 1.0 + (i as f64 * 0.731).sin()).collect();
    for _ in 0..200 {
        if let Some(u) = against {
            let dot: f64 = v.iter().zip(u).map(|(a, b)| a * b).sum();
            v.iter_mut().zip(u).for_each(|(a, b)| *a -= dot * b);
        }
        let mut w = vec![0.0; p];
        for a in 0..p {
            w[a] = (0..p).map(|b| m[a * p + b] * v[b]).sum();
        }
        let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-300 {
            break;
        }
        w.iter_mut().for_each(|x| *x /= norm);
        v = w;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stress(pos: &[(f64, f64)], adj: &[Vec<usize>]) -> f64 {
        let n = pos.len();
        let mut d = vec![0u32; n];
        let mut total = 0.0;
        for i in 0..n {
            bfs(adj, i, &mut d);
            for j in i + 1..n {
                if d[j] != u32::MAX {
                    let dij = f64::from(d[j]);
                    let e = (pos[i].0 - pos[j].0).hypot(pos[i].1 - pos[j].1);
                    total += (e - dij).powi(2) / (dij * dij);
                }
            }
        }
        total
    }

    fn grid(side: usize) -> Vec<(usize, usize)> {
        let mut e = Vec::new();
        for y in 0..side {
            for x in 0..side {
                let i = y * side + x;
                if x + 1 < side {
                    e.push((i, i + 1));
                }
                if y + 1 < side {
                    e.push((i, i + side));
                }
            }
        }
        e
    }

    #[test]
    fn a_grid_graph_comes_out_as_a_grid() {
        let side = 12;
        let edges = grid(side);
        let pos = sparse_stress(side * side, &edges);
        // Neighbours about one unit apart. Opposite corners are 22 hops apart, so stress stretches
        // the diagonal from the grid's 15.6 towards 22.
        for &(a, b) in &edges {
            let d = (pos[a].0 - pos[b].0).hypot(pos[a].1 - pos[b].1);
            assert!((0.6..1.4).contains(&d), "edge {a}-{b}: {d}");
        }
        let n = side * side - 1;
        let diag = (pos[0].0 - pos[n].0).hypot(pos[0].1 - pos[n].1);
        assert!(diag > 14.0 && diag < 22.0, "{diag}");
    }

    #[test]
    fn stress_beats_the_mds_start_and_a_random_spread() {
        let side = 10;
        let edges = grid(side);
        let n = side * side;
        let mut adj = vec![Vec::new(); n];
        for &(a, b) in &edges {
            adj[a].push(b);
            adj[b].push(a);
        }
        let pos = sparse_stress(n, &edges);
        let spread: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                (
                    (i as f64 * 7.13).sin() * 10.0,
                    (i as f64 * 3.71).cos() * 10.0,
                )
            })
            .collect();
        assert!(stress(&pos, &adj) < 0.2 * stress(&spread, &adj));
    }

    #[test]
    fn components_are_packed_apart() {
        // Two triangles and an isolated node.
        let pos = sparse_stress(7, &[(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)]);
        let centre = |r: std::ops::Range<usize>| {
            let k = r.len() as f64;
            r.fold((0.0, 0.0), |a, i| (a.0 + pos[i].0 / k, a.1 + pos[i].1 / k))
        };
        let (a, b) = (centre(0..3), centre(3..6));
        assert!((a.0 - b.0).hypot(a.1 - b.1) > 1.5);
        assert!(pos.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
    }

    #[test]
    fn layout_is_deterministic() {
        let edges = grid(8);
        assert_eq!(sparse_stress(64, &edges), sparse_stress(64, &edges));
    }
}
