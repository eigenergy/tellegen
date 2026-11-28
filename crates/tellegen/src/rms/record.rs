//! Recording a run: the channel catalog (bus voltages, machine states,
//! controller signals, powers, branch flows), the samples of the selected
//! channels at the output steps, and the map frames at their own step.
//!
//! @block Recording

use std::collections::BTreeMap;

use num_complex::Complex;

use super::models;
use super::system::{DynSystem, GenOut};
use super::{RecordSpec, RmsSeries};

/// Where a channel's value comes from.
#[derive(Clone, Copy, Debug)]
pub(super) enum Source {
    Vm(usize),
    Va(usize),
    Delta(usize),
    Omega(usize),
    Efd(usize),
    Tm(usize),
    Vs(usize),
    Pe(usize),
    Qe(usize),
    /// Device, index into its graph signals.
    Sig(usize, usize),
    /// Held unit (constant current or simulated plant): active, reactive
    /// power out, system base.
    HeldPe(usize),
    HeldQe(usize),
    /// Simulated plant, local state index.
    PlantState(usize, usize),
    /// Power drawn by the loads of a bus (not its held units), system base.
    Pl(usize),
    Ql(usize),
    PFrom(usize),
    QFrom(usize),
    PTo(usize),
    QTo(usize),
    /// A channel the caller derives at each sample (a live session's COI and
    /// bus frequencies), index into its values.
    Derived(usize),
}

/// One recorded set of channels: indices into the catalog, its step and the
/// samples (sample-major).
struct Group {
    sel: Vec<usize>,
    /// Record every `step` seconds; 0: at every output sample.
    step: f64,
    next: f64,
    time: Vec<f64>,
    values: Vec<f64>,
    /// Which inputs the selection needs.
    outs: bool,
    sig: bool,
}

/// The recorder of a run or a live session: the catalog of every channel,
/// the selected groups and their samples.
// @block The recorder
pub(super) struct Recorder {
    /// Further channels recorded at every output sample (aligned with the
    /// main group).
    channels: BTreeMap<String, Vec<f64>>,
    gen_keys: Vec<String>,
    branch_keys: Vec<String>,
    outs: Vec<GenOut>,
    spec: Option<RecordSpec>,
    /// Every channel, built at the first sample (graph signal names need
    /// the state): its key and source.
    pub(super) catalog: Vec<(String, Source)>,
    /// Counts the catalogs built: the channels of the groups keep their
    /// order while it stays ([`RmsLive::advance_columns`]).
    pub(super) layout: u32,
    main: Group,
    frames: Option<Group>,
    /// Channels derived outside the recorder: their keys and the values of
    /// the sample being recorded ([`Recorder::set_derived`]).
    derived: Option<(Vec<String>, Vec<f64>)>,
}

impl Recorder {
    pub(super) fn new(sys: &DynSystem, spec: Option<RecordSpec>) -> Self {
        let gen_keys = sys
            .devices
            .iter()
            .map(|d| {
                if d.machine_id == "1" {
                    d.bus_number.to_string()
                } else {
                    format!("{}#{}", d.bus_number, d.machine_id)
                }
            })
            .collect();
        // Lines and transformers keep separate PSS/E circuit-id spaces, so a
        // line and a transformer between the same buses can share `from-to`;
        // a repeated key takes the branch index, so every channel is one series.
        let mut seen = std::collections::BTreeSet::new();
        let branch_keys = sys
            .branch_keys
            .iter()
            .enumerate()
            .map(|(e, (f, t, c))| {
                let key = if c.trim() == "1" {
                    format!("{f}-{t}")
                } else {
                    format!("{f}-{t}#{}", c.trim())
                };
                if seen.insert(key.clone()) {
                    key
                } else {
                    format!("{key}@{e}")
                }
            })
            .collect();
        let group = |step: f64| Group {
            sel: Vec::new(),
            step,
            next: 0.0,
            time: Vec::new(),
            values: Vec::new(),
            outs: false,
            sig: false,
        };
        let frames = spec
            .as_ref()
            .filter(|s| !s.frames.is_empty() && s.frame_step > 0.0)
            .map(|s| group(s.frame_step));
        Self {
            channels: BTreeMap::new(),
            gen_keys,
            branch_keys,
            outs: Vec::new(),
            spec,
            catalog: Vec::new(),
            layout: 0,
            main: group(0.0),
            frames,
            derived: None,
        }
    }

    /// The catalog of every channel and the groups' selections.
    // @block Channel catalog
    pub(super) fn build_catalog(&mut self, sys: &DynSystem, x: &[f64]) {
        let n = sys.n;
        let mut cat: Vec<(String, Source)> = Vec::new();
        for (b, &number) in sys.bus_numbers.iter().enumerate() {
            cat.push((format!("vm:{number}"), Source::Vm(b)));
            cat.push((format!("va:{number}"), Source::Va(b)));
        }
        for (k, d) in sys.devices.iter().enumerate() {
            let key = &self.gen_keys[k];
            cat.push((format!("delta:{key}"), Source::Delta(k)));
            cat.push((format!("omega:{key}"), Source::Omega(k)));
            if !matches!(d.machine, models::Machine::Classic { .. }) {
                cat.push((format!("efd:{key}"), Source::Efd(k)));
            }
            cat.push((format!("tm:{key}"), Source::Tm(k)));
            if !matches!(d.pss, models::Stabilizer::None) {
                cat.push((format!("vs:{key}"), Source::Vs(k)));
            }
            cat.push((format!("pe:{key}"), Source::Pe(k)));
            cat.push((format!("qe:{key}"), Source::Qe(k)));
            let (vr, vi) = (x[d.bus], x[n + d.bus]);
            for (j, (name, _)) in d
                .graph_signals(&x[d.offset..d.offset + d.n_states()], vr, vi)
                .into_iter()
                .enumerate()
            {
                cat.push((format!("sig:{name}:{key}"), Source::Sig(k, j)));
            }
        }
        for (k, h) in sys.held.iter().enumerate() {
            let key = if h.machine_id == "1" {
                h.bus_number.to_string()
            } else {
                format!("{}#{}", h.bus_number, h.machine_id)
            };
            cat.push((format!("pe:{key}"), Source::HeldPe(k)));
            cat.push((format!("qe:{key}"), Source::HeldQe(k)));
            if let Some(p) = &h.plant {
                for (j, name) in p.state_names().into_iter().enumerate() {
                    cat.push((format!("inv.{name}:{key}"), Source::PlantState(k, j)));
                }
            }
        }
        for (b, &number) in sys.bus_numbers.iter().enumerate() {
            if sys.loads[b].has_load() {
                cat.push((format!("pl:{number}"), Source::Pl(b)));
                cat.push((format!("ql:{number}"), Source::Ql(b)));
            }
        }
        if let Some((keys, _)) = &self.derived {
            for (i, key) in keys.iter().enumerate() {
                cat.push((key.clone(), Source::Derived(i)));
            }
        }
        for e in 0..sys.ac.m {
            let key = &self.branch_keys[e];
            cat.push((format!("p_from:{key}"), Source::PFrom(e)));
            cat.push((format!("q_from:{key}"), Source::QFrom(e)));
            cat.push((format!("p_to:{key}"), Source::PTo(e)));
            cat.push((format!("q_to:{key}"), Source::QTo(e)));
        }
        let select = |patterns: Option<&[String]>| -> Vec<usize> {
            match patterns {
                Some(p) if !p.is_empty() => (0..cat.len())
                    .filter(|&i| RecordSpec::matches(p, &cat[i].0))
                    .collect(),
                _ => (0..cat.len()).collect(),
            }
        };
        let needs = |sel: &[usize]| {
            let outs = sel.iter().any(|&i| {
                matches!(
                    cat[i].1,
                    Source::Efd(_) | Source::Tm(_) | Source::Vs(_) | Source::Pe(_) | Source::Qe(_)
                )
            });
            let sig = sel.iter().any(|&i| matches!(cat[i].1, Source::Sig(..)));
            (outs, sig)
        };
        self.main.sel = select(self.spec.as_ref().map(|s| s.channels.as_slice()));
        (self.main.outs, self.main.sig) = needs(&self.main.sel);
        if let (Some(g), Some(s)) = (self.frames.as_mut(), self.spec.as_ref()) {
            g.sel = select(Some(s.frames.as_slice()));
            (g.outs, g.sig) = needs(&g.sel);
        }
        self.catalog = cat;
        self.layout += 1;
    }

    /// Offer channels derived outside the recorder under `keys`; the caller
    /// gives their values before each sample ([`Self::set_derived`]).
    #[cfg(feature = "rms-experimental")]
    pub(super) fn with_derived(&mut self, keys: Vec<String>) {
        let n = keys.len();
        self.derived = Some((keys, vec![f64::NAN; n]));
        self.catalog.clear();
    }

    /// The derived channels' values at the sample about to be recorded.
    #[cfg(feature = "rms-experimental")]
    pub(super) fn set_derived(&mut self, values: &[f64]) {
        if let Some((_, v)) = &mut self.derived {
            v.copy_from_slice(values);
        }
    }

    /// Change what is recorded from the next sample on (a live session's
    /// watch set). Samples recorded so far stay in their groups.
    #[cfg(feature = "rms-experimental")]
    pub(super) fn set_spec(&mut self, spec: Option<RecordSpec>) {
        let step = spec
            .as_ref()
            .filter(|s| !s.frames.is_empty() && s.frame_step > 0.0)
            .map(|s| s.frame_step);
        self.spec = spec;
        match (step, self.frames.as_mut()) {
            (Some(st), Some(g)) => g.step = st,
            (Some(st), None) => {
                self.frames = Some(Group {
                    sel: Vec::new(),
                    step: st,
                    next: 0.0,
                    time: Vec::new(),
                    values: Vec::new(),
                    outs: false,
                    sig: false,
                });
            }
            (None, _) => self.frames = None,
        }
        // Rebuilt at the next sample, with the new selection; a group whose
        // selection changes starts its samples over.
        self.catalog.clear();
        self.main.time.clear();
        self.main.values.clear();
        if let Some(g) = &mut self.frames {
            g.time.clear();
            g.values.clear();
        }
    }

    /// Move out the samples recorded so far: the main channels and the
    /// frames.
    pub(super) fn take(&mut self) -> (Vec<f64>, BTreeMap<String, Vec<f64>>, Option<RmsSeries>) {
        let mut channels = std::mem::take(&mut self.channels);
        let time = std::mem::take(&mut self.main.time);
        let values = std::mem::take(&mut self.main.values);
        if !time.is_empty() {
            Self::unpack(
                &self.catalog,
                &self.main.sel,
                &values,
                time.len(),
                &mut channels,
            );
        }
        // No frame in this span: no frame series (a live chunk between frames).
        let frames = self.frames.as_mut().and_then(|g| {
            let t = std::mem::take(&mut g.time);
            let v = std::mem::take(&mut g.values);
            if t.is_empty() {
                return None;
            }
            let mut ch = BTreeMap::new();
            Self::unpack(&self.catalog, &g.sel, &v, t.len(), &mut ch);
            Some(RmsSeries {
                time: t,
                channels: ch,
            })
        });
        (time, channels, frames)
    }

    /// Move out the samples recorded so far as they were recorded: per
    /// group its times and its values, sample after sample in the order of
    /// [`Self::keys`] (nothing is copied per channel).
    #[cfg(feature = "rms-experimental")]
    pub(super) fn take_columns(&mut self) -> [(Vec<f64>, Vec<f64>); 2] {
        let main = (
            std::mem::take(&mut self.main.time),
            std::mem::take(&mut self.main.values),
        );
        let frames = self.frames.as_mut().map_or_else(Default::default, |g| {
            (std::mem::take(&mut g.time), std::mem::take(&mut g.values))
        });
        [main, frames]
    }

    /// The channel keys of the main group and of the frames, in the order
    /// of their values.
    #[cfg(feature = "rms-experimental")]
    pub(super) fn keys(&self) -> [Vec<&str>; 2] {
        let of = |sel: &[usize]| sel.iter().map(|&i| self.catalog[i].0.as_str()).collect();
        [
            of(&self.main.sel),
            self.frames.as_ref().map_or_else(Vec::new, |g| of(&g.sel)),
        ]
    }

    pub(super) fn unpack(
        catalog: &[(String, Source)],
        sel: &[usize],
        values: &[f64],
        samples: usize,
        out: &mut BTreeMap<String, Vec<f64>>,
    ) {
        let w = sel.len();
        for (j, &i) in sel.iter().enumerate() {
            let series: Vec<f64> = (0..samples).map(|s| values[s * w + j]).collect();
            out.insert(catalog[i].0.clone(), series);
        }
    }

    /// One value of every channel at state `x` (the live session's snapshot
    /// for on-demand channels).
    #[cfg(feature = "rms-experimental")]
    pub(super) fn snapshot(
        &mut self,
        sys: &DynSystem,
        x: &[f64],
        patterns: &[String],
    ) -> BTreeMap<String, f64> {
        if self.catalog.is_empty() {
            self.build_catalog(sys, x);
        }
        let sel: Vec<usize> = (0..self.catalog.len())
            .filter(|&i| patterns.is_empty() || RecordSpec::matches(patterns, &self.catalog[i].0))
            .collect();
        let outs = sel.iter().any(|&i| {
            matches!(
                self.catalog[i].1,
                Source::Efd(_) | Source::Tm(_) | Source::Vs(_) | Source::Pe(_) | Source::Qe(_)
            )
        });
        let sig = sel
            .iter()
            .any(|&i| matches!(self.catalog[i].1, Source::Sig(..)));
        let mut vals = Vec::with_capacity(sel.len());
        self.eval(sys, x, &sel, outs, sig, &mut vals);
        sel.iter()
            .zip(vals)
            .map(|(&i, v)| (self.catalog[i].0.clone(), v))
            .collect()
    }

    /// Append the values of `sel` at state `x` to `out`.
    // @block Evaluate channels
    pub(super) fn eval(
        &mut self,
        sys: &DynSystem,
        x: &[f64],
        sel: &[usize],
        outs: bool,
        sig: bool,
        out: &mut Vec<f64>,
    ) {
        let n = sys.n;
        if outs {
            let mut f = vec![0.0; sys.nx];
            let mut o = std::mem::take(&mut self.outs);
            sys.residual(x, &mut f, Some(&mut o));
            self.outs = o;
        }
        let _ = sig;
        let mut sigs: Vec<Option<Vec<f64>>> = Vec::new();
        if sel
            .iter()
            .any(|&i| matches!(self.catalog[i].1, Source::Sig(..)))
        {
            sigs = vec![None; sys.devices.len()];
        }
        // Every branch's flows at once, PowerIO's, when a flow is recorded.
        let mut flows: Option<powerio_matrix::AcBranchFlows> = None;
        for &i in sel {
            let v = match self.catalog[i].1 {
                Source::Vm(b) => Complex::new(x[b], x[n + b]).norm(),
                Source::Va(b) => Complex::new(x[b], x[n + b]).arg(),
                Source::Delta(k) => {
                    let d = &sys.devices[k];
                    let [_, s, ..] = d.layout();
                    x[d.offset + s]
                }
                Source::Omega(k) => {
                    let d = &sys.devices[k];
                    let [_, s, ..] = d.layout();
                    x[d.offset + s + 1]
                }
                Source::Efd(k) => self.outs[k].vf,
                Source::Tm(k) => self.outs[k].tm,
                Source::Vs(k) => self.outs[k].vs,
                Source::Pe(k) => {
                    let d = &sys.devices[k];
                    let o = &self.outs[k];
                    x[d.bus] * o.ir + x[n + d.bus] * o.ii
                }
                Source::Qe(k) => {
                    let d = &sys.devices[k];
                    let o = &self.outs[k];
                    x[n + d.bus] * o.ir - x[d.bus] * o.ii
                }
                Source::Sig(k, j) => {
                    let s = sigs[k].get_or_insert_with(|| {
                        let d = &sys.devices[k];
                        d.graph_signals(
                            &x[d.offset..d.offset + d.n_states()],
                            x[d.bus],
                            x[n + d.bus],
                        )
                        .into_iter()
                        .map(|(_, v)| v)
                        .collect()
                    });
                    s.get(j).copied().unwrap_or(f64::NAN)
                }
                Source::HeldPe(k) | Source::HeldQe(k) => {
                    let h = &sys.held[k];
                    let v = Complex::new(x[h.bus], x[n + h.bus]);
                    let s = v * h.current_at(x, v).conj();
                    if matches!(self.catalog[i].1, Source::HeldPe(_)) {
                        s.re
                    } else {
                        s.im
                    }
                }
                Source::PlantState(k, j) => x[sys.held[k].offset + j],
                Source::Pl(b) => {
                    sys.loads[b]
                        .power(Complex::new(x[b], x[n + b]), sys.low_voltage)
                        .re
                }
                Source::Ql(b) => {
                    sys.loads[b]
                        .power(Complex::new(x[b], x[n + b]), sys.low_voltage)
                        .im
                }
                Source::PFrom(e) => flows.get_or_insert_with(|| sys.branch_flows(x)).pf[e],
                Source::QFrom(e) => flows.get_or_insert_with(|| sys.branch_flows(x)).qf[e],
                Source::PTo(e) => flows.get_or_insert_with(|| sys.branch_flows(x)).pt[e],
                Source::QTo(e) => flows.get_or_insert_with(|| sys.branch_flows(x)).qt[e],
                Source::Derived(i) => self.derived.as_ref().map_or(f64::NAN, |(_, v)| v[i]),
            };
            out.push(v);
        }
    }

    /// Record the channels due at time `t`: the main group at every output
    /// sample, the map frames at their own step.
    // @block Take a sample
    pub(super) fn record(&mut self, sys: &DynSystem, t: f64, x: &[f64]) {
        if self.catalog.is_empty() {
            self.build_catalog(sys, x);
        }
        let mut main = std::mem::replace(
            &mut self.main,
            Group {
                sel: Vec::new(),
                step: 0.0,
                next: 0.0,
                time: Vec::new(),
                values: Vec::new(),
                outs: false,
                sig: false,
            },
        );
        let mut frames = self.frames.take();
        let frame_due = frames.as_ref().is_some_and(|g| t >= g.next - 1e-9);
        let outs = main.outs || (frame_due && frames.as_ref().is_some_and(|g| g.outs));
        let sig = main.sig || (frame_due && frames.as_ref().is_some_and(|g| g.sig));
        // Device outputs once for both groups.
        if outs {
            let mut f = vec![0.0; sys.nx];
            let mut o = std::mem::take(&mut self.outs);
            sys.residual(x, &mut f, Some(&mut o));
            self.outs = o;
        }
        main.time.push(t);
        let sel = std::mem::take(&mut main.sel);
        self.eval(sys, x, &sel, false, sig && main.sig, &mut main.values);
        main.sel = sel;
        if let Some(g) = frames.as_mut().filter(|_| frame_due) {
            g.time.push(t);
            let sel = std::mem::take(&mut g.sel);
            self.eval(sys, x, &sel, false, g.sig, &mut g.values);
            g.sel = sel;
            g.next = t + g.step;
        }
        self.main = main;
        self.frames = frames;
    }
}
