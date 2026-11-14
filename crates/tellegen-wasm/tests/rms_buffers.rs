//! The packing of a browser RMS run (`tellegen_wasm::rms::run_buffers`), on
//! the WECC 240 case the web app ships: the buffers hold every key's samples,
//! the recorded channels are the engine's bit for bit, the bus angles give way
//! to the bus frequencies, and a run too long for the transfer is coarsened
//! with a note.

use std::path::PathBuf;

use tellegen::rms::{LoadModel, RecordSpec, RmsEvent, RmsOptions, RmsRequest, ScreeningCriteria};
use tellegen_wasm::rms::{flat_check, run_buffers, RunRequest};

fn wecc240() -> Option<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tellegen/tests/data/rms_examples/wecc240");
    let raw = std::fs::read(dir.join("case.raw")).ok()?;
    let dyr = std::fs::read_to_string(dir.join("case.dyr")).ok()?;
    let module = powerio::parse(powerio::Source::from_memory("case.raw", raw).unwrap()).unwrap();
    Some((tellegen::ir::serialize_module(&module).unwrap(), dyr))
}

fn request(t_end: f64) -> RmsRequest {
    RmsRequest {
        t_end,
        dt: 0.005,
        output_step: 0.01,
        events: vec![
            RmsEvent::BusFault {
                time: 0.5,
                bus: 1101,
                r: 0.0,
                x: 0.01,
            },
            RmsEvent::ClearFault {
                time: 0.6,
                bus: 1101,
            },
        ],
        ..RmsRequest::default()
    }
}

#[test]
fn a_display_run_packs_the_engines_channels() {
    let Some((module, dyr)) = wecc240() else {
        eprintln!("skipped: the WECC 240 case data is not here");
        return;
    };
    let sim = tellegen::rms::prepare_module_json(
        &module,
        &dyr,
        LoadModel::PowerFlow,
        RmsOptions::default(),
    )
    .unwrap();
    let mut seen = Vec::new();
    let b = run_buffers(
        &sim,
        RunRequest {
            request: request(1.5),
            criteria: ScreeningCriteria::default(),
        },
        &mut |t| seen.push(t),
    )
    .unwrap();
    let n = b.time.len();
    assert_eq!(b.values.len(), b.meta.keys.len() * n);
    assert_eq!(n, 151);
    // Progress rises to the end.
    assert!(seen.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(seen.last().copied(), Some(1.5));
    // No bus angles; a frequency per bus and the COI series instead.
    assert!(!b.meta.keys.iter().any(|k| k.starts_with("va:")));
    let f_count = b.meta.keys.iter().filter(|k| k.starts_with("f:")).count();
    assert_eq!(f_count, b.meta.buses.len());
    for k in ["coi:f", "coi:omega", "coi:delta"] {
        assert!(b.meta.keys.iter().any(|x| x == k), "{k}");
    }
    // The recorded channels equal a direct run's, bit for bit.
    let direct = sim
        .run(&RmsRequest {
            record: Some(RecordSpec::display()),
            ..request(1.5)
        })
        .unwrap();
    assert_eq!(direct.time, b.time);
    for (i, key) in b.meta.keys.iter().enumerate() {
        let Some(series) = direct.channels.get(key) else {
            continue;
        };
        let packed = &b.values[i * n..(i + 1) * n];
        assert!(
            series
                .iter()
                .zip(packed)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "{key}"
        );
    }
    assert!(b.meta.screening.frequency.nominal_hz == 60.0);
    assert!(b.meta.request.record.is_none());
}

#[test]
fn a_long_run_is_coarsened_and_says_so() {
    let Some((module, dyr)) = wecc240() else {
        return;
    };
    let sim = tellegen::rms::prepare_module_json(
        &module,
        &dyr,
        LoadModel::PowerFlow,
        RmsOptions::default(),
    )
    .unwrap();
    // 1,100 channels at 1 ms over 4 s: 4.4e6 values, past the limit.
    let mut req = request(4.0);
    req.dt = 0.001;
    req.output_step = 0.001;
    let b = run_buffers(
        &sim,
        RunRequest {
            request: req,
            criteria: ScreeningCriteria::default(),
        },
        &mut |_| {},
    )
    .unwrap();
    assert!(b.meta.output_step > 0.001);
    let ratio = b.meta.output_step / 0.001;
    assert!((ratio - ratio.round()).abs() < 1e-9, "{ratio}");
    assert!(b.values.len() as f64 <= tellegen_wasm::rms::MAX_VALUES * 1.01);
    assert!(
        b.meta.notes.iter().any(|n| n.contains("instead of every")),
        "{:?}",
        b.meta.notes
    );
}

#[test]
fn the_case_rests_without_events() {
    let Some((module, dyr)) = wecc240() else {
        return;
    };
    let sim = tellegen::rms::prepare_module_json(
        &module,
        &dyr,
        LoadModel::PowerFlow,
        RmsOptions::default(),
    )
    .unwrap();
    let c = flat_check(&sim, 2.0).unwrap();
    assert!(c.max_speed_drift < 1e-6, "{c:?}");
    assert!(c.max_voltage_drift < 1e-6, "{c:?}");
    let i = sim.inspection();
    assert!(i.generators.len() > 100, "{}", i.generators.len());
    assert_eq!(i.plants.len(), 37);
    assert!(i.initial_residual < 1e-6);
    assert!(i.reference_unit.is_some());
}
