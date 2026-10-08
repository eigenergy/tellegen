//! DC OPF at transmission-interconnection scale: time and heap for reading a
//! case, building its PowerIO instance, and solving it under one or more
//! requests.
//!
//! The input is a case file PowerIO reads (PGLib `case78484` is the largest in
//! v23.07) or a synthetic meshed grid from [`benchmarks::meshed_grid`]. Each
//! `--request NAME=JSON` runs one `solve_instance` call through the public API;
//! by default it runs every declared limit, the same without angle-difference
//! rows, and lazily enforced thermal limits. One JSON line per stage goes to
//! stdout.
//!
//! Usage:
//! ```text
//! cargo run -p benchmarks --profile release-py --bin dcopf-scale-bench -- [flags]
//!   --case PATH            a case file PowerIO reads
//!   --grid ROWSxCOLS       a synthetic meshed grid (default 316x316, 99,856 buses)
//!   --request NAME=JSON    a solve request to time; repeatable
//! ```
//!
//! Use the `release-py` profile (opt-level 3, thin LTO); the default `release`
//! profile is opt-level "s" for wasm size.

use std::path::PathBuf;

use benchmarks::heap::{measure, CountingAllocator};
use benchmarks::meshed_grid::{self, GridShape};
use serde_json::{json, Value};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const USAGE: &str = "usage: dcopf-scale-bench [--case PATH | --grid ROWSxCOLS] \
[--request NAME=JSON]...";

/// Every declared limit; without angle-difference rows; and lazily enforced
/// thermal limits.
const DEFAULT_REQUESTS: [(&str, &str); 3] = [
    ("full", r#"{"limits":{}}"#),
    ("no-angle-rows", r#"{"limits":{"angle_difference":false}}"#),
    ("lazy", r#"{"limits":{"lazy":{"near_binding":0.98}}}"#),
];

enum Input {
    Case(PathBuf),
    Grid(GridShape),
}

fn parse_args() -> Result<(Input, Vec<(String, String)>), String> {
    let mut input = Input::Grid(GridShape::new(316, 316));
    let mut requests = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| format!("{flag} requires a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--case" => input = Input::Case(PathBuf::from(value()?)),
            "--grid" => {
                let text = value()?;
                input = Input::Grid(
                    GridShape::parse(&text).ok_or_else(|| format!("bad --grid `{text}`"))?,
                );
            }
            "--request" => {
                let text = value()?;
                let (name, request) = text
                    .split_once('=')
                    .ok_or_else(|| format!("--request takes NAME=JSON, got `{text}`"))?;
                requests.push((name.to_owned(), request.to_owned()));
            }
            other => return Err(format!("unknown flag `{other}`\n{USAGE}")),
        }
    }
    if requests.is_empty() {
        for (name, request) in DEFAULT_REQUESTS {
            requests.push((name.to_owned(), request.to_owned()));
        }
    }
    Ok((input, requests))
}

fn stage(name: &str, ms: f64, peak_bytes: usize, extra: Value) {
    let mut line = json!({
        "stage": name,
        "ms": (ms * 10.0).round() / 10.0,
        "peak_heap_mb": (peak_bytes as f64 / 1e6 * 10.0).round() / 10.0,
    });
    if let (Some(line), Some(extra)) = (line.as_object_mut(), extra.as_object()) {
        line.extend(extra.clone());
    }
    println!("{line}");
}

fn main() -> Result<(), String> {
    let (input, requests) = parse_args()?;
    let (name, bytes) = match &input {
        Input::Case(path) => {
            let read =
                measure(|| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display())));
            stage("read", read.ms, read.peak_bytes, json!({}));
            (
                path.file_name().map_or_else(
                    || "case".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                ),
                read.value?,
            )
        }
        Input::Grid(shape) => {
            let generated = measure(|| meshed_grid::matpower(shape));
            stage(
                "generate",
                generated.ms,
                generated.peak_bytes,
                json!({ "rows": shape.rows, "cols": shape.cols }),
            );
            (
                format!("meshed_{}x{}.m", shape.rows, shape.cols),
                generated.value.into_bytes(),
            )
        }
    };
    let parsed = measure(|| -> Result<powerio::BalancedNetwork, String> {
        let source = powerio::Source::from_memory(&name, bytes).map_err(|e| e.to_string())?;
        let module = powerio::parse(source).map_err(|e| e.to_string())?;
        Ok(tellegen::ir::balanced_module(module)?.into_value())
    });
    let network = parsed.value?;
    stage(
        "parse",
        parsed.ms,
        parsed.peak_bytes,
        json!({
            "case": name,
            "buses": network.buses().len(),
            "branches": network.branches().len(),
            "generators": network.generators().len(),
        }),
    );
    let built = measure(|| powerio::DcOpfInstance::from_network(network));
    stage("instance", built.ms, built.peak_bytes, json!({}));
    let instance = built.value.map_err(|e| e.to_string())?;

    for (name, request_json) in requests {
        let request: tellegen::SolveRequest =
            serde_json::from_str(&request_json).map_err(|e| format!("{name}: {e}"))?;
        let solved = measure(|| tellegen::solve_instance(&instance, &request));
        let response = match solved.value {
            Ok(response) => response,
            Err(error) => {
                stage(
                    &format!("solve:{name}"),
                    solved.ms,
                    solved.peak_bytes,
                    json!({ "error": error }),
                );
                continue;
            }
        };
        let flows = response.flows.as_deref().unwrap_or_default();
        let max_loading = flows.iter().map(|f| f.loading).fold(0.0, f64::max);
        let binding = flows.iter().filter(|f| f.loading > 1.0 - 1e-6).count();
        let ipm_iterations = match &response.iterations {
            Some(tellegen::Iterations::Ipm(trace)) => trace.len(),
            _ => 0,
        };
        let report = serde_json::to_value(&response).map_err(|e| e.to_string())?;
        stage(
            &format!("solve:{name}"),
            solved.ms,
            solved.peak_bytes,
            json!({
                "objective": response.objective,
                "ipm_iterations": ipm_iterations,
                "max_loading": max_loading,
                "binding_limits": binding,
                "limit_rounds": report.get("limit_rounds"),
            }),
        );
    }
    Ok(())
}
