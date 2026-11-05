//! Substation overview layout on real and synthetic cases: quality metrics (Birchfield & Overbye
//! 2018, §V) and timings. Opt-in and never gating: timings depend on the machine.
//!
//! ```sh
//! TX_EVAL_CASE=case_ACTIVSg200.m TX_EVAL_COORDS=ACTIVSg200-coords.csv \
//!   cargo test --release -p tellegen --test tx_schematic_eval -- --ignored --nocapture
//! ```
//!
//! `TX_EVAL_COORDS` is a CSV with `bus_i,Lat,Lon` columns (as the staged demo cases use).

use std::collections::BTreeMap;
use std::time::Instant;

use powerio_tx::IndexedNetwork;
use tellegen::geo::{spread_stacks, Coords};
use tellegen::substations::{group_substations, GroupingOptions};
use tellegen::tx_schematic::{
    tx_layout_quality, tx_schematic_layout, TxLayoutInput, TxLayoutOptions,
};

fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    const R: f64 = 6_378_137.0;
    let lat = lat.clamp(-85.0, 85.0);
    (
        R * lon.to_radians(),
        -R * (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0)
            .tan()
            .ln(),
    )
}

fn percentile(v: &[f64], p: f64) -> f64 {
    let mut v: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        return f64::NAN;
    }
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// Print one case's metrics, straight and routed, and return them for `TX_EVAL_OUT`.
fn report(name: &str, input: &TxLayoutInput, group_ms: f64) -> Vec<serde_json::Value> {
    let mut runs = Vec::new();
    for iterations in [0, 10] {
        let options = TxLayoutOptions {
            route_iterations: iterations,
            ..TxLayoutOptions::default()
        };
        let t = Instant::now();
        let layout = tx_schematic_layout(input, &options);
        let layout_ms = t.elapsed().as_secs_f64() * 1e3;
        let q = tx_layout_quality(input, &layout);
        let moved = q.displacement.iter().filter(|d| **d > 1e-6).count();
        let mean = |v: &[usize]| v.iter().sum::<usize>() as f64 / v.len().max(1) as f64;
        println!(
            "{name} ({}): {} buses, {} substations | grouping {group_ms:.1} ms, layout {layout_ms:.1} ms\n  \
             displacement (km): median {:.2}, p90 {:.2}, max {:.2}; moved {moved} ({:.0} %)\n  \
             spacing / box side: min {:.2}, p10 {:.2}; overlapping pairs {}\n  \
             line segments through other substations: {} ({} lines)\n  \
             relative length: median {:.2}, p90 {:.2}, max {:.2}; bends per line {:.2}, sharp {:.2}",
            if iterations == 0 { "straight" } else { "routed" },
            input.bus_substation.len(),
            q.displacement.len(),
            percentile(&q.displacement, 0.5) / 1e3,
            percentile(&q.displacement, 0.9) / 1e3,
            percentile(&q.displacement, 1.0) / 1e3,
            100.0 * moved as f64 / q.displacement.len().max(1) as f64,
            percentile(&q.spacing, 0.0),
            percentile(&q.spacing, 0.1),
            q.overlapping_pairs,
            q.lines_through_boxes,
            q.relative_length.len(),
            percentile(&q.relative_length, 0.5),
            percentile(&q.relative_length, 0.9),
            percentile(&q.relative_length, 1.0),
            mean(&q.bends),
            mean(&q.sharp_bends),
        );
        runs.push(serde_json::json!({
            "case": name,
            "routing": if iterations == 0 { "straight" } else { "routed" },
            "buses": input.bus_substation.len(),
            "substations": q.displacement.len(),
            "grouping_ms": group_ms,
            "layout_ms": layout_ms,
            "displacement_km": {
                "median": percentile(&q.displacement, 0.5) / 1e3,
                "p90": percentile(&q.displacement, 0.9) / 1e3,
                "max": percentile(&q.displacement, 1.0) / 1e3,
                "moved_share": moved as f64 / q.displacement.len().max(1) as f64,
            },
            "spacing_over_box_side": { "min": percentile(&q.spacing, 0.0), "p10": percentile(&q.spacing, 0.1) },
            "overlapping_pairs": q.overlapping_pairs,
            "line_segments_through_other_substations": q.lines_through_boxes,
            "lines": q.relative_length.len(),
            "relative_length": {
                "median": percentile(&q.relative_length, 0.5),
                "p90": percentile(&q.relative_length, 0.9),
                "max": percentile(&q.relative_length, 1.0),
            },
            "bends_per_line": mean(&q.bends),
            "sharp_bends_per_line": mean(&q.sharp_bends),
        }));
    }
    runs
}

fn write_out(runs: Vec<serde_json::Value>) {
    if let Ok(path) = std::env::var("TX_EVAL_OUT") {
        let doc = serde_json::json!({
            "generated_by": "cargo test --release -p tellegen --test tx_schematic_eval -- --ignored",
            "runs": runs,
        });
        std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n")
            .expect("write evaluation");
    }
}

#[test]
#[ignore]
fn evaluate_case_from_env() {
    let (Ok(case), Ok(coords)) = (
        std::env::var("TX_EVAL_CASE"),
        std::env::var("TX_EVAL_COORDS"),
    ) else {
        eprintln!("set TX_EVAL_CASE and TX_EVAL_COORDS");
        return;
    };
    let text = std::fs::read_to_string(&case).expect("read case");
    let source = powerio::Source::from_memory(&case, text.into_bytes()).expect("source");
    let options = powerio::ParseOptions::default()
        .format("m")
        .expect("format");
    let module = powerio::parse_with_options(source, &options).expect("parse case");
    let module = tellegen::ir::balanced_module(module).expect("balanced");
    let net = module.value();
    let mut located = Coords::new();
    let csv = std::fs::read_to_string(&coords).expect("read coords");
    let mut lines = csv.lines();
    let header: Vec<String> = lines
        .next()
        .unwrap()
        .split(',')
        .map(|h| h.trim().to_lowercase())
        .collect();
    let col = |names: &[&str]| {
        header
            .iter()
            .position(|h| names.contains(&h.as_str()))
            .unwrap()
    };
    let (bi, la, lo) = (
        col(&["bus_i", "bus"]),
        col(&["lat", "latitude"]),
        col(&["lon", "lng", "longitude"]),
    );
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let r: Vec<&str> = line.split(',').map(str::trim).collect();
        located.insert(
            r[bi].parse().unwrap(),
            (r[lo].parse().unwrap(), r[la].parse().unwrap()),
        );
    }
    let raw = located.clone();
    let stacks = spread_stacks(&mut located);
    let view = IndexedNetwork::new(net);
    let analysis = view.network();
    let t = Instant::now();
    let grouping = group_substations(
        analysis,
        &stacks,
        Some(&located),
        &GroupingOptions::default(),
    );
    let group_ms = t.elapsed().as_secs_f64() * 1e3;
    let rows: BTreeMap<usize, usize> = analysis
        .buses()
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.0, i))
        .collect();
    let mut input = TxLayoutInput {
        bus_substation: grouping.bus_substation.clone(),
        kv: analysis.buses().iter().map(|b| b.base_kv).collect(),
        ..Default::default()
    };
    for b in analysis.buses() {
        let (lon, lat) = raw.get(&b.id.0).copied().unwrap_or((0.0, 0.0));
        let (x, y) = mercator(lon, lat);
        input.xy.extend_from_slice(&[x, y]);
    }
    for br in analysis.branches() {
        input
            .from
            .push(rows.get(&br.from.0).map_or(-1, |&r| r as i64));
        input.to.push(rows.get(&br.to.0).map_or(-1, |&r| r as i64));
        input.in_service.push(u8::from(br.in_service));
    }
    if let Ok(path) = std::env::var("TX_EVAL_DUMP") {
        let json = serde_json::json!({
            "bus_substation": input.bus_substation, "kv": input.kv, "from": input.from,
            "to": input.to, "in_service": input.in_service, "xy": input.xy,
        });
        std::fs::write(path, json.to_string()).expect("write layout input");
    }
    let name = std::path::Path::new(&case)
        .file_name()
        .map_or(case.clone(), |f| f.to_string_lossy().into_owned());
    write_out(report(&name, &input, group_ms));
}

/// A synthetic grid at interconnection scale: substations scattered over a 1,000 km square with
/// denser "cities", two voltage levels each, joined to their nearest neighbours.
#[test]
#[ignore]
fn evaluate_synthetic_scale() {
    let mut runs = Vec::new();
    for substations in [2_000usize, 10_000, 35_000] {
        let mut input = TxLayoutInput::default();
        let hash = |i: usize, k: f64| ((i as f64 * k).sin() * 43_758.545_3).fract().abs();
        let mut row = 0i64;
        let mut first = Vec::new();
        for s in 0..substations {
            // A third of substations cluster around ten cities.
            let (x, y) = if s % 3 == 0 {
                let c = s % 10;
                let (cx, cy) = (hash(c, 1.7) * 1e6, hash(c, 2.3) * 1e6);
                (
                    cx + (hash(s, 3.1) - 0.5) * 6e4,
                    cy + (hash(s, 4.7) - 0.5) * 6e4,
                )
            } else {
                (hash(s, 5.3) * 1e6, hash(s, 6.1) * 1e6)
            };
            first.push(row);
            for (b, kv) in [345.0, 138.0].into_iter().enumerate() {
                input.bus_substation.push(s as u32);
                input.kv.push(kv);
                input.xy.extend_from_slice(&[x, y]);
                if b > 0 {
                    input.from.push(row - 1);
                    input.to.push(row);
                    input.in_service.push(1);
                }
                row += 1;
            }
        }
        // Lines to the next two substations in a space-filling order: a sparse meshed graph.
        let mut order: Vec<usize> = (0..substations).collect();
        order.sort_by_key(|&s| {
            let (x, y) = (
                input.xy[first[s] as usize * 2],
                input.xy[first[s] as usize * 2 + 1],
            );
            ((y / 2e4) as i64, (x / 2e4) as i64)
        });
        for w in order.windows(3) {
            for &t in &w[1..] {
                input.from.push(first[w[0]]);
                input.to.push(first[t]);
                input.in_service.push(1);
            }
        }
        runs.extend(report(&format!("synthetic {substations}"), &input, 0.0));
    }
    write_out(runs);
}
