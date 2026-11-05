//! Substation overview layouts for balanced (transmission) networks.
//!
//! A transmission grid is meshed, so the multiconductor schematic's breadth-first tree does not
//! apply. This module follows Birchfield and Overbye, "Techniques for drawing geographic one-line
//! diagrams: substation spacing and line routing", IEEE Trans. Power Systems 33(6), 2018:
//!
//! 1. **Substation interiors** are drawn independently: one vertical busbar per bus, ordered by
//!    nominal voltage, each as tall as its connections need. Branches inside a substation
//!    (transformers, couplers) join their busbars straight across where they can, else with two
//!    right angles. External branches leave from slots ordered by the direction of their far end,
//!    so they do not cross inside the box.
//! 2. **Placement** is greedy: substations in priority order (highest voltage, then size) take the
//!    candidate point nearest their seed (geographic position) whose box, with a buffer, overlaps
//!    no box placed before. Substations in dense areas are drawn smaller, in three tiers, so cities
//!    do not push their neighbours far from their true position.
//! 3. **Lines** run straight between their attachment slots.
//!
//! Output is in the units of the seed positions (Web Mercator metres for geographic cases), so the
//! schematic stays registered to the basemap and buses can glide from their true positions into
//! their substation. The layout is a drawing, not an electrical claim. Every step is deterministic.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Bump when the geometry for the same input and options changes.
pub const TX_LAYOUT_VERSION: u32 = 1;

/// Edge roles in [`TxLayout::role`].
pub const TX_ROLE_LINE: u8 = 0;
pub const TX_ROLE_INTERNAL: u8 = 1;
pub const TX_ROLE_OPEN: u8 = 2;

/// The network as columns, in payload order (see `txGraph` in `@tellegen/svelte`).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct TxLayoutInput {
    /// Substation index per bus.
    pub bus_substation: Vec<u32>,
    /// Nominal kV per bus (0 when unknown).
    pub kv: Vec<f64>,
    /// Bus row of each branch end; negative when the bus is not drawn.
    pub from: Vec<i64>,
    pub to: Vec<i64>,
    pub in_service: Vec<u8>,
    /// `x, y` per bus (y down): the seed positions.
    pub xy: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TxLayoutOptions {
    /// Typical substation box size as a fraction of the median distance between neighbouring
    /// substations. Smaller keeps substations nearer their seeds; larger shows more detail.
    pub fill: f64,
    /// Size tiers by local density: a substation with at least `tier_counts[0]` (`[1]`) other
    /// substations within `tier_radius` median spacings is drawn at `tier_scales[0]` (`[1]`).
    pub tier_radius: f64,
    pub tier_counts: [usize; 2],
    pub tier_scales: [f64; 2],
    /// Candidate offsets either side of the seed, per axis (Birchfield uses 10: a 21 × 21 grid).
    pub search: i32,
    /// Rounds of channel spacing in the Delaunay line routing (Birchfield uses at least 10); 0
    /// draws lines straight between their boxes.
    pub route_iterations: usize,
}

impl Default for TxLayoutOptions {
    fn default() -> Self {
        Self {
            fill: 0.45,
            tier_radius: 3.0,
            tier_counts: [12, 30],
            tier_scales: [2.0 / 3.0, 4.0 / 9.0],
            search: 10,
            route_iterations: 10,
        }
    }
}

/// Columnar layout output, sized for transfer to the browser.
#[derive(Clone, Debug, Serialize)]
pub struct TxLayout {
    pub version: u32,
    /// `x, y` per bus: the middle of its busbar.
    pub positions: Vec<f64>,
    /// `x0, y0, x1, y1` per bus: its busbar, top to bottom.
    pub busbars: Vec<f64>,
    /// `x0, y0, x1, y1` per substation: its box.
    pub boxes: Vec<f64>,
    /// `x0, y0, x1, y1` per drawn segment; a routed branch has several.
    pub segments: Vec<f64>,
    /// Branch index for each segment.
    pub segment_edge: Vec<u32>,
    /// Per branch: [`TX_ROLE_LINE`], [`TX_ROLE_INTERNAL`] or [`TX_ROLE_OPEN`].
    pub role: Vec<u8>,
    /// Drawing units per block: the grid unit of substation interiors before tier scaling.
    pub unit: f64,
    /// Scale per substation (its tier).
    pub scale: Vec<f64>,
    pub bounds: [f64; 4],
}

/// Interior grid, in blocks.
const BAR_GAP: f64 = 2.0;
const PAD: f64 = 1.0;
const MIN_BAR: f64 = 2.0;
/// Buffer kept free around each box, in blocks.
const BUFFER: f64 = 1.0;

struct Interior {
    /// Members in drawing order (highest kV first).
    buses: Vec<usize>,
    /// Box size in blocks.
    w: f64,
    h: f64,
}

pub fn tx_schematic_layout(input: &TxLayoutInput, options: &TxLayoutOptions) -> TxLayout {
    let n = input.bus_substation.len();
    let m = input.from.len();
    let k = input
        .bus_substation
        .iter()
        .map(|&s| s as usize + 1)
        .max()
        .unwrap_or(0);
    let bus_at = |row: i64| (row >= 0 && (row as usize) < n).then_some(row as usize);
    let sub_of = |row: usize| input.bus_substation[row] as usize;
    let seed_bus = |i: usize| {
        let (x, y) = (
            input.xy.get(i * 2).copied(),
            input.xy.get(i * 2 + 1).copied(),
        );
        match (x, y) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => (x, y),
            _ => (0.0, 0.0),
        }
    };

    // Seeds: the mean of each substation's bus positions.
    let mut seed = vec![(0.0, 0.0); k];
    let mut count = vec![0usize; k];
    for i in 0..n {
        let (x, y) = seed_bus(i);
        let s = sub_of(i);
        seed[s].0 += x;
        seed[s].1 += y;
        count[s] += 1;
    }
    for s in 0..k {
        if count[s] > 0 {
            seed[s].0 /= count[s] as f64;
            seed[s].1 /= count[s] as f64;
        }
    }

    // Interiors: members by voltage, and slot assignment per busbar.
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); k];
    for i in 0..n {
        members[sub_of(i)].push(i);
    }
    for list in &mut members {
        list.sort_by(|&a, &b| input.kv[b].total_cmp(&input.kv[a]).then(a.cmp(&b)));
    }
    let column: Vec<usize> = {
        let mut c = vec![0usize; n];
        for list in &members {
            for (j, &i) in list.iter().enumerate() {
                c[i] = j;
            }
        }
        c
    };
    let ends = |e: usize| Some((bus_at(input.from[e])?, bus_at(input.to[e])?));
    let live = |e: usize| input.in_service.get(e).copied().unwrap_or(1) != 0;

    // Rows: every connection in a substation gets its own row, so lines leave the box without
    // overlapping. Internal branches first, one row each, straight across between their bars.
    let mut next_row = vec![0usize; k];
    let mut row_of: Vec<[usize; 2]> = vec![[usize::MAX; 2]; m];
    for (e, rows) in row_of.iter_mut().enumerate() {
        let Some((a, b)) = ends(e) else { continue };
        if a == b || sub_of(a) != sub_of(b) || !live(e) {
            continue;
        }
        let s = sub_of(a);
        *rows = [next_row[s]; 2];
        next_row[s] += 1;
    }
    // External ends, per substation, ordered top to bottom by the far end's seed so lines fan out
    // without crossing inside the box.
    let mut external: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new(); k];
    for e in 0..m {
        let Some((a, b)) = ends(e) else { continue };
        if a == b || sub_of(a) == sub_of(b) || !live(e) {
            continue;
        }
        external[sub_of(a)].push((seed[sub_of(b)].1, e, 0));
        external[sub_of(b)].push((seed[sub_of(a)].1, e, 1));
    }
    for (s, list) in external.iter_mut().enumerate() {
        list.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
        for &(_, e, side) in list.iter() {
            row_of[e][side] = next_row[s];
            next_row[s] += 1;
        }
    }
    // Each busbar spans the rows its connections use.
    let mut bar_rows = vec![(usize::MAX, 0usize); n];
    for (e, rows) in row_of.iter().enumerate() {
        let Some((a, b)) = ends(e) else { continue };
        for (bus, row) in [(a, rows[0]), (b, rows[1])] {
            if row != usize::MAX {
                bar_rows[bus] = (bar_rows[bus].0.min(row), bar_rows[bus].1.max(row));
            }
        }
    }
    let interiors: Vec<Interior> = members
        .into_iter()
        .enumerate()
        .map(|(s, buses)| {
            let w = (buses.len().saturating_sub(1)) as f64 * BAR_GAP + 2.0 * PAD;
            let h = (next_row[s] as f64).max(MIN_BAR) + 2.0 * PAD;
            Interior { buses, w, h }
        })
        .collect();

    // Scale: blocks per drawing unit from the median nearest-neighbour spacing of the seeds.
    let occupied: Vec<usize> = (0..k).filter(|&s| count[s] > 0).collect();
    let nn = nearest_neighbour_distances(&occupied.iter().map(|&s| seed[s]).collect::<Vec<_>>());
    let spacing = median(&nn).filter(|d| *d > 0.0).unwrap_or(1.0);
    let typical = median(
        &occupied
            .iter()
            .map(|&s| interiors[s].w.max(interiors[s].h))
            .collect::<Vec<_>>(),
    )
    .unwrap_or(1.0)
    .max(1.0);
    let unit = options.fill * spacing / typical;

    // Tiers by local density.
    let radius = options.tier_radius * spacing;
    let neighbours = count_within(
        &occupied.iter().map(|&s| seed[s]).collect::<Vec<_>>(),
        radius,
    );
    let mut scale = vec![1.0; k];
    for (j, &s) in occupied.iter().enumerate() {
        scale[s] = if neighbours[j] >= options.tier_counts[1] {
            options.tier_scales[1]
        } else if neighbours[j] >= options.tier_counts[0] {
            options.tier_scales[0]
        } else {
            1.0
        };
    }

    // Greedy placement, highest voltage and largest first.
    let mut order = occupied.clone();
    let top_kv = |s: usize| interiors[s].buses.first().map_or(0.0, |&i| input.kv[i]);
    order.sort_by(|&a, &b| {
        top_kv(b)
            .total_cmp(&top_kv(a))
            .then(interiors[b].buses.len().cmp(&interiors[a].buses.len()))
            .then(a.cmp(&b))
    });
    let half = |s: usize| {
        let f = unit * scale[s];
        (interiors[s].w * f / 2.0, interiors[s].h * f / 2.0)
    };
    let largest = occupied
        .iter()
        .map(|&s| {
            let (hw, hh) = half(s);
            2.0 * hw.max(hh)
        })
        .fold(0.0, f64::max)
        + 2.0 * BUFFER * unit;
    let mut grid = BoxGrid::new(largest.max(f64::MIN_POSITIVE));
    let mut centre = seed.clone();
    let mut offsets: Vec<(i32, i32)> = Vec::new();
    for dj in -options.search..=options.search {
        for dk in -options.search..=options.search {
            offsets.push((dj, dk));
        }
    }
    offsets.sort_by_key(|&(a, b)| (a * a + b * b, a, b));
    for &s in &order {
        let (hw, hh) = half(s);
        let buffer = BUFFER * unit * scale[s];
        let step = hw.max(hh);
        let (x0, y0) = seed[s];
        let mut chosen = None;
        // Widen the search when a dense area leaves no free spot nearby.
        'widen: for widen in [1.0, 2.0, 4.0] {
            for &(dj, dk) in &offsets {
                let c = (x0 + dj as f64 * step * widen, y0 + dk as f64 * step * widen);
                let bounds = [
                    c.0 - hw - buffer,
                    c.1 - hh - buffer,
                    c.0 + hw + buffer,
                    c.1 + hh + buffer,
                ];
                if !grid.overlaps(&bounds) {
                    chosen = Some(c);
                    break 'widen;
                }
            }
        }
        let c = chosen.unwrap_or((x0, y0));
        centre[s] = c;
        grid.insert([c.0 - hw, c.1 - hh, c.0 + hw, c.1 + hh]);
    }

    // Buses and busbars.
    let frame = |s: usize| {
        let f = unit * scale[s];
        let (hw, hh) = (interiors[s].w * f / 2.0, interiors[s].h * f / 2.0);
        (f, centre[s].0 - hw, centre[s].1 - hh, centre[s].0 + hw)
    };
    let bar_x = |i: usize| {
        let (f, left, _, _) = frame(sub_of(i));
        left + (PAD + column[i] as f64 * BAR_GAP) * f
    };
    let row_y = |s: usize, row: usize| {
        let (f, _, top, _) = frame(s);
        top + (PAD + row as f64 + 0.5) * f
    };
    let mut positions = vec![0.0; n * 2];
    let mut busbars = vec![0.0; n * 4];
    for i in 0..n {
        let s = sub_of(i);
        let (f, _, top, _) = frame(s);
        let (lo, hi) = bar_rows[i];
        // A bar reaches half a row past its outer connections; an unconnected bar sits at the top.
        let (mut y0, mut y1) = if lo == usize::MAX {
            (top + PAD * f, top + (PAD + MIN_BAR) * f)
        } else {
            (row_y(s, lo) - 0.5 * f, row_y(s, hi) + 0.5 * f)
        };
        if y1 - y0 < MIN_BAR * f {
            let mid = (y0 + y1) / 2.0;
            (y0, y1) = (mid - MIN_BAR * f / 2.0, mid + MIN_BAR * f / 2.0);
        }
        let x = bar_x(i);
        busbars[i * 4..i * 4 + 4].copy_from_slice(&[x, y0, x, y1]);
        positions[i * 2] = x;
        positions[i * 2 + 1] = (y0 + y1) / 2.0;
    }
    let mut boxes = vec![0.0; k * 4];
    for &s in &occupied {
        let (hw, hh) = half(s);
        let c = centre[s];
        boxes[s * 4..s * 4 + 4].copy_from_slice(&[c.0 - hw, c.1 - hh, c.0 + hw, c.1 + hh]);
    }

    // Branches. A line runs from its bar along its row to the box side facing the far substation,
    // then straight across to the far box and along its row to the far bar.
    let mut segments = Vec::new();
    let mut segment_edge = Vec::new();
    let mut role = vec![TX_ROLE_OPEN; m];
    let mut push = |pts: &[(f64, f64)], e: usize| {
        for w in pts.windows(2) {
            if (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs() > 0.0 {
                segments.extend_from_slice(&[w[0].0, w[0].1, w[1].0, w[1].1]);
                segment_edge.push(e as u32);
            }
        }
    };
    let mut lines: Vec<LineRequest> = Vec::new();
    for e in 0..m {
        let Some((a, b)) = ends(e) else { continue };
        if a == b {
            continue;
        }
        let [ra, rb] = row_of[e];
        if !live(e) || ra == usize::MAX {
            role[e] = TX_ROLE_OPEN;
            push(
                &[
                    (positions[a * 2], positions[a * 2 + 1]),
                    (positions[b * 2], positions[b * 2 + 1]),
                ],
                e,
            );
            continue;
        }
        let (sa, sb) = (sub_of(a), sub_of(b));
        let p = (bar_x(a), row_y(sa, ra));
        let q = (bar_x(b), row_y(sb, rb));
        if sa == sb {
            role[e] = TX_ROLE_INTERNAL;
            push(&[p, q], e);
            continue;
        }
        role[e] = TX_ROLE_LINE;
        let exit = |s: usize, other: usize, y: f64| {
            let (_, left, _, right) = frame(s);
            (
                if centre[other].0 >= centre[s].0 {
                    right
                } else {
                    left
                },
                y,
            )
        };
        lines.push(LineRequest {
            e,
            sa,
            sb,
            slots: [p, q],
            exits: [exit(sa, sb, p.1), exit(sb, sa, q.1)],
        });
    }
    // Lines between substations route through the channels of the Delaunay triangulation of the
    // substation centres, clear of the boxes they pass (Birchfield & Overbye §IV-B).
    let gap = 0.8 * unit;
    let waypoints = if options.route_iterations > 0 {
        route_lines(
            &occupied,
            &centre,
            &boxes,
            &scale,
            &lines,
            gap,
            options.route_iterations,
        )
    } else {
        vec![Vec::new(); lines.len()]
    };
    // Legs that still clip another substation (near a box corner, where channel spacing does not
    // look) step around it.
    let obstacles = BoxIndex::new(&boxes, &occupied);
    for (line, via) in lines.iter().zip(&waypoints) {
        let mut middle = vec![line.exits[0]];
        middle.extend_from_slice(via);
        middle.push(line.exits[1]);
        if options.route_iterations > 0 {
            detour(&mut middle, &obstacles, [line.sa, line.sb], gap);
        }
        let mut pts = vec![line.slots[0]];
        pts.extend_from_slice(&middle);
        pts.push(line.slots[1]);
        push(&pts, line.e);
    }

    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for &s in &occupied {
        let b = &boxes[s * 4..s * 4 + 4];
        bounds = [
            bounds[0].min(b[0]),
            bounds[1].min(b[1]),
            bounds[2].max(b[2]),
            bounds[3].max(b[3]),
        ];
    }
    if !bounds[0].is_finite() {
        bounds = [0.0, 0.0, 1.0, 1.0];
    }
    TxLayout {
        version: TX_LAYOUT_VERSION,
        positions,
        busbars,
        boxes,
        segments,
        segment_edge,
        role,
        unit,
        scale,
        bounds,
    }
}

/// A line between two substations, before routing: its bar slots and box exits at both ends.
struct LineRequest {
    e: usize,
    sa: usize,
    sb: usize,
    slots: [(f64, f64); 2],
    exits: [(f64, f64); 2],
}

/// One crossing of a line with a Delaunay channel: where it sits along the channel (0 at `u`).
struct Crossing {
    line: usize,
    /// Its position in the line's sequence of crossings.
    k: usize,
    t: f64,
}

/// A Delaunay edge between two substation centres that lines pass through.
struct Channel {
    u: (f64, f64),
    w: (f64, f64),
    /// Allowed range along the channel, clear of both boxes.
    lo: f64,
    hi: f64,
    /// Minimum spacing between lines, as a fraction of the channel length.
    gap: f64,
    crossings: Vec<Crossing>,
}

/// Waypoints for each line (Birchfield & Overbye §IV-B): trace the straight line between its two
/// substation centres through the Delaunay triangulation of all centres, add a waypoint on each
/// channel (triangle edge) it crosses, then repeatedly space the waypoints on each channel: each
/// takes the point nearest where the line through its neighbours crosses the channel, clear of
/// the two substations' boxes and of the lines already placed on that channel.
fn route_lines(
    occupied: &[usize],
    centre: &[(f64, f64)],
    boxes: &[f64],
    scale: &[f64],
    lines: &[LineRequest],
    gap: f64,
    iterations: usize,
) -> Vec<Vec<(f64, f64)>> {
    let mut out = vec![Vec::new(); lines.len()];
    if occupied.len() < 3 {
        return out;
    }
    let points: Vec<delaunator::Point> = occupied
        .iter()
        .map(|&s| delaunator::Point {
            x: centre[s].0,
            y: centre[s].1,
        })
        .collect();
    let tri = delaunator::triangulate(&points);
    if tri.triangles.is_empty() {
        return out;
    }
    let mut vertex_of = HashMap::with_capacity(occupied.len());
    for (v, &s) in occupied.iter().enumerate() {
        vertex_of.insert(s, v);
    }
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); occupied.len()];
    for t in 0..tri.triangles.len() / 3 {
        for k in 0..3 {
            around[tri.triangles[t * 3 + k]].push(t);
        }
    }
    let next = |h: usize| if h % 3 == 2 { h - 2 } else { h + 1 };
    let pt = |v: usize| (points[v].x, points[v].y);

    // Trace each line; channels keyed by their undirected vertex pair.
    let mut channels: HashMap<(usize, usize), usize> = HashMap::new();
    let mut list: Vec<Channel> = Vec::new();
    // Per line, its crossings in order as (channel, crossing index on that channel).
    let mut path: Vec<Vec<(usize, usize)>> = vec![Vec::new(); lines.len()];
    for (l, line) in lines.iter().enumerate() {
        let (Some(&va), Some(&vb)) = (vertex_of.get(&line.sa), vertex_of.get(&line.sb)) else {
            continue;
        };
        let (p, q) = (pt(va), pt(vb));
        // The halfedge opposite `va` in a triangle around it that the line leaves through.
        let mut crossed = None;
        for &t in &around[va] {
            let hs = [t * 3, t * 3 + 1, t * 3 + 2];
            if hs.iter().any(|&h| tri.triangles[h] == vb) {
                break;
            }
            for &h in &hs {
                let (a, b) = (tri.triangles[h], tri.triangles[next(h)]);
                if a != va && b != va && segments_cross(p, q, pt(a), pt(b)) {
                    crossed = Some(h);
                }
            }
            if crossed.is_some() {
                break;
            }
        }
        let mut steps = 0;
        while let Some(h) = crossed {
            steps += 1;
            if steps > tri.triangles.len() {
                path[l].clear();
                break;
            }
            let (a, b) = (tri.triangles[h], tri.triangles[next(h)]);
            let key = (a.min(b), a.max(b));
            let c = *channels.entry(key).or_insert_with(|| {
                list.push(channel(
                    pt(key.0),
                    pt(key.1),
                    occupied[key.0],
                    occupied[key.1],
                    boxes,
                    scale,
                    gap,
                ));
                list.len() - 1
            });
            let t = line_param(p, q, list[c].u, list[c].w).unwrap_or(0.5);
            let k = path[l].len();
            path[l].push((c, list[c].crossings.len()));
            list[c].crossings.push(Crossing { line: l, k, t });
            // Into the neighbouring triangle, and out through one of its other two edges.
            let twin = tri.halfedges[h];
            if twin == delaunator::EMPTY {
                break;
            }
            let t3 = twin - twin % 3;
            let third = (0..3)
                .map(|k| tri.triangles[t3 + k])
                .find(|&v| v != a && v != b);
            if third == Some(vb) {
                break;
            }
            crossed = [next(twin), next(next(twin))].into_iter().find(|&g| {
                let (x, y) = (tri.triangles[g], tri.triangles[next(g)]);
                segments_cross(p, q, pt(x), pt(y))
            });
            if crossed.is_none() {
                // The line runs through a vertex: leave it straight.
                path[l].clear();
            }
        }
    }

    // Where each line's waypoint `j` is now.
    let at = |list: &[Channel], (c, j): (usize, usize)| {
        let ch = &list[c];
        let t = ch.crossings[j].t;
        (
            ch.u.0 + t * (ch.w.0 - ch.u.0),
            ch.u.1 + t * (ch.w.1 - ch.u.1),
        )
    };
    for _ in 0..iterations {
        for c in 0..list.len() {
            let mut desired: Vec<(f64, usize)> = Vec::with_capacity(list[c].crossings.len());
            for (j, x) in list[c].crossings.iter().enumerate() {
                let codes = &path[x.line];
                // A line whose trace was abandoned keeps its crossing where it is.
                if codes.get(x.k) != Some(&(c, j)) {
                    desired.push((x.t, j));
                    continue;
                }
                let k = x.k;
                let prev = if k == 0 {
                    lines[x.line].exits[0]
                } else {
                    at(&list, codes[k - 1])
                };
                let next = if k + 1 == codes.len() {
                    lines[x.line].exits[1]
                } else {
                    at(&list, codes[k + 1])
                };
                let t = line_param(prev, next, list[c].u, list[c].w).unwrap_or(x.t);
                desired.push((t, j));
            }
            let ch = &mut list[c];
            let placed = space(&mut desired, ch.lo, ch.hi, ch.gap);
            for (j, t) in placed {
                ch.crossings[j].t = t;
            }
        }
    }
    for (l, codes) in path.iter().enumerate() {
        out[l] = codes.iter().map(|&code| at(&list, code)).collect();
    }
    out
}

/// Substation boxes on a grid, for finding the boxes a segment may cross.
struct BoxIndex<'a> {
    boxes: &'a [f64],
    cell: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl<'a> BoxIndex<'a> {
    fn new(boxes: &'a [f64], occupied: &[usize]) -> Self {
        let cell = occupied
            .iter()
            .map(|&s| (boxes[s * 4 + 2] - boxes[s * 4]).max(boxes[s * 4 + 3] - boxes[s * 4 + 1]))
            .fold(0.0, f64::max)
            .max(f64::MIN_POSITIVE);
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for &s in occupied {
            let b = &boxes[s * 4..s * 4 + 4];
            let (x0, y0) = ((b[0] / cell).floor() as i64, (b[1] / cell).floor() as i64);
            let (x1, y1) = ((b[2] / cell).floor() as i64, (b[3] / cell).floor() as i64);
            for x in x0..=x1 {
                for y in y0..=y1 {
                    cells.entry((x, y)).or_default().push(s);
                }
            }
        }
        Self { boxes, cell, cells }
    }

    /// The first box along `p → q` that the segment crosses, other than `skip`. Walks exactly
    /// the grid cells the segment passes through (Amanatides & Woo); every box is registered in
    /// each cell it overlaps, so no box it crosses is missed.
    fn first_hit(&self, p: (f64, f64), q: (f64, f64), skip: [usize; 2]) -> Option<usize> {
        let c = self.cell;
        let (mut cx, mut cy) = ((p.0 / c).floor() as i64, (p.1 / c).floor() as i64);
        let (ex, ey) = ((q.0 / c).floor() as i64, (q.1 / c).floor() as i64);
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let (sx, sy) = (dx.signum() as i64, dy.signum() as i64);
        let boundary = |cell: i64, step: i64| (cell + i64::from(step > 0)) as f64 * c;
        let mut t_max_x = if dx != 0.0 {
            (boundary(cx, sx) - p.0) / dx
        } else {
            f64::INFINITY
        };
        let mut t_max_y = if dy != 0.0 {
            (boundary(cy, sy) - p.1) / dy
        } else {
            f64::INFINITY
        };
        let t_dx = if dx != 0.0 {
            c / dx.abs()
        } else {
            f64::INFINITY
        };
        let t_dy = if dy != 0.0 {
            c / dy.abs()
        } else {
            f64::INFINITY
        };
        let steps = ((ex - cx).abs() + (ey - cy).abs()) as usize;
        let mut best: Option<(f64, usize)> = None;
        for _ in 0..=steps.min(1_000_000) {
            if let Some(ids) = self.cells.get(&(cx, cy)) {
                for &s in ids {
                    if skip.contains(&s) {
                        continue;
                    }
                    let b = &self.boxes[s * 4..s * 4 + 4];
                    if segment_hits_box(&[p.0, p.1, q.0, q.1], b) {
                        let m = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
                        let d = (m.0 - p.0).hypot(m.1 - p.1);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, s));
                        }
                    }
                }
            }
            // A hit in this cell may still lie beyond a box in the next one; boxes span at most
            // two cells along an axis, so finishing the following cell settles which is first.
            if best.is_some() && (t_max_x.min(t_max_y) > 1.0 || (cx, cy) != (ex, ey)) {
                if let Some((d, _)) = best {
                    let reached = t_max_x.min(t_max_y) * dx.hypot(dy);
                    if reached > d + c {
                        break;
                    }
                }
            }
            if (cx, cy) == (ex, ey) {
                break;
            }
            if t_max_x < t_max_y {
                t_max_x += t_dx;
                cx += sx;
            } else {
                t_max_y += t_dy;
                cy += sy;
            }
        }
        best.map(|(_, s)| s)
    }
}

/// Step each leg of `path` that crosses a box (other than its own substations') around the corner
/// of that box that adds the least length, a few times at most per line.
fn detour(path: &mut Vec<(f64, f64)>, index: &BoxIndex, own: [usize; 2], gap: f64) {
    let mut inserted = 0;
    let mut i = 0;
    while i + 1 < path.len() && inserted < 4 {
        let (p, q) = (path[i], path[i + 1]);
        let Some(s) = index.first_hit(p, q, own) else {
            i += 1;
            continue;
        };
        let b = &index.boxes[s * 4..s * 4 + 4];
        let g = gap * 0.5;
        let corners = [
            (b[0] - g, b[1] - g),
            (b[2] + g, b[1] - g),
            (b[2] + g, b[3] + g),
            (b[0] - g, b[3] + g),
        ];
        let cost = |c: &(f64, f64)| (c.0 - p.0).hypot(c.1 - p.1) + (q.0 - c.0).hypot(q.1 - c.1);
        let shrunk = [b[0] + 1e-9, b[1] + 1e-9, b[2] - 1e-9, b[3] - 1e-9];
        // Prefer a corner whose two new legs clear this box; else the shortest.
        let corner = corners
            .iter()
            .filter(|c| {
                !segment_hits_box(&[p.0, p.1, c.0, c.1], &shrunk)
                    && !segment_hits_box(&[c.0, c.1, q.0, q.1], &shrunk)
            })
            .min_by(|a, b| cost(a).total_cmp(&cost(b)))
            .or_else(|| corners.iter().min_by(|a, b| cost(a).total_cmp(&cost(b))))
            .copied();
        let Some(c) = corner else { break };
        path.insert(i + 1, c);
        inserted += 1;
    }
}

/// A channel between substations `su` and `sv` with centres `u` and `w`: the stretch of the edge
/// outside both boxes (plus a gap) is where lines may cross it.
fn channel(
    u: (f64, f64),
    w: (f64, f64),
    su: usize,
    sv: usize,
    boxes: &[f64],
    scale: &[f64],
    gap: f64,
) -> Channel {
    let len = (w.0 - u.0).hypot(w.1 - u.1).max(f64::MIN_POSITIVE);
    let d = ((w.0 - u.0) / len, (w.1 - u.1) / len);
    // Distance from a box's centre along `d` to its edge.
    let reach = |s: usize| {
        let b = &boxes[s * 4..s * 4 + 4];
        let (hw, hh) = ((b[2] - b[0]) / 2.0, (b[3] - b[1]) / 2.0);
        let tx = if d.0.abs() > 1e-12 {
            hw / d.0.abs()
        } else {
            f64::INFINITY
        };
        let ty = if d.1.abs() > 1e-12 {
            hh / d.1.abs()
        } else {
            f64::INFINITY
        };
        tx.min(ty)
    };
    let g = gap * scale[su].min(scale[sv]);
    let (mut lo, mut hi) = ((reach(su) + g) / len, 1.0 - (reach(sv) + g) / len);
    if lo > hi {
        let mid = (lo + hi) / 2.0;
        (lo, hi) = (mid, mid);
    }
    Channel {
        u,
        w,
        lo: lo.clamp(0.0, 1.0),
        hi: hi.clamp(0.0, 1.0),
        gap: g / len,
        crossings: Vec::new(),
    }
}

/// Place waypoints along a channel nearest their desired positions, in order, at least `gap`
/// apart, within `[lo, hi]`; spread evenly when they do not fit.
fn space(desired: &mut [(f64, usize)], lo: f64, hi: f64, gap: f64) -> Vec<(usize, f64)> {
    desired.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let n = desired.len();
    if n == 0 {
        return Vec::new();
    }
    let span = hi - lo;
    if (n - 1) as f64 * gap > span {
        return desired
            .iter()
            .enumerate()
            .map(|(i, &(_, j))| {
                let t = if n == 1 {
                    (lo + hi) / 2.0
                } else {
                    lo + span * i as f64 / (n - 1) as f64
                };
                (j, t)
            })
            .collect();
    }
    let mut t: Vec<f64> = desired.iter().map(|d| d.0.clamp(lo, hi)).collect();
    for i in 1..n {
        t[i] = t[i].max(t[i - 1] + gap);
    }
    t[n - 1] = t[n - 1].min(hi);
    for i in (0..n - 1).rev() {
        t[i] = t[i].min(t[i + 1] - gap);
    }
    desired.iter().zip(t).map(|(&(_, j), t)| (j, t)).collect()
}

/// Whether segments `p–q` and `a–b` cross at a point inside both.
fn segments_cross(p: (f64, f64), q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> bool {
    let orient = |o: (f64, f64), x: (f64, f64), y: (f64, f64)| {
        (x.0 - o.0) * (y.1 - o.1) - (x.1 - o.1) * (y.0 - o.0)
    };
    let (d1, d2) = (orient(a, b, p), orient(a, b, q));
    let (d3, d4) = (orient(p, q, a), orient(p, q, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Where the line through `p` and `q` crosses segment `u–w`, as a fraction along `u–w`; None when
/// they are parallel.
fn line_param(p: (f64, f64), q: (f64, f64), u: (f64, f64), w: (f64, f64)) -> Option<f64> {
    let (r, s) = ((q.0 - p.0, q.1 - p.1), (w.0 - u.0, w.1 - u.1));
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.abs() < 1e-12 * (r.0.hypot(r.1) * s.0.hypot(s.1)).max(f64::MIN_POSITIVE) {
        return None;
    }
    let t = ((u.0 - p.0) * r.1 - (u.1 - p.1) * r.0) / denom;
    Some(t.clamp(0.0, 1.0))
}

fn median(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// Points bucketed on a uniform grid of square cells.
struct PointGrid {
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl PointGrid {
    fn new(points: &[(f64, f64)], cell: f64) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            cells.entry(Self::key(p, cell)).or_default().push(i);
        }
        Self { cells }
    }
    fn key(p: &(f64, f64), cell: f64) -> (i64, i64) {
        ((p.0 / cell).floor() as i64, (p.1 / cell).floor() as i64)
    }
    fn ring(&self, centre: (i64, i64), r: i64) -> impl Iterator<Item = usize> + '_ {
        (-r..=r)
            .flat_map(move |dx| {
                (-r..=r)
                    .filter(move |dy| dx.abs() == r || dy.abs() == r)
                    .filter_map(move |dy| self.cells.get(&(centre.0 + dx, centre.1 + dy)))
            })
            .flatten()
            .copied()
    }
}

fn grid_cell(points: &[(f64, f64)]) -> f64 {
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in points {
        x0 = x0.min(p.0);
        x1 = x1.max(p.0);
        y0 = y0.min(p.1);
        y1 = y1.max(p.1);
    }
    let area = ((x1 - x0) * (y1 - y0)).max(0.0);
    let cell = (area / points.len().max(1) as f64).sqrt();
    if cell.is_finite() && cell > 0.0 {
        cell
    } else {
        ((x1 - x0).max(y1 - y0) / points.len().max(1) as f64).max(1.0)
    }
}

/// Distance from each point to its nearest other point (infinite when alone), by expanding rings
/// on a grid with about one point per cell: near linear for spread-out points.
fn nearest_neighbour_distances(points: &[(f64, f64)]) -> Vec<f64> {
    if points.len() < 2 {
        return vec![f64::INFINITY; points.len()];
    }
    let cell = grid_cell(points);
    let grid = PointGrid::new(points, cell);
    let max_ring = grid
        .cells
        .keys()
        .map(|k| k.0.abs().max(k.1.abs()))
        .max()
        .unwrap_or(0)
        * 2
        + 2;
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let centre = PointGrid::key(p, cell);
            let mut best = f64::INFINITY;
            let mut r = 0;
            // Once a ring is further than the best distance found, no further ring can beat it.
            while r <= max_ring && (r as f64 - 1.0) * cell <= best {
                for j in grid.ring(centre, r) {
                    if j != i {
                        let d = (points[j].0 - p.0).hypot(points[j].1 - p.1);
                        if d > 0.0 {
                            best = best.min(d);
                        }
                    }
                }
                r += 1;
            }
            best
        })
        .collect()
}

/// How many other points lie within `radius` of each point.
fn count_within(points: &[(f64, f64)], radius: f64) -> Vec<usize> {
    if points.is_empty() || radius.is_nan() || radius <= 0.0 {
        return vec![0; points.len()];
    }
    let grid = PointGrid::new(points, radius);
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let centre = PointGrid::key(p, radius);
            (0..=1)
                .flat_map(|r| grid.ring(centre, r).collect::<Vec<_>>())
                .filter(|&j| j != i && (points[j].0 - p.0).hypot(points[j].1 - p.1) <= radius)
                .count()
        })
        .collect()
}

/// Placed boxes bucketed on a grid, for overlap tests against a candidate.
struct BoxGrid {
    cell: f64,
    boxes: Vec<[f64; 4]>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl BoxGrid {
    fn new(cell: f64) -> Self {
        Self {
            cell,
            boxes: Vec::new(),
            cells: HashMap::new(),
        }
    }
    fn span(&self, b: &[f64; 4]) -> (i64, i64, i64, i64) {
        (
            (b[0] / self.cell).floor() as i64,
            (b[1] / self.cell).floor() as i64,
            (b[2] / self.cell).floor() as i64,
            (b[3] / self.cell).floor() as i64,
        )
    }
    fn insert(&mut self, b: [f64; 4]) {
        let id = self.boxes.len();
        let (x0, y0, x1, y1) = self.span(&b);
        for x in x0..=x1 {
            for y in y0..=y1 {
                self.cells.entry((x, y)).or_default().push(id);
            }
        }
        self.boxes.push(b);
    }
    fn overlaps(&self, b: &[f64; 4]) -> bool {
        let (x0, y0, x1, y1) = self.span(b);
        for x in x0..=x1 {
            for y in y0..=y1 {
                if let Some(ids) = self.cells.get(&(x, y)) {
                    for &id in ids {
                        let o = &self.boxes[id];
                        if b[0] < o[2] && o[0] < b[2] && b[1] < o[3] && o[1] < b[3] {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// Visual quality of a layout (Birchfield & Overbye §V), for tests and evidence.
#[derive(Clone, Debug, Serialize)]
pub struct TxLayoutQuality {
    /// Distance from seed to drawn centre per substation, in drawing units.
    pub displacement: Vec<f64>,
    /// Centre distance to the nearest other substation divided by the shorter side of the box.
    pub spacing: Vec<f64>,
    /// Pairs of substation boxes that overlap.
    pub overlapping_pairs: usize,
    /// Line segments that cross a substation box other than their own ends'.
    pub lines_through_boxes: usize,
    /// Per line between substations: drawn length over the straight distance between its ends.
    pub relative_length: Vec<f64>,
    /// Per line: turns of more than one degree, and of more than sixty.
    pub bends: Vec<usize>,
    pub sharp_bends: Vec<usize>,
}

pub fn tx_layout_quality(input: &TxLayoutInput, layout: &TxLayout) -> TxLayoutQuality {
    let k = layout.boxes.len() / 4;
    let n = input.bus_substation.len();
    let mut seed = vec![(0.0, 0.0, 0usize); k];
    for i in 0..n {
        let s = input.bus_substation[i] as usize;
        seed[s].0 += input.xy[i * 2];
        seed[s].1 += input.xy[i * 2 + 1];
        seed[s].2 += 1;
    }
    let occupied: Vec<usize> = (0..k).filter(|&s| seed[s].2 > 0).collect();
    let b = |s: usize| &layout.boxes[s * 4..s * 4 + 4];
    let centre = |s: usize| ((b(s)[0] + b(s)[2]) / 2.0, (b(s)[1] + b(s)[3]) / 2.0);
    let displacement = occupied
        .iter()
        .map(|&s| {
            let (x, y) = (seed[s].0 / seed[s].2 as f64, seed[s].1 / seed[s].2 as f64);
            let c = centre(s);
            (c.0 - x).hypot(c.1 - y)
        })
        .collect();
    let centres: Vec<(f64, f64)> = occupied.iter().map(|&s| centre(s)).collect();
    let nn = nearest_neighbour_distances(&centres);
    let spacing = occupied
        .iter()
        .zip(&nn)
        .map(|(&s, d)| {
            d / (b(s)[2] - b(s)[0])
                .min(b(s)[3] - b(s)[1])
                .max(f64::MIN_POSITIVE)
        })
        .collect();
    let mut grid = BoxGrid::new(
        occupied
            .iter()
            .map(|&s| (b(s)[2] - b(s)[0]).max(b(s)[3] - b(s)[1]))
            .fold(0.0, f64::max)
            .max(f64::MIN_POSITIVE),
    );
    let mut overlapping_pairs = 0;
    for &s in &occupied {
        let bb = [b(s)[0], b(s)[1], b(s)[2], b(s)[3]];
        // Count against boxes already inserted: each pair once. Touching edges do not count.
        let shrunk = [bb[0] + 1e-9, bb[1] + 1e-9, bb[2] - 1e-9, bb[3] - 1e-9];
        if grid.overlaps(&shrunk) {
            overlapping_pairs += 1;
        }
        grid.insert(bb);
    }
    let sub_of_row = |row: i64| (row >= 0).then(|| input.bus_substation[row as usize] as usize);
    let index = BoxIndex::new(&layout.boxes, &occupied);
    let mut lines_through_boxes = 0;
    for (j, &e) in layout.segment_edge.iter().enumerate() {
        if layout.role[e as usize] != TX_ROLE_LINE {
            continue;
        }
        let own = [
            sub_of_row(input.from[e as usize]).unwrap_or(usize::MAX),
            sub_of_row(input.to[e as usize]).unwrap_or(usize::MAX),
        ];
        let seg = &layout.segments[j * 4..j * 4 + 4];
        if index
            .first_hit((seg[0], seg[1]), (seg[2], seg[3]), own)
            .is_some()
        {
            lines_through_boxes += 1;
        }
    }
    // Polylines per line, from consecutive segments of the same branch.
    let mut relative_length = Vec::new();
    let mut bends = Vec::new();
    let mut sharp_bends = Vec::new();
    let mut j = 0;
    let segs = layout.segment_edge.len();
    while j < segs {
        let e = layout.segment_edge[j];
        let start = j;
        while j < segs && layout.segment_edge[j] == e {
            j += 1;
        }
        if layout.role[e as usize] != TX_ROLE_LINE {
            continue;
        }
        let seg = |i: usize| &layout.segments[i * 4..i * 4 + 4];
        let drawn: f64 = (start..j)
            .map(|i| (seg(i)[2] - seg(i)[0]).hypot(seg(i)[3] - seg(i)[1]))
            .sum();
        let (a, b) = (seg(start), seg(j - 1));
        let straight = (b[2] - a[0]).hypot(b[3] - a[1]);
        relative_length.push(if straight > 0.0 {
            drawn / straight
        } else {
            1.0
        });
        let (mut turns, mut sharp) = (0, 0);
        for i in start + 1..j {
            let (p, q) = (seg(i - 1), seg(i));
            let a1 = (p[3] - p[1]).atan2(p[2] - p[0]);
            let a2 = (q[3] - q[1]).atan2(q[2] - q[0]);
            let mut d = (a2 - a1).abs().to_degrees();
            if d > 180.0 {
                d = 360.0 - d;
            }
            if d > 1.0 {
                turns += 1;
            }
            if d > 60.0 {
                sharp += 1;
            }
        }
        bends.push(turns);
        sharp_bends.push(sharp);
    }
    TxLayoutQuality {
        displacement,
        spacing,
        overlapping_pairs,
        lines_through_boxes,
        relative_length,
        bends,
        sharp_bends,
    }
}

/// Liang–Barsky: whether segment `s` (`x0, y0, x1, y1`) meets box `b`.
fn segment_hits_box(s: &[f64], b: &[f64]) -> bool {
    let (dx, dy) = (s[2] - s[0], s[3] - s[1]);
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, s[0] - b[0]),
        (dx, b[2] - s[0]),
        (-dy, s[1] - b[1]),
        (dy, b[3] - s[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Substations on a jittered grid, `per` buses each (two voltage levels joined by a
    /// transformer), neighbours joined by lines.
    fn grid_case(side: usize, per: usize, gap: f64) -> TxLayoutInput {
        let mut input = TxLayoutInput::default();
        let mut row = 0i64;
        let mut first = Vec::new();
        for gy in 0..side {
            for gx in 0..side {
                let s = (gy * side + gx) as u32;
                let jitter = ((s as f64 * 12.9898).sin() * 43758.5453).fract() * 0.2 * gap;
                first.push(row);
                for b in 0..per {
                    input.bus_substation.push(s);
                    input.kv.push(if b == 0 { 345.0 } else { 138.0 });
                    input
                        .xy
                        .extend_from_slice(&[gx as f64 * gap + jitter, gy as f64 * gap - jitter]);
                    if b > 0 {
                        input.from.push(row - b as i64);
                        input.to.push(row);
                        input.in_service.push(1);
                    }
                    row += 1;
                }
            }
        }
        for gy in 0..side {
            for gx in 0..side {
                let s = gy * side + gx;
                if gx + 1 < side {
                    input.from.push(first[s]);
                    input.to.push(first[s + 1]);
                    input.in_service.push(1);
                }
                if gy + 1 < side {
                    input.from.push(first[s] + per as i64 - 1);
                    input.to.push(first[s + side] + per as i64 - 1);
                    input.in_service.push(1);
                }
            }
        }
        input
    }

    #[test]
    fn sparse_substations_stay_put_without_overlap() {
        let input = grid_case(6, 2, 10_000.0);
        let layout = tx_schematic_layout(&input, &TxLayoutOptions::default());
        let q = tx_layout_quality(&input, &layout);
        assert_eq!(q.overlapping_pairs, 0);
        // Boxes are under half the spacing: every substation takes its own seed.
        assert!(
            q.displacement.iter().all(|d| *d < 1e-6),
            "{:?}",
            q.displacement
        );
        assert!(q.spacing.iter().all(|s| *s >= 1.0));
    }

    #[test]
    fn crowded_seeds_spread_apart_and_stay_near() {
        // Nine substations on one point: the greedy search spreads them without overlap.
        let mut input = grid_case(3, 2, 10_000.0);
        let mut far = grid_case(3, 2, 10_000.0);
        for v in input.xy.iter_mut() {
            *v = 0.0;
        }
        // A sparse ring of neighbours sets the scale.
        let offset = input.bus_substation.len() as i64;
        for v in far.xy.iter_mut() {
            *v = *v * 5.0 + 200_000.0;
        }
        input
            .bus_substation
            .extend(far.bus_substation.iter().map(|s| s + 9));
        input.kv.extend(far.kv);
        input.xy.extend(far.xy);
        input.from.extend(far.from.iter().map(|r| r + offset));
        input.to.extend(far.to.iter().map(|r| r + offset));
        input.in_service.extend(far.in_service);
        let layout = tx_schematic_layout(&input, &TxLayoutOptions::default());
        let q = tx_layout_quality(&input, &layout);
        assert_eq!(q.overlapping_pairs, 0);
        let worst = q.displacement[..9].iter().fold(0.0f64, |a, &b| a.max(b));
        assert!(worst > 0.0 && worst < 200_000.0, "{worst}");
    }

    #[test]
    fn busbars_order_by_voltage_and_transformers_run_inside() {
        let input = grid_case(2, 3, 10_000.0);
        let layout = tx_schematic_layout(&input, &TxLayoutOptions::default());
        // Bus 0 (345 kV) is the left bar of its substation; buses 1 and 2 (138 kV) to its right.
        assert!(layout.positions[0] < layout.positions[2]);
        assert!(layout.positions[2] < layout.positions[4]);
        // The first two branches are transformers inside substation 0.
        assert_eq!(layout.role[0], TX_ROLE_INTERNAL);
        assert_eq!(layout.role[1], TX_ROLE_INTERNAL);
        let lines = layout.role.iter().filter(|&&r| r == TX_ROLE_LINE).count();
        assert_eq!(lines, 4);
        // Every bus sits inside its substation's box.
        for i in 0..input.bus_substation.len() {
            let s = input.bus_substation[i] as usize;
            let b = &layout.boxes[s * 4..s * 4 + 4];
            let (x, y) = (layout.positions[i * 2], layout.positions[i * 2 + 1]);
            assert!(x > b[0] && x < b[2] && y > b[1] && y < b[3]);
        }
    }

    #[test]
    fn out_of_service_branches_are_drawn_open() {
        let mut input = grid_case(2, 1, 10_000.0);
        input.in_service[0] = 0;
        let layout = tx_schematic_layout(&input, &TxLayoutOptions::default());
        assert_eq!(layout.role[0], TX_ROLE_OPEN);
        assert_eq!(layout.role[1], TX_ROLE_LINE);
    }

    #[test]
    fn layout_is_deterministic() {
        let input = grid_case(5, 2, 7_000.0);
        let a = tx_schematic_layout(&input, &TxLayoutOptions::default());
        let b = tx_schematic_layout(&input, &TxLayoutOptions::default());
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.segments, b.segments);
    }

    #[test]
    fn nearest_neighbours_match_brute_force() {
        let pts: Vec<(f64, f64)> = (0..300)
            .map(|i| {
                let t = i as f64;
                ((t * 7.31).sin() * 1000.0, (t * 3.17).cos() * 700.0)
            })
            .collect();
        let fast = nearest_neighbour_distances(&pts);
        for (i, p) in pts.iter().enumerate() {
            let slow = pts
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, q)| (q.0 - p.0).hypot(q.1 - p.1))
                .fold(f64::INFINITY, f64::min);
            assert!((fast[i] - slow).abs() < 1e-9, "{i}: {} vs {slow}", fast[i]);
        }
    }

    #[test]
    fn routed_lines_step_around_substations_in_their_way() {
        // A, B and C in a row (with D and E off the line so the triangulation has area); the
        // line A–C would run straight through B.
        let seeds = [
            (0.0, 0.0),
            (10_000.0, 0.0),
            (20_000.0, 0.0),
            (10_000.0, 12_000.0),
            (10_000.0, -12_000.0),
        ];
        let mut input = TxLayoutInput::default();
        for (s, &(x, y)) in seeds.iter().enumerate() {
            input.bus_substation.push(s as u32);
            input.kv.push(230.0);
            input.xy.extend_from_slice(&[x, y]);
        }
        for (a, b) in [(0, 2), (0, 1), (1, 2), (0, 3), (3, 2), (0, 4), (4, 2)] {
            input.from.push(a);
            input.to.push(b);
            input.in_service.push(1);
        }
        let straight = tx_schematic_layout(
            &input,
            &TxLayoutOptions {
                route_iterations: 0,
                ..TxLayoutOptions::default()
            },
        );
        assert!(tx_layout_quality(&input, &straight).lines_through_boxes > 0);
        let routed = tx_schematic_layout(&input, &TxLayoutOptions::default());
        let q = tx_layout_quality(&input, &routed);
        assert_eq!(q.lines_through_boxes, 0);
        assert!(
            q.relative_length.iter().all(|r| *r < 2.0),
            "{:?}",
            q.relative_length
        );
    }

    #[test]
    fn segments_meet_boxes_only_when_they_cross() {
        let b = [0.0, 0.0, 1.0, 1.0];
        assert!(segment_hits_box(&[-1.0, 0.5, 2.0, 0.5], &b));
        assert!(!segment_hits_box(&[-1.0, 2.0, 2.0, 2.0], &b));
        assert!(!segment_hits_box(&[-1.0, -0.1, 0.5, -2.0], &b));
    }
}
