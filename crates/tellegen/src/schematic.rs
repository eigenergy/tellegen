//! Schematic layouts for multiconductor graphs.
//!
//! A schematic is a drawing, not an electrical claim. Buses are placed on a breadth-first spanning
//! forest over closed branches, rooted at source buses, so depth reads as branch steps from a
//! source. Tree branches route as right-angled elbows; branches that close a loop and open or
//! unreachable branches are drawn straight so they cannot be mistaken for tree routes. The forest
//! matches the browser's display tree (`displayTree` in `@tellegen/svelte`) bus for bus, so the
//! drawing and the voltage profile always follow the same path.

use std::collections::{HashMap, VecDeque};

use powerio_dist::DistGraph;
#[cfg(test)]
use powerio_dist::DistGraphAttachment;
use serde::{Deserialize, Serialize};

/// Bump when the geometry for the same graph and options changes.
pub const SCHEMATIC_LAYOUT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SchematicStrategy {
    /// Every leaf on its own row; simple and stable under edits.
    LeafOrder,
    /// Subtrees packed as close as their per-depth contours allow; much more compact.
    #[default]
    Tidy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SchematicOptions {
    pub strategy: SchematicStrategy,
    /// Horizontal distance per branch step.
    pub dx: f64,
    /// Minimum vertical distance between rows.
    pub dy: f64,
    /// Vertical space reserved below a bus per kind of attached equipment (load, generator,
    /// IBR, shunt, source), so attachment symbols never overlap the next row. Zero disables.
    pub attachment_gap: f64,
}

impl Default for SchematicOptions {
    fn default() -> Self {
        Self {
            strategy: SchematicStrategy::Tidy,
            dx: 120.0,
            dy: 28.0,
            attachment_gap: 22.0,
        }
    }
}

/// Edge roles in [`SchematicLayout::role`].
pub const ROLE_TREE: u8 = 0;
pub const ROLE_LOOP: u8 = 1;
pub const ROLE_OPEN: u8 = 2;

/// Columnar layout output in graph order, sized for transfer to the browser.
#[derive(Clone, Debug, Serialize)]
pub struct SchematicLayout {
    pub version: u32,
    pub strategy: SchematicStrategy,
    /// `x, y` per bus; y grows downwards.
    pub positions: Vec<f64>,
    /// `x0, y0, x1, y1` per drawn segment; a routed edge has several.
    pub segments: Vec<f64>,
    /// Edge index for each segment.
    pub segment_edge: Vec<u32>,
    /// Per edge: [`ROLE_TREE`], [`ROLE_LOOP`] or [`ROLE_OPEN`].
    pub role: Vec<u8>,
    /// Display forest: parent bus (a root is its own parent), parent edge (-1 for roots), depth.
    pub parent: Vec<u32>,
    pub parent_edge: Vec<i64>,
    pub depth: Vec<u32>,
    pub roots: Vec<u32>,
    pub bounds: [f64; 4],
    /// Echo of [`SchematicOptions::attachment_gap`]; attachment symbol `i` of a bus sits
    /// `(i + 1) * attachment_gap` below it.
    pub attachment_gap: f64,
    /// Reserved symbol slots per bus: one per distinct attachment kind, plus one when the bus has
    /// perfectly grounded terminals.
    pub attachment_slots: Vec<u8>,
}

struct Forest {
    parent: Vec<usize>,
    parent_edge: Vec<Option<usize>>,
    depth: Vec<usize>,
    roots: Vec<usize>,
    children: Vec<Vec<usize>>,
    ends: Vec<Option<(usize, usize)>>,
}

/// Breadth-first forest rooted at source buses in graph order, then at any bus they cannot reach.
/// Adjacency follows edge order and ids match case-insensitively, exactly as in the browser.
fn forest(graph: &DistGraph) -> Forest {
    let n = graph.buses.len();
    let index: HashMap<String, usize> = graph
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.to_lowercase(), i))
        .collect();
    let ends: Vec<Option<(usize, usize)>> = graph
        .edges
        .iter()
        .map(|e| {
            Some((
                *index.get(&e.from.to_lowercase())?,
                *index.get(&e.to.to_lowercase())?,
            ))
        })
        .collect();
    let mut adjacency: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (k, e) in graph.edges.iter().enumerate() {
        if let (true, Some((a, b))) = (e.closed, ends[k]) {
            if a != b {
                adjacency[a].push((b, k));
                adjacency[b].push((a, k));
            }
        }
    }
    let mut parent = vec![usize::MAX; n];
    let mut parent_edge = vec![None; n];
    let mut depth = vec![usize::MAX; n];
    let mut roots = Vec::new();
    let mut children = vec![Vec::new(); n];
    let mut grow = |root: usize, parent: &mut Vec<usize>, depth: &mut Vec<usize>| {
        roots.push(root);
        parent[root] = root;
        depth[root] = 0;
        let mut queue = VecDeque::from([root]);
        while let Some(u) = queue.pop_front() {
            for &(v, k) in &adjacency[u] {
                if depth[v] != usize::MAX {
                    continue;
                }
                parent[v] = u;
                parent_edge[v] = Some(k);
                depth[v] = depth[u] + 1;
                children[u].push(v);
                queue.push_back(v);
            }
        }
    };
    for (i, bus) in graph.buses.iter().enumerate() {
        if bus.has_source && depth[i] == usize::MAX {
            grow(i, &mut parent, &mut depth);
        }
    }
    for i in 0..n {
        if depth[i] == usize::MAX {
            grow(i, &mut parent, &mut depth);
        }
    }
    Forest {
        parent,
        parent_edge,
        depth,
        roots,
        children,
        ends,
    }
}

/// A subtree's vertical extent per depth, stored relative to `offset` so shifting is O(1).
struct Contour {
    rows: VecDeque<(f64, f64)>,
    offset: f64,
}

impl Contour {
    fn get(&self, i: usize) -> (f64, f64) {
        let (lo, hi) = self.rows[i];
        (lo + self.offset, hi + self.offset)
    }
    /// Merge `other` (already shifted) into the longer of the two; cost is the shorter length.
    fn merge(mut self, mut other: Contour) -> Contour {
        if other.rows.len() > self.rows.len() {
            std::mem::swap(&mut self, &mut other);
        }
        for i in 0..other.rows.len() {
            let (a_lo, a_hi) = self.get(i);
            let (b_lo, b_hi) = other.get(i);
            self.rows[i] = (a_lo.min(b_lo) - self.offset, a_hi.max(b_hi) - self.offset);
        }
        self
    }
}

/// Vertical positions: `y` of each bus within its subtree's frame plus each child's frame shift.
fn rows(forest: &Forest, strategy: SchematicStrategy, dy: f64, extent: &[f64]) -> Vec<f64> {
    let n = forest.parent.len();
    let mut local = vec![0.0; n];
    let mut shift = vec![0.0; n];
    let mut contours: Vec<Option<Contour>> = (0..n).map(|_| None).collect();
    let mut next_leaf = 0.0;
    let mut component_shift = Vec::with_capacity(forest.roots.len());
    let mut bottom = f64::NEG_INFINITY;
    for &root in &forest.roots {
        // Iterative post-order: deep feeders must not overflow the stack.
        let mut stack = vec![(root, 0usize)];
        while let Some(&mut (u, ref mut next)) = stack.last_mut() {
            if *next < forest.children[u].len() {
                let child = forest.children[u][*next];
                *next += 1;
                stack.push((child, 0));
                continue;
            }
            stack.pop();
            let kids = &forest.children[u];
            match strategy {
                SchematicStrategy::LeafOrder => {
                    if kids.is_empty() {
                        local[u] = next_leaf;
                    } else {
                        let (first, last) = (kids[0], kids[kids.len() - 1]);
                        local[u] = (local[first] + local[last]) / 2.0;
                    }
                    // The next row starts below this bus and its attachment symbols.
                    next_leaf = f64::max(next_leaf, local[u] + extent[u] + dy);
                }
                SchematicStrategy::Tidy => {
                    let mut merged: Option<Contour> = None;
                    for &child in kids {
                        let mut contour = contours[child].take().expect("child contour");
                        if let Some(acc) = &merged {
                            let overlap = acc.rows.len().min(contour.rows.len());
                            let mut s = f64::NEG_INFINITY;
                            for i in 0..overlap {
                                s = s.max(acc.get(i).1 - contour.get(i).0 + dy);
                            }
                            shift[child] = s.max(0.0);
                            contour.offset += shift[child];
                        }
                        merged = Some(match merged {
                            None => contour,
                            Some(acc) => acc.merge(contour),
                        });
                    }
                    if let (Some(&first), Some(&last)) = (kids.first(), kids.last()) {
                        local[u] = (local[first] + shift[first] + local[last] + shift[last]) / 2.0;
                    }
                    let mut contour = merged.unwrap_or(Contour {
                        rows: VecDeque::new(),
                        offset: 0.0,
                    });
                    contour.rows.push_front((
                        local[u] - contour.offset,
                        local[u] + extent[u] - contour.offset,
                    ));
                    contours[u] = Some(contour);
                }
            }
        }
        // Components stack top to bottom with a blank row between them.
        match strategy {
            SchematicStrategy::LeafOrder => {
                component_shift.push(0.0);
                next_leaf += dy;
            }
            SchematicStrategy::Tidy => {
                let contour = contours[root].take().expect("root contour");
                let top = (0..contour.rows.len())
                    .map(|i| contour.get(i).0)
                    .fold(f64::INFINITY, f64::min);
                let low = (0..contour.rows.len())
                    .map(|i| contour.get(i).1)
                    .fold(f64::NEG_INFINITY, f64::max);
                let place = if bottom.is_finite() {
                    bottom + 2.0 * dy - top
                } else {
                    -top
                };
                component_shift.push(place);
                bottom = low + place;
            }
        }
    }
    // Resolve frames top-down: a child's frame is its parent's frame plus the child's shift.
    let mut frame = vec![0.0; n];
    let mut y = vec![0.0; n];
    for (c, &root) in forest.roots.iter().enumerate() {
        frame[root] = component_shift[c];
        let mut queue = VecDeque::from([root]);
        while let Some(u) = queue.pop_front() {
            y[u] = frame[u] + local[u];
            for &child in &forest.children[u] {
                frame[child] = frame[u] + shift[child];
                queue.push_back(child);
            }
        }
    }
    y
}

pub fn schematic_layout(graph: &DistGraph, options: &SchematicOptions) -> SchematicLayout {
    let forest = forest(graph);
    // One reserved slot per distinct attachment kind; each kind is drawn once with its count.
    let attachment_slots: Vec<u8> = graph
        .buses
        .iter()
        .map(|bus| {
            let mut kinds: Vec<_> = bus
                .terminal_attachments
                .values()
                .flatten()
                .map(|a| a.kind)
                .collect();
            kinds.sort_by_key(|k| format!("{k:?}"));
            kinds.dedup();
            // Perfectly grounded terminals draw one solid-earth symbol in a slot of its own.
            kinds.len() as u8 + u8::from(!bus.grounded.is_empty())
        })
        .collect();
    let extent: Vec<f64> = attachment_slots
        .iter()
        .map(|&k| f64::from(k) * options.attachment_gap)
        .collect();
    let y = rows(&forest, options.strategy, options.dy, &extent);
    let n = graph.buses.len();
    let mut positions = Vec::with_capacity(n * 2);
    for (depth, row) in forest.depth.iter().zip(&y) {
        positions.push(*depth as f64 * options.dx);
        positions.push(*row);
    }
    let at = |i: usize| (positions[i * 2], positions[i * 2 + 1]);
    let mut role = Vec::with_capacity(graph.edges.len());
    let mut segments = Vec::new();
    let mut segment_edge = Vec::new();
    for (k, e) in graph.edges.iter().enumerate() {
        let Some((a, b)) = forest.ends[k] else {
            role.push(ROLE_OPEN);
            continue;
        };
        let is_tree = forest.parent_edge[a] == Some(k) || forest.parent_edge[b] == Some(k);
        role.push(if !e.closed {
            ROLE_OPEN
        } else if is_tree {
            ROLE_TREE
        } else {
            ROLE_LOOP
        });
        let points: Vec<(f64, f64)> = if is_tree && e.closed {
            let (p, c) = if forest.parent[b] == a {
                (a, b)
            } else {
                (b, a)
            };
            let ((xp, yp), (xc, yc)) = (at(p), at(c));
            let xm = xp + options.dx / 2.0;
            if yp == yc {
                vec![(xp, yp), (xc, yc)]
            } else {
                vec![(xp, yp), (xm, yp), (xm, yc), (xc, yc)]
            }
        } else {
            vec![at(a), at(b)]
        };
        for pair in points.windows(2) {
            segments.extend([pair[0].0, pair[0].1, pair[1].0, pair[1].1]);
            segment_edge.push(k as u32);
        }
    }
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (i, below) in extent.iter().enumerate() {
        let (x, y) = at(i);
        bounds = [
            bounds[0].min(x),
            bounds[1].min(y),
            bounds[2].max(x),
            bounds[3].max(y + below),
        ];
    }
    if n == 0 {
        bounds = [0.0, 0.0, 1.0, 1.0];
    }
    SchematicLayout {
        version: SCHEMATIC_LAYOUT_VERSION,
        strategy: options.strategy,
        positions,
        segments,
        segment_edge,
        role,
        parent: forest.parent.iter().map(|&p| p as u32).collect(),
        parent_edge: forest
            .parent_edge
            .iter()
            .map(|e| e.map_or(-1, |k| k as i64))
            .collect(),
        depth: forest.depth.iter().map(|&d| d as u32).collect(),
        roots: forest.roots.iter().map(|&r| r as u32).collect(),
        bounds,
        attachment_gap: options.attachment_gap,
        attachment_slots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(buses: &[(&str, bool)], edges: &[(&str, &str, &str, &str, bool)]) -> DistGraph {
        serde_json::from_value(serde_json::json!({
            "buses": buses.iter().map(|(id, source)| serde_json::json!({
                "id": id, "terminals": ["1", "2", "3", "n"], "grounded": [],
                "load_kw": 0.0, "gen_kw": 0.0, "has_source": source
            })).collect::<Vec<_>>(),
            "edges": edges.iter().map(|(kind, id, from, to, closed)| serde_json::json!({
                "kind": kind, "id": id, "from": from, "to": to,
                "conductors": [], "closed": closed, "n_phases": 3
            })).collect::<Vec<_>>()
        }))
        .expect("graph JSON")
    }

    /// The same fixture as `tests/diagram-profile.test.ts` in `@tellegen/svelte`.
    fn feeder() -> DistGraph {
        graph(
            &[
                ("S", true),
                ("M", false),
                ("L1", false),
                ("L2", false),
                ("X", false),
                ("Y", false),
            ],
            &[
                ("line", "e1", "S", "M", true),
                ("transformer", "t1", "M", "L1", true),
                ("line", "e2", "l1", "L2", true),
                ("switch", "sw", "L2", "X", false),
            ],
        )
    }

    #[test]
    fn forest_matches_the_browser_display_tree() {
        let layout = schematic_layout(&feeder(), &SchematicOptions::default());
        assert_eq!(layout.roots, vec![0, 4, 5]);
        assert_eq!(layout.depth, vec![0, 1, 2, 3, 0, 0]);
        assert_eq!(layout.parent, vec![0, 0, 1, 2, 4, 5]);
        assert_eq!(layout.parent_edge, vec![-1, 0, 1, 2, -1, -1]);
        assert_eq!(
            layout.role,
            vec![ROLE_TREE, ROLE_TREE, ROLE_TREE, ROLE_OPEN]
        );
    }

    /// The browser's `displayTree` reads the same fixture and must reproduce the same forest
    /// (`tests/display-forest.test.ts` in `@tellegen/svelte`).
    #[test]
    fn forest_matches_the_shared_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/data/schematic/display-forest.json"))
                .expect("fixture JSON");
        let graph: DistGraph = serde_json::from_value(fixture["graph"].clone()).expect("graph");
        let layout = schematic_layout(&graph, &SchematicOptions::default());
        let forest = &fixture["forest"];
        let ints = |key: &str| -> Vec<i64> {
            forest[key]
                .as_array()
                .expect(key)
                .iter()
                .map(|v| v.as_i64().expect(key))
                .collect()
        };
        let widen = |v: &[u32]| v.iter().map(|&x| i64::from(x)).collect::<Vec<_>>();
        assert_eq!(widen(&layout.parent), ints("parent"));
        assert_eq!(layout.parent_edge, ints("parent_edge"));
        assert_eq!(widen(&layout.depth), ints("depth"));
        assert_eq!(widen(&layout.roots), ints("roots"));
    }

    #[test]
    fn leaf_order_matches_the_browser_layout() {
        let options = SchematicOptions {
            strategy: SchematicStrategy::LeafOrder,
            dx: 100.0,
            dy: 20.0,
            ..SchematicOptions::default()
        };
        let layout = schematic_layout(&feeder(), &options);
        assert_eq!(
            layout.positions,
            vec![0.0, 0.0, 100.0, 0.0, 200.0, 0.0, 300.0, 0.0, 0.0, 40.0, 0.0, 80.0]
        );
        assert_eq!(layout.bounds, [0.0, 0.0, 300.0, 80.0]);
        assert_eq!(layout.segment_edge.iter().filter(|&&k| k == 3).count(), 1);
    }

    #[test]
    fn tidy_packs_subtrees_without_overlap_and_centres_parents() {
        // A shallow leaf `a` beside a deeper subtree `b`: tidy lets `b` sit half a row from `a`
        // because they only share depth 1, while leaf order spends a full row per leaf.
        let g = graph(
            &[
                ("r", true),
                ("a", false),
                ("b", false),
                ("b1", false),
                ("b2", false),
                ("b3", false),
            ],
            &[
                ("line", "1", "r", "a", true),
                ("line", "2", "r", "b", true),
                ("line", "3", "b", "b1", true),
                ("line", "4", "b1", "b2", true),
                ("line", "5", "b1", "b3", true),
            ],
        );
        let options = SchematicOptions {
            strategy: SchematicStrategy::Tidy,
            dx: 10.0,
            dy: 1.0,
            ..SchematicOptions::default()
        };
        let layout = schematic_layout(&g, &options);
        let y: Vec<f64> = (0..6).map(|i| layout.positions[i * 2 + 1]).collect();
        assert_eq!(y, vec![0.5, 0.0, 1.0, 1.0, 0.5, 1.5]);
        assert_eq!(layout.bounds[3], 1.5);
        let leaf = SchematicOptions {
            strategy: SchematicStrategy::LeafOrder,
            ..options
        };
        assert_eq!(schematic_layout(&g, &leaf).bounds[3], 2.0);
    }

    /// Attaches equipment `(bus, kind, id)` to terminal 1 of the named buses.
    fn attach(mut g: DistGraph, items: &[(&str, &str, &str)]) -> DistGraph {
        for (bus, kind, id) in items {
            let b = g.buses.iter_mut().find(|b| b.id == *bus).expect("bus");
            let a: DistGraphAttachment =
                serde_json::from_value(serde_json::json!({ "kind": kind, "id": id }))
                    .expect("attachment JSON");
            b.terminal_attachments
                .entry("1".into())
                .or_default()
                .push(a);
        }
        g
    }

    #[test]
    fn attachments_reserve_room_below_their_bus_in_both_strategies() {
        // L2 carries a load and a generator (two slots); X carries two loads (one slot, the
        // symbol shows the count). Every row below moves down by the reserved room.
        let g = attach(
            feeder(),
            &[
                ("L2", "load", "ld1"),
                ("L2", "generator", "g1"),
                ("X", "load", "ld2"),
                ("X", "load", "ld3"),
            ],
        );
        for strategy in [SchematicStrategy::LeafOrder, SchematicStrategy::Tidy] {
            let options = SchematicOptions {
                strategy,
                dx: 100.0,
                dy: 20.0,
                attachment_gap: 10.0,
            };
            let bare = schematic_layout(&feeder(), &options);
            let layout = schematic_layout(&g, &options);
            assert_eq!(layout.attachment_slots, vec![0, 0, 0, 2, 1, 0]);
            assert_eq!(layout.attachment_gap, 10.0);
            let y = |l: &SchematicLayout, i: usize| l.positions[i * 2 + 1];
            assert_eq!(y(&layout, 4) - y(&bare, 4), 20.0, "{strategy:?}");
            assert_eq!(y(&layout, 5) - y(&bare, 5), 30.0, "{strategy:?}");
            assert_eq!(layout.bounds[3], y(&layout, 5));
        }
        // Zero gap disables the reservation.
        let flat = SchematicOptions {
            attachment_gap: 0.0,
            ..SchematicOptions::default()
        };
        assert_eq!(
            schematic_layout(&g, &flat).positions,
            schematic_layout(&feeder(), &flat).positions
        );
    }

    #[test]
    fn perfectly_grounded_terminals_reserve_a_slot_of_their_own() {
        let mut g = attach(feeder(), &[("L2", "load", "ld1")]);
        let l2 = g.buses.iter_mut().find(|b| b.id == "L2").expect("bus");
        l2.grounded = vec!["n".into()];
        let options = SchematicOptions {
            strategy: SchematicStrategy::LeafOrder,
            dx: 100.0,
            dy: 20.0,
            attachment_gap: 10.0,
        };
        let layout = schematic_layout(&g, &options);
        assert_eq!(layout.attachment_slots, vec![0, 0, 0, 2, 0, 0]);
        let bare = schematic_layout(&feeder(), &options);
        assert_eq!(layout.positions[9] - bare.positions[9], 20.0);
    }

    #[test]
    fn tidy_is_linear_on_a_long_chain_and_handles_loops() {
        let n = 50_000;
        let names: Vec<String> = (0..n).map(|i| format!("b{i}")).collect();
        let buses: Vec<(&str, bool)> = names
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i == 0))
            .collect();
        let ids: Vec<String> = (1..n).map(|i| format!("e{i}")).collect();
        let mut edges: Vec<(&str, &str, &str, &str, bool)> = (1..n)
            .map(|i| {
                (
                    "line",
                    ids[i - 1].as_str(),
                    names[i - 1].as_str(),
                    names[i].as_str(),
                    true,
                )
            })
            .collect();
        edges.push(("line", "loop", "b0", "b10", true));
        let layout = schematic_layout(&graph(&buses, &edges), &SchematicOptions::default());
        // Breadth-first search takes the b0-b10 shortcut, so the far end is 9 steps closer. Fronts
        // from b1 and from b10 then meet at depth 5, so chain edge b5-b6 closes the loop.
        assert_eq!(layout.depth[n - 1] as usize, n - 1 - 9);
        assert_eq!(layout.role.iter().filter(|&&r| r == ROLE_LOOP).count(), 1);
        assert_eq!(layout.role[5], ROLE_LOOP);
        assert!(layout.positions.iter().all(|v| v.is_finite()));
    }
}
