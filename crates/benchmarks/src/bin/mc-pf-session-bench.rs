//! Retained multiconductor PF session benchmark (eigenergy/tellegen#132).
//!
//! Measures the public `McPfSession` API exactly as the browser engine drives
//! it: cold creation from a stored PowerIO module, single-branch absolute load
//! edits (the accumulated edit set the UI sends), the per-edit JSON result
//! payload the WASM adapter returns, a feeder-wide edit checked against a
//! fresh solve, and on-demand input/snapshot materialization. A counting
//! global allocator reports retained and peak heap bytes.
//!
//! Usage:
//! ```text
//! cargo run -p benchmarks --profile release-py --bin mc-pf-session-bench -- [flags]
//!   --preset NAME          tiny | feeder-10k | feeder-106k | x300k | x650k
//!   --module PATH          a stored PowerIO module (MulticonductorNetwork or McAcPfInstance)
//!   --bmopf PATH           a raw BMOPF document
//!   --edits N              single-branch edits (default 20)
//!   --seed N               edit sequence seed (default 132)
//!   --write-module PATH    also write the module JSON (and PATH's .edits.json sequence)
//!   --out DIR              artifact directory (default target/mc-pf-bench)
//!   --label NAME           artifact label (default: preset or file stem)
//!   --load-scale X         override the preset load multiplier
//!   --calibrate X,Y,...    solve the preset at each load multiplier and exit
//! ```
//!
//! Use the `release-py` profile (opt-level 3, thin LTO). The default `release`
//! profile is opt-level "s" for wasm size; the artifacts record which profile
//! built the binary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use benchmarks::mc_feeder::{self, FeederShape, Preset, SplitMix64};
use powerio::{PioModule, PioValue};
use powerio_dist::MulticonductorNetwork;
use powerio_prob::McAcPfInstance;
use serde_json::{json, Value};
use tellegen::{McLoadPowerEdit, McPfOptions, McPfSession};

/// Counts live and peak heap bytes on top of the system allocator.
struct CountingAllocator;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

fn shrink(bytes: usize) {
    LIVE.fetch_sub(bytes, Ordering::Relaxed);
}

// SAFETY: every call forwards to `System` with the caller's layout; the
// counters are side bookkeeping and never affect the returned pointers.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        shrink(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            if new_size >= layout.size() {
                grow(new_size - layout.size());
            } else {
                shrink(layout.size() - new_size);
            }
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// One timed region with its heap accounting.
struct Measured<T> {
    value: T,
    ms: f64,
    /// Highest live heap during the region, above the live heap at its start.
    peak_bytes: usize,
    /// Live heap after the region (with `value` still alive) minus before.
    retained_bytes: i64,
}

fn measure<T>(region: impl FnOnce() -> T) -> Measured<T> {
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let started = Instant::now();
    let value = region();
    let ms = started.elapsed().as_secs_f64() * 1e3;
    let after = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    Measured {
        value,
        ms,
        peak_bytes: peak.saturating_sub(before),
        retained_bytes: after as i64 - before as i64,
    }
}

enum Input {
    Preset(Preset),
    Module(PathBuf),
    Bmopf(PathBuf),
}

struct Args {
    input: Input,
    edits: usize,
    seed: u64,
    write_module: Option<PathBuf>,
    out: PathBuf,
    label: Option<String>,
    load_scale: Option<f64>,
    calibrate: Option<Vec<f64>>,
}

const USAGE: &str = "usage: mc-pf-session-bench (--preset NAME | --module PATH | --bmopf PATH) \
[--edits N] [--seed N] [--write-module PATH] [--out DIR] [--label NAME] [--load-scale X] \
[--calibrate X,Y,...]\npresets: tiny, feeder-10k, feeder-106k, x300k, x650k";

fn parse_args() -> Result<Args, String> {
    let mut input = None;
    let mut edits = 20usize;
    let mut seed = 132u64;
    let mut write_module = None;
    let mut out = PathBuf::from("target/mc-pf-bench");
    let mut label = None;
    let mut load_scale = None;
    let mut calibrate = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag.to_owned(), Some(value.to_owned())),
            None => (arg, None),
        };
        if flag == "--help" || flag == "-h" {
            println!("{USAGE}");
            std::process::exit(0);
        }
        let mut value = || {
            inline
                .clone()
                .or_else(|| it.next())
                .ok_or_else(|| format!("{flag} requires a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--preset" => {
                let name = value()?;
                let preset =
                    Preset::parse(&name).ok_or_else(|| format!("unknown preset `{name}`"))?;
                input = Some(Input::Preset(preset));
            }
            "--module" => input = Some(Input::Module(PathBuf::from(value()?))),
            "--bmopf" => input = Some(Input::Bmopf(PathBuf::from(value()?))),
            "--edits" => edits = value()?.parse().map_err(|e| format!("--edits: {e}"))?,
            "--seed" => seed = value()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--write-module" => write_module = Some(PathBuf::from(value()?)),
            "--out" => out = PathBuf::from(value()?),
            "--label" => label = Some(value()?),
            "--load-scale" => {
                load_scale = Some(value()?.parse().map_err(|e| format!("--load-scale: {e}"))?)
            }
            "--calibrate" => {
                calibrate = Some(
                    value()?
                        .split(',')
                        .map(|x| x.trim().parse::<f64>())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("--calibrate: {e}"))?,
                )
            }
            other => return Err(format!("unknown flag `{other}`\n{USAGE}")),
        }
    }
    let input = input.ok_or_else(|| USAGE.to_owned())?;
    if calibrate.is_some() && !matches!(input, Input::Preset(_)) {
        return Err("--calibrate requires --preset".to_owned());
    }
    if load_scale.is_some() && !matches!(input, Input::Preset(_)) {
        return Err("--load-scale requires --preset".to_owned());
    }
    Ok(Args {
        input,
        edits,
        seed,
        write_module,
        out,
        label,
        load_scale,
        calibrate,
    })
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("mc-pf-session-bench: {error}");
            std::process::exit(2);
        }
    };
    let outcome = match &args.calibrate {
        Some(scales) => calibrate(&args, scales),
        None => run(&args),
    };
    if let Err(error) = outcome {
        eprintln!("mc-pf-session-bench: {error}");
        std::process::exit(1);
    }
}

fn preset_spec(preset: Preset, load_scale: Option<f64>) -> mc_feeder::FeederSpec {
    let mut spec = preset.spec();
    if let Some(scale) = load_scale {
        spec.load_scale = scale;
    }
    spec
}

/// Solve a preset at several load multipliers to choose its frozen scale.
fn calibrate(args: &Args, scales: &[f64]) -> Result<(), String> {
    let Input::Preset(preset) = args.input else {
        return Err("--calibrate requires --preset".to_owned());
    };
    println!(
        "| preset | load_scale | iterations | min V (pu) | max V (pu) | violations | cold ms |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: |");
    for &scale in scales {
        let network = mc_feeder::generate_spec(&preset_spec(preset, Some(scale)));
        let instance = McAcPfInstance::from_network(network).map_err(|e| e.to_string())?;
        let started = Instant::now();
        match McPfSession::new(instance, McPfOptions::default()) {
            Ok(session) => {
                let result = session.result();
                println!(
                    "| {preset} | {scale} | {} | {:.4} | {:.4} | {} | {:.0} |",
                    result.iterations,
                    result.min_voltage_pu.unwrap_or(f64::NAN),
                    result.max_voltage_pu.unwrap_or(f64::NAN),
                    result.voltage_violations.len(),
                    started.elapsed().as_secs_f64() * 1e3
                );
            }
            Err(error) => println!("| {preset} | {scale} | failed: {error} | | | | |"),
        }
    }
    Ok(())
}

/// The network a module or BMOPF input calculates on.
fn network_of(value: &PioValue) -> Result<MulticonductorNetwork, String> {
    match value {
        PioValue::MulticonductorNetwork(network) => Ok(network.clone()),
        PioValue::McAcPfInstance(instance) => Ok(instance.network().clone()),
        other => Err(format!(
            "module holds {}; expected MulticonductorNetwork or McAcPfInstance",
            other.type_name()
        )),
    }
}

fn file_stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "input".to_owned());
    for suffix in [".pio.json", ".bmopf.json", ".json"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    name
}

/// The profile directory the binary was built into (`target/<profile>/`).
fn build_profile() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .and_then(|dir| dir.file_name())
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|text| !text.is_empty())
}

#[derive(Default)]
struct Stats {
    values: Vec<f64>,
}

impl Stats {
    fn push(&mut self, value: f64) {
        self.values.push(value);
    }

    fn summary(&self) -> Value {
        if self.values.is_empty() {
            return Value::Null;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(f64::total_cmp);
        let rank =
            |q: f64| sorted[((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1];
        let median = if sorted.len() % 2 == 1 {
            sorted[sorted.len() / 2]
        } else {
            0.5 * (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2])
        };
        json!({
            "n": sorted.len(),
            "min": sorted[0],
            "median": median,
            "p95": rank(0.95),
            "max": sorted[sorted.len() - 1],
            "mean": sorted.iter().sum::<f64>() / sorted.len() as f64,
        })
    }
}

fn mib(bytes: f64) -> String {
    format!("{:.1} MiB", bytes / (1024.0 * 1024.0))
}

/// Deterministic single-branch edits: a seeded branch and independent
/// ±5-20 % changes of its base P and Q.
fn edit_sequence(
    branches: &[tellegen::McLoadBranchState],
    count: usize,
    seed: u64,
) -> Vec<McLoadPowerEdit> {
    let mut rng = SplitMix64::new(seed);
    let factor = |rng: &mut SplitMix64| {
        let magnitude = rng.uniform(0.05, 0.20);
        if rng.chance(0.5) {
            1.0 + magnitude
        } else {
            1.0 - magnitude
        }
    };
    (0..count)
        .map(|_| {
            let branch = &branches[rng.below(branches.len())];
            let p = factor(&mut rng);
            let q = factor(&mut rng);
            McLoadPowerEdit {
                load: branch.load.clone(),
                branch: branch.branch,
                p_w: branch.base_p_w * p,
                q_var: branch.base_q_var * q,
            }
        })
        .collect()
}

fn run(args: &Args) -> Result<(), String> {
    let options = McPfOptions::default();
    let mut notes = vec![
        "replace_load_powers times include today's full MulticonductorNetwork clone, McAcPfInstance rebuild, warm solve, and complete McPfResult construction; they are inseparable through the public API".to_owned(),
        "result_json_* is serde_json::to_string(&McPfResult), the payload the WASM adapter returns per edit".to_owned(),
    ];

    // Input: a generated preset, a stored module, or a raw BMOPF document.
    let (label, source, generation, network, module_json, serialize) = match &args.input {
        Input::Preset(preset) => {
            let spec = preset_spec(*preset, args.load_scale);
            eprintln!("generating {preset} (load_scale {})", spec.load_scale);
            let generated = measure(|| mc_feeder::generate_spec(&spec));
            let module = PioModule::new(PioValue::MulticonductorNetwork(generated.value.clone()));
            let serialized = measure(|| tellegen::ir::serialize_module(&module));
            drop(module);
            let label = match args.load_scale {
                Some(scale) => format!("{preset}-scale-{scale}"),
                None => preset.name().to_owned(),
            };
            (
                label,
                json!({"kind": "preset", "preset": preset.name(), "seed": spec.seed, "load_scale": spec.load_scale}),
                json!({"ms": generated.ms, "peak_heap_bytes": generated.peak_bytes, "retained_heap_bytes": generated.retained_bytes}),
                generated.value,
                serialized.value?,
                json!({"ms": serialized.ms, "peak_heap_bytes": serialized.peak_bytes}),
            )
        }
        Input::Module(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let parsed = measure(|| tellegen::ir::deserialize_module(&text));
            let module = parsed.value?;
            let network = network_of(module.value())?;
            (
                file_stem(path),
                json!({"kind": "module", "path": path.display().to_string()}),
                json!({"bench_side_deserialize_ms": parsed.ms}),
                network,
                text,
                Value::Null,
            )
        }
        Input::Bmopf(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let parsed = measure(|| tellegen::parse_bmopf_instance(&text));
            let instance = parsed.value?;
            let network = instance.network().clone();
            let module = PioModule::new(PioValue::MulticonductorNetwork(network.clone()));
            let serialized = measure(|| tellegen::ir::serialize_module(&module));
            notes.push("the BMOPF document is converted to a stored MulticonductorNetwork module before the session is created".to_owned());
            (
                file_stem(path),
                json!({"kind": "bmopf", "path": path.display().to_string(), "bmopf_bytes": text.len()}),
                json!({"bmopf_parse_ms": parsed.ms}),
                network,
                serialized.value?,
                json!({"ms": serialized.ms, "peak_heap_bytes": serialized.peak_bytes}),
            )
        }
    };
    let label = args.label.clone().unwrap_or(label);
    let shape = FeederShape::of(&network);
    eprintln!(
        "{label}: {} buses, {} terminals, {} lines, {} transformers, {} loads, {} load branches; module {}",
        shape.buses,
        shape.terminals,
        shape.lines,
        shape.transformers,
        shape.loads,
        shape.load_branches,
        mib(module_json.len() as f64)
    );

    // Cold: parse + prepare + factor + solve.
    eprintln!("cold session creation");
    let created = measure(|| McPfSession::from_module_json(&module_json, options));
    let cold_ms = created.ms;
    let cold_peak = created.peak_bytes;
    let cold_retained = created.retained_bytes;
    let mut session = created.value?;
    let initial = session.result();
    let initial_json = measure(|| serde_json::to_string(initial).map_err(|e| e.to_string()));
    let initial_json_bytes = initial_json.value?.len();
    let branches = measure(|| session.load_branches());
    let branches_json =
        measure(|| serde_json::to_string(&branches.value).map_err(|e| e.to_string()));
    let branches_json_bytes = branches_json.value?.len();
    let cold = json!({
        "from_module_json_ms": cold_ms,
        "iterations": initial.iterations,
        "converged": initial.converged,
        "voltage_valid": initial.voltage_valid,
        "min_voltage_pu": initial.min_voltage_pu,
        "max_voltage_pu": initial.max_voltage_pu,
        "voltage_violations": initial.voltage_violations.len(),
        "matrix_dimension": initial.matrix_dimension,
        "matrix_nonzeros": initial.matrix_nonzeros,
        "factorization_count": initial.factorization_count,
        "retained_heap_bytes": cold_retained,
        "peak_heap_bytes": cold_peak,
        "result_json_ms": initial_json.ms,
        "result_json_bytes": initial_json_bytes,
        "load_branches_ms": branches.ms,
        "load_branches_json_ms": branches_json.ms,
        "load_branches_json_bytes": branches_json_bytes,
    });
    eprintln!(
        "  {:.0} ms, {} iterations, dim {}, nnz {}, min {:.4} pu, retained {}, peak {}",
        cold_ms,
        initial.iterations,
        initial.matrix_dimension,
        initial.matrix_nonzeros,
        initial.min_voltage_pu.unwrap_or(f64::NAN),
        mib(cold_retained as f64),
        mib(cold_peak as f64)
    );
    let branches = branches.value;
    if branches.is_empty() {
        return Err("the network has no editable load branches".to_owned());
    }

    // Single-branch edits; each call sends the accumulated absolute set.
    let sequence = edit_sequence(&branches, args.edits, args.seed);
    if let Some(path) = &args.write_module {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, &module_json)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let edits_path = edits_path_for(path);
        let edits_doc = json!({
            "schema": "tellegen-mc-pf-bench-edits",
            "version": 1,
            "seed": args.seed,
            "note": "single-branch edits in order; each request sends the accumulated absolute set",
            "edits": sequence,
        });
        std::fs::write(&edits_path, edits_doc.to_string())
            .map_err(|e| format!("cannot write {}: {e}", edits_path.display()))?;
        eprintln!("wrote {} and {}", path.display(), edits_path.display());
    }
    let mut accumulated: Vec<McLoadPowerEdit> = Vec::new();
    let mut position: BTreeMap<(String, usize), usize> = BTreeMap::new();
    let mut per_edit = Vec::new();
    let (mut replace_ms, mut iterations, mut json_ms, mut json_bytes, mut peak, mut lb_ms) = (
        Stats::default(),
        Stats::default(),
        Stats::default(),
        Stats::default(),
        Stats::default(),
        Stats::default(),
    );
    for (index, edit) in sequence.iter().enumerate() {
        let key = (edit.load.clone(), edit.branch);
        match position.get(&key) {
            Some(&at) => accumulated[at] = edit.clone(),
            None => {
                position.insert(key, accumulated.len());
                accumulated.push(edit.clone());
            }
        }
        let replaced = measure(|| {
            session
                .replace_load_powers(&accumulated)
                .map(|result| (result.iterations, result.factorization_count))
        });
        let (edit_iterations, factorizations) = replaced
            .value
            .map_err(|e| format!("edit {index} failed: {e}"))?;
        let serialized =
            measure(|| serde_json::to_string(session.result()).map_err(|e| e.to_string()));
        let bytes = serialized.value?.len();
        let listed =
            measure(|| serde_json::to_string(&session.load_branches()).map_err(|e| e.to_string()));
        let listed_bytes = listed.value?.len();
        replace_ms.push(replaced.ms);
        iterations.push(edit_iterations as f64);
        json_ms.push(serialized.ms);
        json_bytes.push(bytes as f64);
        peak.push(replaced.peak_bytes as f64);
        lb_ms.push(listed.ms);
        eprintln!(
            "  edit {index:>2}: {:>8.1} ms, {:>2} iterations, result JSON {:>6.1} ms / {}, peak {}",
            replaced.ms,
            edit_iterations,
            serialized.ms,
            mib(bytes as f64),
            mib(replaced.peak_bytes as f64)
        );
        per_edit.push(json!({
            "index": index,
            "load": edit.load,
            "branch": edit.branch,
            "accumulated_edits": accumulated.len(),
            "replace_load_powers_ms": replaced.ms,
            "iterations": edit_iterations,
            "factorization_count": factorizations,
            "peak_heap_bytes": replaced.peak_bytes,
            "result_json_ms": serialized.ms,
            "result_json_bytes": bytes,
            "result_json_peak_heap_bytes": serialized.peak_bytes,
            "load_branches_json_ms": listed.ms,
            "load_branches_json_bytes": listed_bytes,
        }));
    }
    let edits = json!({
        "count": sequence.len(),
        "seed": args.seed,
        "per_edit": per_edit,
        "replace_load_powers_ms": replace_ms.summary(),
        "iterations": iterations.summary(),
        "result_json_ms": json_ms.summary(),
        "result_json_bytes": json_bytes.summary(),
        "peak_heap_bytes": peak.summary(),
        "load_branches_json_ms": lb_ms.summary(),
    });

    // Feeder-wide 1.05x edit, then agreement with a fresh prepared solve.
    eprintln!("feeder-wide 1.05x edit");
    let scaled: Vec<McLoadPowerEdit> = branches
        .iter()
        .map(|branch| McLoadPowerEdit {
            load: branch.load.clone(),
            branch: branch.branch,
            p_w: branch.base_p_w * 1.05,
            q_var: branch.base_q_var * 1.05,
        })
        .collect();
    let wide = measure(|| {
        session
            .replace_load_powers(&scaled)
            .map(|result| result.iterations)
    });
    let wide_iterations = wide
        .value
        .map_err(|e| format!("feeder-wide edit failed: {e}"))?;
    let wide_json = measure(|| serde_json::to_string(session.result()).map_err(|e| e.to_string()));
    let wide_json_bytes = wide_json.value?.len();
    let mut fresh_network = network.clone();
    for load in fresh_network.loads_mut() {
        load.p_nom.iter_mut().for_each(|p| *p *= 1.05);
        load.q_nom.iter_mut().for_each(|q| *q *= 1.05);
    }
    let fresh_instance = McAcPfInstance::from_network(fresh_network).map_err(|e| e.to_string())?;
    drop(network);
    let fresh = measure(|| tellegen::solve_mc_ac_pf_instance(&fresh_instance, &options));
    let fresh_result = fresh.value?;
    let warm = session.result();
    if warm.terminals.len() != fresh_result.terminals.len() {
        return Err("warm and fresh terminal counts differ".to_owned());
    }
    let (mut max_dv, mut max_di) = (0.0f64, 0.0f64);
    for (w, f) in warm.terminals.iter().zip(&fresh_result.terminals) {
        if w.bus != f.bus || w.terminal != f.terminal {
            return Err(format!(
                "terminal order differs: {}:{} vs {}:{}",
                w.bus, w.terminal, f.bus, f.terminal
            ));
        }
        max_dv = max_dv.max((w.voltage.re - f.voltage.re).hypot(w.voltage.im - f.voltage.im));
        max_di = max_di.max(
            (w.current_into_network.re - f.current_into_network.re)
                .hypot(w.current_into_network.im - f.current_into_network.im),
        );
    }
    let feeder_wide = json!({
        "edited_branches": scaled.len(),
        "replace_load_powers_ms": wide.ms,
        "iterations": wide_iterations,
        "peak_heap_bytes": wide.peak_bytes,
        "result_json_ms": wide_json.ms,
        "result_json_bytes": wide_json_bytes,
        "fresh_solve_ms": fresh.ms,
        "fresh_iterations": fresh_result.iterations,
        "fresh_peak_heap_bytes": fresh.peak_bytes,
        "max_abs_voltage_difference_v": max_dv,
        "max_abs_current_difference_a": max_di,
    });
    eprintln!(
        "  {:.0} ms, {} iterations; fresh {:.0} ms; max |dV| {:.3e} V, max |dI| {:.3e} A",
        wide.ms, wide_iterations, fresh.ms, max_dv, max_di
    );
    drop(fresh_result);
    drop(fresh_instance);
    let factorization_count = session.factorization_count();
    if factorization_count != 1 {
        notes.push(format!(
            "factorization_count is {factorization_count} after the edits; expected 1"
        ));
    }

    // On-demand portable materialization.
    eprintln!("input module and snapshot");
    let input_module = measure(|| session.input_module_json());
    let input_module_bytes = input_module.value?.len();
    let snapshot = measure(|| session.snapshot("mc-pf-session-bench", "Session benchmark"));
    let snapshot_ms = snapshot.ms;
    let snapshot_peak = snapshot.peak_bytes;
    let (snapshot_json, snapshot_value) = match snapshot.value {
        Ok(value) => {
            let text = measure(|| serde_json::to_string(&value).map_err(|e| e.to_string()));
            let bytes = text.value.as_ref().map(String::len).map_err(Clone::clone);
            (
                json!({
                    "snapshot_ms": snapshot_ms,
                    "snapshot_peak_heap_bytes": snapshot_peak,
                    "input_module_bytes": value.input_module.len(),
                    "solution_module_bytes": value.solution_module.len(),
                    "to_string_ms": text.ms,
                    "to_string_peak_heap_bytes": text.peak_bytes,
                    "json_bytes": bytes.ok(),
                }),
                Some(value),
            )
        }
        Err(error) => (json!({"snapshot_ms": snapshot_ms, "error": error}), None),
    };
    drop(snapshot_value);
    let materialize = json!({
        "input_module_json_ms": input_module.ms,
        "input_module_json_bytes": input_module_bytes,
        "input_module_json_peak_heap_bytes": input_module.peak_bytes,
        "snapshot": snapshot_json,
    });

    let report = json!({
        "schema": "tellegen-mc-pf-session-bench",
        "version": 1,
        "label": label,
        "source": source,
        "build": {
            "profile": build_profile(),
            "debug_assertions": cfg!(debug_assertions),
            "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            "git_commit": command_output("git", &["rev-parse", "HEAD"]),
        },
        "machine": {
            "cpu": command_output("sysctl", &["-n", "machdep.cpu.brand_string"]),
            "memory_bytes": command_output("sysctl", &["-n", "hw.memsize"]).and_then(|v| v.parse::<u64>().ok()),
            "available_parallelism": std::thread::available_parallelism().map(|n| n.get()).ok(),
        },
        "options": options,
        "generation": generation,
        "shape": shape,
        "module": {"bytes": module_json.len(), "serialize": serialize},
        "cold": cold,
        "edits": edits,
        "feeder_wide": feeder_wide,
        "factorization_count_after_edits": factorization_count,
        "solve_count": session.solve_count(),
        "materialize": materialize,
        "notes": notes,
    });
    std::fs::create_dir_all(&args.out)
        .map_err(|e| format!("cannot create {}: {e}", args.out.display()))?;
    let json_path = args.out.join(format!("native-{label}.json"));
    let md_path = args.out.join(format!("native-{label}.md"));
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("cannot write {}: {e}", json_path.display()))?;
    std::fs::write(&md_path, markdown(&report))
        .map_err(|e| format!("cannot write {}: {e}", md_path.display()))?;
    eprintln!("wrote {} and {}", json_path.display(), md_path.display());
    print!("{}", markdown(&report));
    Ok(())
}

fn edits_path_for(module: &Path) -> PathBuf {
    let name = module
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = name
        .strip_suffix(".pio.json")
        .or_else(|| name.strip_suffix(".json"))
        .unwrap_or(&name);
    module.with_file_name(format!("{stem}.edits.json"))
}

fn markdown(report: &Value) -> String {
    let get = |pointer: &str| report.pointer(pointer).cloned().unwrap_or(Value::Null);
    let num = |pointer: &str| get(pointer).as_f64().unwrap_or(f64::NAN);
    let ms = |pointer: &str| format!("{:.1}", num(pointer));
    let bytes = |pointer: &str| mib(num(pointer));
    let count = |pointer: &str| match get(pointer) {
        Value::Number(n) => n.to_string(),
        Value::Null => "-".to_owned(),
        other => other.to_string(),
    };
    let mut rows: Vec<(String, String)> = vec![
        (
            "build profile".into(),
            get("/build/profile").as_str().unwrap_or("?").to_owned(),
        ),
        (
            "shape (buses / terminals / lines / transformers)".into(),
            format!(
                "{} / {} / {} / {}",
                count("/shape/buses"),
                count("/shape/terminals"),
                count("/shape/lines"),
                count("/shape/transformers")
            ),
        ),
        (
            "loads / load branches".into(),
            format!(
                "{} / {}",
                count("/shape/loads"),
                count("/shape/load_branches")
            ),
        ),
        ("generation ms".into(), ms("/generation/ms")),
        (
            "module JSON (bytes, serialize ms)".into(),
            format!(
                "{} ({}), {}",
                bytes("/module/bytes"),
                count("/module/bytes"),
                ms("/module/serialize/ms")
            ),
        ),
        (
            "cold from_module_json ms".into(),
            ms("/cold/from_module_json_ms"),
        ),
        (
            "cold iterations / matrix dim / nnz".into(),
            format!(
                "{} / {} / {}",
                count("/cold/iterations"),
                count("/cold/matrix_dimension"),
                count("/cold/matrix_nonzeros")
            ),
        ),
        (
            "cold min / max load-branch V (pu)".into(),
            format!(
                "{:.4} / {:.4}",
                num("/cold/min_voltage_pu"),
                num("/cold/max_voltage_pu")
            ),
        ),
        (
            "cold heap retained / peak".into(),
            format!(
                "{} / {}",
                bytes("/cold/retained_heap_bytes"),
                bytes("/cold/peak_heap_bytes")
            ),
        ),
        (
            "initial result JSON (ms, size)".into(),
            format!(
                "{}, {}",
                ms("/cold/result_json_ms"),
                bytes("/cold/result_json_bytes")
            ),
        ),
        (
            "load_branches JSON (ms, size)".into(),
            format!(
                "{}, {}",
                ms("/cold/load_branches_json_ms"),
                bytes("/cold/load_branches_json_bytes")
            ),
        ),
    ];
    let summary = |name: &str, pointer: &str, unit: &dyn Fn(f64) -> String| {
        (
            name.to_owned(),
            format!(
                "{} / {} / {}",
                unit(num(&format!("{pointer}/median"))),
                unit(num(&format!("{pointer}/p95"))),
                unit(num(&format!("{pointer}/max")))
            ),
        )
    };
    let plain_ms = |value: f64| format!("{value:.1} ms");
    let plain = |value: f64| format!("{value:.0}");
    let size = |value: f64| mib(value);
    rows.push(("single-branch edits".into(), count("/edits/count")));
    rows.push(summary(
        "replace_load_powers median / p95 / max",
        "/edits/replace_load_powers_ms",
        &plain_ms,
    ));
    rows.push(summary(
        "edit iterations median / p95 / max",
        "/edits/iterations",
        &plain,
    ));
    rows.push(summary(
        "result JSON serialize median / p95 / max",
        "/edits/result_json_ms",
        &plain_ms,
    ));
    rows.push(summary(
        "result JSON size median / p95 / max",
        "/edits/result_json_bytes",
        &size,
    ));
    rows.push(summary(
        "edit heap peak median / p95 / max",
        "/edits/peak_heap_bytes",
        &size,
    ));
    rows.push(summary(
        "load_branches + JSON median / p95 / max",
        "/edits/load_branches_json_ms",
        &plain_ms,
    ));
    rows.push((
        "feeder-wide 1.05x (ms, iterations)".into(),
        format!(
            "{}, {}",
            ms("/feeder_wide/replace_load_powers_ms"),
            count("/feeder_wide/iterations")
        ),
    ));
    rows.push((
        "fresh 1.05x solve (ms, iterations)".into(),
        format!(
            "{}, {}",
            ms("/feeder_wide/fresh_solve_ms"),
            count("/feeder_wide/fresh_iterations")
        ),
    ));
    rows.push((
        "warm vs fresh max abs dV / dI".into(),
        format!(
            "{:.2e} V / {:.2e} A",
            num("/feeder_wide/max_abs_voltage_difference_v"),
            num("/feeder_wide/max_abs_current_difference_a")
        ),
    ));
    rows.push((
        "factorization count after edits".into(),
        count("/factorization_count_after_edits"),
    ));
    rows.push((
        "input_module_json (ms, size, peak)".into(),
        format!(
            "{}, {}, {}",
            ms("/materialize/input_module_json_ms"),
            bytes("/materialize/input_module_json_bytes"),
            bytes("/materialize/input_module_json_peak_heap_bytes")
        ),
    ));
    if get("/materialize/snapshot/error").is_null() {
        rows.push((
            "snapshot (ms, peak)".into(),
            format!(
                "{}, {}",
                ms("/materialize/snapshot/snapshot_ms"),
                bytes("/materialize/snapshot/snapshot_peak_heap_bytes")
            ),
        ));
        rows.push((
            "snapshot to_string (ms, size, peak)".into(),
            format!(
                "{}, {}, {}",
                ms("/materialize/snapshot/to_string_ms"),
                bytes("/materialize/snapshot/json_bytes"),
                bytes("/materialize/snapshot/to_string_peak_heap_bytes")
            ),
        ));
    } else {
        rows.push((
            "snapshot".into(),
            format!("failed: {}", get("/materialize/snapshot/error")),
        ));
    }
    let mut text = format!(
        "### Native session benchmark: {}\n\n| measurement | value |\n| --- | --- |\n",
        get("/label").as_str().unwrap_or("?")
    );
    for (name, value) in rows {
        text.push_str(&format!("| {name} | {value} |\n"));
    }
    text.push('\n');
    text
}
