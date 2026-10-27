//! `tellegen generate` and `tellegen stats`: synthetic grid generation and
//! the structural statistics it is measured by, from the command line. Both
//! use the generator's own code, so a generated grid and a reference case are
//! measured identically.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use powerio::PioValue;
use tellegen::ir::{balanced_module, deserialize_module, serialize_module};
use tellegen::{Problem, Study};
use tellegen_synth::request::CalibrationTargets;
use tellegen_synth::{Generator, Places, SynthRequest};

pub const GENERATE_USAGE: &str = "usage: tellegen generate --request PATH --out DIR [--places PATH] [--osm PATH] [--export FORMAT[,FORMAT...]] [--gridfm]";
pub const STATS_USAGE: &str =
    "usage: tellegen stats [--case PATH] [--targets PATH]   (a PowerIO module on stdin without --case)";

/// Parsed `generate` arguments.
#[derive(Debug, PartialEq)]
pub struct GenerateArgs {
    pub request: PathBuf,
    pub out: PathBuf,
    pub places: Option<PathBuf>,
    pub osm: Option<PathBuf>,
    pub exports: Vec<String>,
    pub gridfm: bool,
}

pub fn parse_generate_args(args: &[String]) -> Result<GenerateArgs, String> {
    let mut request = None;
    let mut out = None;
    let mut places = None;
    let mut osm = None;
    let mut exports = Vec::new();
    let mut gridfm = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value\n\n{GENERATE_USAGE}"))
        };
        match arg.as_str() {
            "--request" => request = Some(PathBuf::from(value("--request")?)),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--places" => places = Some(PathBuf::from(value("--places")?)),
            "--osm" => osm = Some(PathBuf::from(value("--osm")?)),
            "--export" => exports.extend(
                value("--export")?
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            ),
            "--gridfm" => gridfm = true,
            other => return Err(format!("unknown argument {other:?}\n\n{GENERATE_USAGE}")),
        }
    }
    Ok(GenerateArgs {
        request: request.ok_or_else(|| format!("--request is required\n\n{GENERATE_USAGE}"))?,
        out: out.ok_or_else(|| format!("--out is required\n\n{GENERATE_USAGE}"))?,
        places,
        osm,
        exports,
        gridfm,
    })
}

/// Milliseconds since the first call: the stage clock of a native session.
fn clock_ms() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn write(dir: &Path, name: &str, text: &str, files: &mut Vec<String>) -> Result<(), String> {
    std::fs::write(dir.join(name), text).map_err(|e| format!("cannot write {name}: {e}"))?;
    files.push(name.to_owned());
    Ok(())
}

/// Generate a grid and write `network.pio.json`, `report.json`,
/// `network.geo.json`, each requested export and the GridFM document into the
/// output directory. Returns a JSON summary. `cancel` stops the run between
/// stages and inside a DC OPF.
pub fn generate_command(
    args: &GenerateArgs,
    cancel: Option<&dyn Fn(Arc<std::sync::atomic::AtomicBool>)>,
) -> Result<String, String> {
    let request: SynthRequest = serde_json::from_str(&read_text(&args.request)?)
        .map_err(|e| format!("{}: {e}", args.request.display()))?;
    let places = match &args.places {
        Some(path) => {
            let bytes =
                std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            Places::parse(&bytes).map_err(|e| e.to_string())?
        }
        None => Places::embedded().map_err(|e| e.to_string())?,
    };
    let osm = args.osm.as_deref().map(read_text).transpose()?;
    let mut session = Generator::new(request, Arc::new(places), osm).map_err(|e| e.to_string())?;
    session.set_clock(clock_ms);
    if let Some(install) = cancel {
        install(session.cancel_flag());
    }
    while !session.is_done() {
        session.step().map_err(|e| e.to_string())?;
    }
    let gridfm = if args.gridfm {
        Some(session.export_gridfm().map_err(|e| e.to_string())?)
    } else {
        None
    };
    let (module, report) = session.into_parts().map_err(|e| e.to_string())?;

    std::fs::create_dir_all(&args.out)
        .map_err(|e| format!("cannot create {}: {e}", args.out.display()))?;
    let mut files = Vec::new();
    let geo = module
        .value()
        .to_geo_layer()
        .to_geojson_checked()
        .map_err(|e| e.to_string())?;
    let study = if args.exports.is_empty() {
        None
    } else {
        Some(Study::from_module(module.clone(), Problem::DcOpf)?)
    };
    let pio = serialize_module(&module.map_value(PioValue::BalancedNetwork))?;
    write(&args.out, "network.pio.json", &pio, &mut files)?;
    write(
        &args.out,
        "report.json",
        &serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        &mut files,
    )?;
    write(&args.out, "network.geo.json", &geo, &mut files)?;
    if let Some(study) = &study {
        for format in &args.exports {
            let exported = study
                .export(format)
                .map_err(|e| format!("export {format}: {e}"))?;
            write(
                &args.out,
                &format!("network-{format}.{}", exported.extension),
                &exported.text,
                &mut files,
            )?;
        }
    }
    if let Some(text) = gridfm {
        write(&args.out, "network.gridfm.json", &text, &mut files)?;
    }
    let summary = serde_json::json!({
        "out": args.out.display().to_string(),
        "files": files,
        "request_sha256": report.provenance.request_sha256,
        "buses": report.stats.as_ref().map(|s| s.n_buses),
        "branches": report.stats.as_ref().map(|s| s.n_branches),
        "dc_opf": report.dc_opf.as_ref().map(|d| d.status),
        "ac_pf": report.ac_pf.as_ref().map(|a| a.status),
        "calibration": report.calibration.as_ref().map(|c| c.status),
        "warnings": report.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(),
    });
    serde_json::to_string(&summary).map_err(|e| e.to_string())
}

/// The structural statistics of a stored module, compared with the reference
/// ranges (`targets`, or the published ones).
pub fn stats_text(module_json: &str, targets: Option<&str>) -> Result<String, String> {
    stats_of(balanced_module(deserialize_module(module_json)?)?, targets)
}

/// The structural statistics of any case file PowerIO reads (MATPOWER, PSS/E,
/// PowerWorld, ...), its format taken from the file name.
pub fn stats_case(path: &Path, targets: Option<&str>) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "case".to_owned());
    let source = powerio::Source::from_memory(name, bytes).map_err(|e| e.to_string())?;
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
    stats_of(balanced_module(module)?, targets)
}

fn stats_of(
    module: powerio::PioModule<powerio::BalancedNetwork>,
    targets: Option<&str>,
) -> Result<String, String> {
    let targets: CalibrationTargets = match targets {
        Some(text) => serde_json::from_str(text).map_err(|e| format!("targets: {e}"))?,
        None => CalibrationTargets::published(),
    };
    let mut stats = tellegen_synth::stats::measure(module.value(), None);
    tellegen_synth::stats::compare(&mut stats, &targets);
    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

/// `(case, targets)` paths of a `stats` invocation.
pub fn parse_stats_args(args: &[String]) -> Result<(Option<PathBuf>, Option<PathBuf>), String> {
    let mut case = None;
    let mut targets = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let value = it
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| STATS_USAGE.to_owned())?;
        match arg.as_str() {
            "--case" => case = Some(value),
            "--targets" => targets = Some(value),
            _ => return Err(STATS_USAGE.to_owned()),
        }
    }
    Ok((case, targets))
}

/// Install a Ctrl-C handler that sets the session's cancel flag.
pub fn ctrlc_cancel(flag: Arc<std::sync::atomic::AtomicBool>) {
    // A second handler in one process is refused; the first one stays in force.
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn generate_arguments() {
        let parsed = parse_generate_args(&strings(&[
            "--request",
            "r.json",
            "--out",
            "out",
            "--export",
            "matpower,psse35",
            "--export",
            "pandapower-json",
            "--gridfm",
        ]))
        .unwrap();
        assert_eq!(parsed.request, PathBuf::from("r.json"));
        assert_eq!(
            parsed.exports,
            vec!["matpower", "psse35", "pandapower-json"]
        );
        assert!(parsed.gridfm && parsed.places.is_none());
        assert!(parse_generate_args(&strings(&["--out", "x"])).is_err());
        assert!(
            parse_generate_args(&strings(&["--request", "r", "--out", "o", "--bogus"])).is_err()
        );
        assert!(parse_generate_args(&strings(&["--request"])).is_err());
        assert_eq!(parse_stats_args(&strings(&[])).unwrap(), (None, None));
        assert_eq!(
            parse_stats_args(&strings(&["--case", "c.m", "--targets", "t.json"])).unwrap(),
            (Some(PathBuf::from("c.m")), Some(PathBuf::from("t.json")))
        );
        assert!(parse_stats_args(&strings(&["--nope", "x"])).is_err());
        assert!(parse_stats_args(&strings(&["--case"])).is_err());
    }

    #[test]
    fn generate_writes_every_artifact_and_stats_reads_it_back() {
        let dir = std::env::temp_dir().join(format!("tellegen-generate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request = dir.join("request.json");
        std::fs::write(
            &request,
            serde_json::json!({
                "region": {"geometry": {"type": "Polygon", "coordinates": [[[-2.0, 5.0], [1.0, 5.0], [1.0, 8.0], [-2.0, 8.0], [-2.0, 5.0]]]}},
                "target_bus_count": 30
            })
            .to_string(),
        )
        .unwrap();
        let out = dir.join("out");
        let args = GenerateArgs {
            request,
            out: out.clone(),
            places: None,
            osm: None,
            exports: vec!["matpower".into(), "psse35".into()],
            gridfm: true,
        };
        let summary: serde_json::Value =
            serde_json::from_str(&generate_command(&args, None).unwrap()).unwrap();
        assert_eq!(summary["buses"], 30);
        assert_eq!(summary["dc_opf"], "optimal");
        for name in [
            "network.pio.json",
            "report.json",
            "network.geo.json",
            "network-matpower.m",
            "network-psse35.raw",
            "network.gridfm.json",
        ] {
            assert!(out.join(name).exists(), "{name}");
        }
        let stats: serde_json::Value = serde_json::from_str(
            &stats_text(
                &std::fs::read_to_string(out.join("network.pio.json")).unwrap(),
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap())
                .unwrap();
        assert_eq!(stats["n_buses"], 30);
        assert_eq!(stats["n_branches"], report["stats"]["n_branches"]);
        assert_eq!(stats["avg_degree"], report["stats"]["avg_degree"]);
        // The MATPOWER export reads back through the case route.
        let from_case: serde_json::Value =
            serde_json::from_str(&stats_case(&out.join("network-matpower.m"), None).unwrap())
                .unwrap();
        assert_eq!(from_case["n_buses"], 30);
        assert_eq!(from_case["n_branches"], stats["n_branches"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
