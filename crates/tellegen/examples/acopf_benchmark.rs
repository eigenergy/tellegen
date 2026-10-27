//! Emit reproducible JSONL records for MATPOWER cases and optional frozen objectives.
use std::{collections::BTreeMap, fs, time::Instant};

use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    provenance: String,
    /// SHA-256 of the exact input bytes -> independently obtained objective.
    objectives: BTreeMap<String, f64>,
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let reference_path = args
        .next()
        .ok_or("usage: acopf_benchmark REFERENCE.json|- CASE.m ...")?;
    let reference = if reference_path == "-" {
        None
    } else {
        let reference: Reference = serde_json::from_slice(&fs::read(reference_path)?)?;
        if reference.provenance.trim().is_empty() || reference.objectives.is_empty() {
            return Err("reference must state provenance and at least one objective".into());
        }
        Some(reference)
    };
    let cases: Vec<_> = args.collect();
    if cases.is_empty() {
        return Err("at least one MATPOWER case is required".into());
    }
    let artifact_sha256 = digest(&fs::read(std::env::current_exe()?)?);
    let mut failed = false;
    for path in cases {
        let started = Instant::now();
        let mut record = json!({"path": path, "artifact_sha256": artifact_sha256});
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let bytes = fs::read(&path)?;
            let hash = digest(&bytes);
            record["input_sha256"] = hash.clone().into();
            let expected = reference
                .as_ref()
                .map(|reference| {
                    record["reference_provenance"] = reference.provenance.clone().into();
                    reference
                        .objectives
                        .get(&hash)
                        .copied()
                        .ok_or("input checksum missing from reference")
                })
                .transpose()?;
            let parse_started = Instant::now();
            let module = powerio::parse(powerio::Source::from_memory("case.m", bytes)?)?;
            let network = tellegen::ir::balanced_module(module)?;
            let instance = powerio_prob::AcOpfInstance::from_network(network.value().clone())?;
            record["parse_ms"] = (parse_started.elapsed().as_secs_f64() * 1000.0).into();
            let solve_started = Instant::now();
            let result = tellegen::solve_ac_opf_instance(
                &instance,
                &tellegen::SolveRequest {
                    formulation: tellegen::Problem::Acopf,
                    ..Default::default()
                },
            );
            // Includes model construction and independent validation, excludes input parsing/JSON.
            record["solve_ms"] = (solve_started.elapsed().as_secs_f64() * 1000.0).into();
            let response = result?;
            record["objective"] = serde_json::to_value(response.objective)?;
            record["status"] = serde_json::to_value(response.status)?;
            record["iterations"] = serde_json::to_value(response.iterations)?;
            if let Some(expected) = expected {
                let actual = record["objective"].as_f64().ok_or("missing objective")?;
                let relative_error = (actual - expected).abs() / expected.abs().max(1.0);
                record["reference_objective"] = expected.into();
                record["relative_objective_error"] = relative_error.into();
                if !relative_error.is_finite() || relative_error > 1e-4 {
                    return Err(
                        "objective differs from reference by more than 1e-4 relative".into(),
                    );
                }
            }
            Ok(())
        })();
        record["total_elapsed_ms"] = (started.elapsed().as_secs_f64() * 1000.0).into();
        if let Err(error) = result {
            failed = true;
            record["error"] = error.to_string().into();
        }
        println!("{record}");
    }
    if failed {
        return Err("one or more benchmark cases failed; see JSONL records".into());
    }
    Ok(())
}
