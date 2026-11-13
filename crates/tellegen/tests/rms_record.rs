//! What an RMS run records: every channel by default, or a watch set at the
//! output step, map frames at their own step and state checkpoints from which
//! any other channel is replayed. The WECC 240-bus case with a bus fault and
//! its clearing.

#![cfg(feature = "sensitivity")]

use std::path::Path;

use tellegen::rms::{
    Checkpoint, LoadModel, RecordSpec, RmsEvent, RmsRequest, RmsResult, RmsSimulation,
};

/// The recorder tests run on the WECC 240-bus case that ships with the code.
fn sim() -> RmsSimulation {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240");
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    RmsSimulation::new(net, &dyr, LoadModel::ConstantImpedance).unwrap()
}

fn request(record: Option<RecordSpec>) -> RmsRequest {
    RmsRequest {
        t_end: 3.0,
        dt: 0.005,
        output_step: 0.005,
        load_model: LoadModel::ConstantImpedance,
        events: vec![
            RmsEvent::BusFault {
                time: 0.5,
                bus: 1101,
                r: 0.0,
                x: 0.05,
            },
            RmsEvent::ClearFault {
                time: 0.6,
                bus: 1101,
            },
        ],
        small_signal: false,
        record,
    }
}

#[test]
fn watch_set_frames_and_replay_reproduce_the_full_recording() {
    let sim = sim();
    let full = sim.run(&request(None)).unwrap();
    assert!(full.frames.is_none() && full.checkpoints.is_empty());
    let n = full.time.len();
    assert_eq!(n, 601);

    // A watch set: exactly the matching channels, the same values.
    let spec = RecordSpec {
        channels: vec!["omega:".into(), "vm:1101".into(), "p_from:1101-1401".into()],
        frames: vec!["vm:".into(), "va:".into()],
        frame_step: 0.1,
        checkpoint_step: 0.5,
    };
    let watched = sim.run(&request(Some(spec.clone()))).unwrap();
    let keys: Vec<&String> = watched.channels.keys().collect();
    let want: Vec<&String> = full
        .channels
        .keys()
        .filter(|k| k.starts_with("omega:") || *k == "vm:1101" || *k == "p_from:1101-1401")
        .collect();
    assert_eq!(keys, want);
    assert_eq!(watched.time, full.time);
    for (k, v) in &watched.channels {
        assert_eq!(v, &full.channels[k], "{k}");
    }

    // The frames: every 0.1 s, every bus, the same values.
    let frames = watched.frames.as_ref().unwrap();
    assert_eq!(frames.time.len(), 31);
    assert!((frames.time[1] - 0.1).abs() < 1e-9 && (frames.time[30] - 3.0).abs() < 1e-9);
    assert_eq!(frames.channels.len(), 2 * full.buses.len());
    for (k, v) in &frames.channels {
        for (j, &t) in frames.time.iter().enumerate() {
            let i = (t / 0.005).round() as usize;
            assert_eq!(v[j], full.channels[k][i], "{k} at {t}");
        }
    }

    // Checkpoints every 0.5 s, the first at the start.
    let cps: Vec<f64> = watched.checkpoints.iter().map(|c| c.t).collect();
    assert_eq!(cps.len(), 7);
    assert!(cps
        .iter()
        .enumerate()
        .all(|(i, t)| (t - 0.5 * i as f64).abs() < 1e-9));

    // Replay channels nobody watched, from the checkpoint at 1.0 s (after the
    // fault was cleared) and from the start (before the fault).
    let on_demand = RecordSpec {
        channels: vec!["efd:".into(), "q_to:1101-1401".into(), "va:1401".into()],
        ..RecordSpec::default()
    };
    for cp in [&watched.checkpoints[2], &watched.checkpoints[0]] {
        let replay: RmsResult = sim.replay(&request(Some(on_demand.clone())), cp).unwrap();
        let i0 = (cp.t / 0.005).round() as usize;
        assert_eq!(replay.time.len(), n - i0);
        assert!((replay.time[0] - cp.t).abs() < 1e-12);
        assert!(!replay.channels.is_empty());
        // The replay is the run, sample for sample.
        for (k, v) in &replay.channels {
            assert_eq!(v.as_slice(), &full.channels[k][i0..], "{k} from {} s", cp.t);
        }
    }

    // A checkpoint of another system is refused.
    let wrong = Checkpoint {
        past: Vec::new(),
        t: 1.0,
        x: vec![0.0; 3],
        solver: None,
    };
    assert!(sim.replay(&request(None), &wrong).is_err());
}

#[test]
fn live_watch_set_and_snapshot() {
    let sim = sim();
    let mut full = sim.live(0.005, 0.005);
    let mut live = sim.live(0.005, 0.005);
    live.set_record(Some(RecordSpec {
        channels: vec!["omega:".into()],
        frames: vec!["vm:".into()],
        frame_step: 0.05,
        checkpoint_step: 0.0,
    }));
    let a = full.advance(0.2).unwrap();
    let b = live.advance(0.2).unwrap();
    assert!(b.channels.keys().all(|k| k.starts_with("omega:")));
    for (k, v) in &b.channels {
        let w = &a.channels[k];
        assert_eq!(v.as_slice(), &w[w.len() - v.len()..], "{k}");
    }
    let frames = b.frames.as_ref().unwrap();
    assert!(frames.time.len() >= 4 && frames.channels.keys().all(|k| k.starts_with("vm:")));
    // Any channel at the current time, on demand.
    let snap = live.snapshot(&["p_from:".into(), "efd:1032#C".into()]);
    assert!(snap.contains_key("efd:1032#C") && snap.keys().any(|k| k.starts_with("p_from:")));
    let all = live.snapshot(&[]);
    assert_eq!(all.len(), a.channels.len());
    for (k, v) in &all {
        assert!((v - a.channels[k].last().unwrap()).abs() < 1e-12, "{k}");
    }
}

/// The columns of a live session are its chunks' samples, bit for bit, and
/// name their channels once per layout.
#[test]
fn live_columns_are_the_chunks_samples() {
    let sim = sim();
    let spec = RecordSpec {
        channels: vec!["omega:".into(), "pe:".into()],
        frames: vec!["vm:".into(), "va:".into()],
        frame_step: 0.02,
        checkpoint_step: 0.0,
    };
    let mut chunks = sim.live(0.005, 0.005);
    let mut columns = sim.live(0.005, 0.005);
    chunks.set_record(Some(spec.clone()));
    columns.set_record(Some(spec));
    let fault = RmsEvent::BusFault {
        time: 0.0,
        bus: 1101,
        r: 0.0,
        x: 0.05,
    };
    let (mut layout, mut keys, mut frame_keys) = (0u32, Vec::<String>::new(), Vec::<String>::new());
    let mut named = 0;
    for slice in 0..12 {
        if slice == 4 {
            chunks.apply(fault.clone()).unwrap();
            columns.apply(fault.clone()).unwrap();
        }
        if slice == 8 {
            // A new watch set: the keys come again, under a new layout.
            let other = RecordSpec {
                channels: vec!["delta:".into()],
                frames: vec!["vm:".into()],
                frame_step: 0.01,
                checkpoint_step: 0.0,
            };
            chunks.set_record(Some(other.clone()));
            columns.set_record(Some(other));
        }
        let a = chunks.advance(0.04).unwrap();
        let b = columns.advance_columns(0.04, layout).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&b.meta).unwrap();
        let list = |k: &str| -> Vec<f64> { serde_json::from_value(meta[k].clone()).unwrap() };
        if let Some(k) = meta.get("keys") {
            keys = serde_json::from_value(k.clone()).unwrap();
            frame_keys = serde_json::from_value(meta["frame_keys"].clone()).unwrap();
            layout = u32::try_from(meta["layout"].as_u64().unwrap()).unwrap();
            named += 1;
        }
        assert_eq!(meta["layout"].as_u64().unwrap(), u64::from(layout));
        assert_eq!(meta["t"].as_f64().unwrap(), a.t);
        assert_eq!(meta["events"].as_array().unwrap().len(), a.events.len());
        let time = list("time");
        assert_eq!(time, a.time);
        assert_eq!(keys.len(), a.channels.len());
        assert_eq!(b.values.len(), keys.len() * time.len());
        for (j, k) in keys.iter().enumerate() {
            for (s, v) in a.channels[k].iter().enumerate() {
                assert_eq!(v.to_bits(), b.values[s * keys.len() + j].to_bits(), "{k}");
            }
        }
        let frames = a.frames.unwrap_or_default();
        let frame_time = list("frame_time");
        assert_eq!(frame_time, frames.time);
        assert_eq!(b.frame_values.len(), frame_keys.len() * frame_time.len());
        if !frame_time.is_empty() {
            assert_eq!(frame_keys.len(), frames.channels.len());
            for (j, k) in frame_keys.iter().enumerate() {
                for (s, v) in frames.channels[k].iter().enumerate() {
                    assert_eq!(
                        v.to_bits(),
                        b.frame_values[s * frame_keys.len() + j].to_bits(),
                        "{k}"
                    );
                }
            }
        }
    }
    // Named at the first slice and after the new watch set, not in between.
    assert_eq!(named, 2);
    assert!(keys.iter().all(|k| k.starts_with("delta:")));
}

/// A live session takes a new step while it runs: the samples keep their
/// times and the run stays the same trajectory within the step's accuracy.
#[test]
fn live_step_changes_while_running() {
    let sim = sim();
    let mut fixed = sim.live(0.005, 0.02);
    let mut changed = sim.live(0.005, 0.02);
    assert_eq!(changed.step(), 0.005);
    let fault = RmsEvent::BusFault {
        time: 0.0,
        bus: 1101,
        r: 0.0,
        x: 0.05,
    };
    let clear = RmsEvent::ClearFault {
        time: 0.0,
        bus: 1101,
    };
    let mut a = fixed.advance(0.2).unwrap();
    let mut b = changed.advance(0.2).unwrap();
    assert_eq!(a.channels, b.channels);
    assert!(changed.set_step(0.0).is_err() && changed.set_step(f64::NAN).is_err());
    // One step per sample; a step above the output step is the output step.
    changed.set_step(0.02).unwrap();
    assert_eq!(changed.step(), 0.02);
    changed.set_step(0.5).unwrap();
    assert_eq!(changed.step(), 0.02);
    for event in [Some(fault), Some(clear), None] {
        if let Some(e) = event {
            fixed.apply(e.clone()).unwrap();
            changed.apply(e).unwrap();
        }
        a = fixed.advance(0.1).unwrap();
        b = changed.advance(0.1).unwrap();
        assert_eq!(a.time, b.time);
    }
    // Back to the fine step: the two runs end within the coarse steps' error.
    changed.set_step(0.0025).unwrap();
    assert_eq!(changed.step(), 0.0025);
    a = fixed.advance(1.0).unwrap();
    b = changed.advance(1.0).unwrap();
    assert_eq!(a.time, b.time);
    for (k, v) in a.channels.iter().filter(|(k, _)| k.starts_with("omega:")) {
        let w = &b.channels[k];
        let worst = v
            .iter()
            .zip(w)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(worst < 2e-4, "{k}: {worst}");
    }
}
