//! DEBUG (debug/pf-worst-case): prepare an RMS case natively the way the browser
//! engine does (PSS/E RAW -> PowerIO module -> `prepare_module_json`) and print the
//! notes, or the full error with the AC power flow's worst-case report.
//!
//! `cargo run -p tellegen --release --example rms_init_debug --features rms-experimental -- <dir>`
//! where `<dir>` holds `case.raw`, `case.dyr` and optionally `case.models.json`.

use std::path::Path;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: rms_init_debug <case dir>");
    let dir = Path::new(&dir);
    let raw = std::fs::read(dir.join("case.raw")).expect("case.raw");
    let dyr = std::fs::read_to_string(dir.join("case.dyr")).expect("case.dyr");
    // The app passes {"dyr", "models"} when the case has controller graphs.
    let dyn_text = match std::fs::read_to_string(dir.join("case.models.json")) {
        Ok(models) => {
            let v: serde_json::Value = serde_json::from_str(&models).expect("models json");
            serde_json::json!({ "dyr": dyr, "models": v["models"] }).to_string()
        }
        Err(_) => dyr,
    };
    let info = powerio::resolve_format("raw").expect("raw format");
    let source = powerio::Source::from_memory("<case>", raw)
        .expect("source")
        .with_format(powerio::FormatId::new(info.token).expect("format id"));
    let module = powerio::parse(source).expect("parse");
    let module = tellegen::ir::balanced_module(module).expect("balanced");
    let module_json = tellegen::ir::serialize_module(&module).expect("serialize");
    let t = std::time::Instant::now();
    match tellegen::rms::prepare_module_json(
        &module_json,
        &dyn_text,
        tellegen::rms::LoadModel::default(),
        tellegen::rms::RmsOptions::default(),
    ) {
        Ok(sim) => {
            println!("prepared in {:.2} s", t.elapsed().as_secs_f64());
            for note in sim.inspection().notes {
                println!("note: {note}");
            }
            // The operating point: the first sample of every voltage, unit and
            // branch channel, as `<dir>/snap.json` ({"vm:bus": pu, ...}).
            let request = tellegen::rms::RmsRequest {
                t_end: 0.005,
                dt: 0.005,
                output_step: 0.005,
                record: Some(tellegen::rms::RecordSpec {
                    channels: [
                        "vm:", "va:", "pe:", "qe:", "p_from:", "q_from:", "p_to:", "q_to:",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let res = sim.run(&request).expect("operating point run");
            let snap: serde_json::Map<String, serde_json::Value> = res
                .channels
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v[0])))
                .collect();
            std::fs::write(
                dir.join("snap.json"),
                serde_json::Value::Object(snap).to_string(),
            )
            .expect("write snap.json");
            println!("wrote {}", dir.join("snap.json").display());
        }
        Err(e) => {
            println!("FAILED after {:.2} s:\n{e}", t.elapsed().as_secs_f64());
            std::process::exit(1);
        }
    }
}
