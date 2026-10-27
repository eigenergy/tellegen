//! OPF-in-the-loop calibration: a recorded search over a declared parameter
//! set against a declared objective. Every candidate is evaluated by an
//! exact DC OPF re-solve through the engine; every accepted or rejected step
//! is written to the report with the objective before and after. No figure
//! is adjusted after the fact: when nothing improves, the status says
//! `Unchanged` and the final objective equals the initial one.
//!
//! Parameters: a rating multiplier per line class, a capacity multiplier per
//! fuel, per-branch rating notches on binding branches, and (only when the
//! request allows it) a load multiplier. Objective terms: DC infeasibility,
//! worst-branch loading above the target, under-utilisation of the 90th
//! percentile branch, the binding fraction against its reference range, the
//! reserve margin shortfall, and two structural terms (mean degree and
//! high-voltage share against the reference) that the parameters cannot
//! move but that the report states beside the rest.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use powerio::BalancedNetwork;

use crate::electrical::params::{RATE_B_FACTOR, RATE_C_FACTOR};
use crate::error::SynthError;
use crate::report::{
    CalibrationParameters, CalibrationReport, CalibrationStatus, CalibrationStep, TopologyStats,
};
use crate::request::{CalibrationOptions, CalibrationTargets, Fuel};
use crate::validate::{run_dc_opf, Clock, DcOpfRun};

/// Bounds on the scale parameters.
pub const SCALE_MIN: f64 = 0.8;
pub const SCALE_MAX: f64 = 1.5;
/// Rating increase per notch on a binding branch.
pub const NOTCH_FACTOR: f64 = 1.15;
/// Uniform rating step on infeasibility.
pub const INFEASIBLE_STEP: f64 = 1.10;
/// Objective improvement below this is noise.
pub const TOLERANCE: f64 = 1e-9;

pub struct Calibrator {
    options: CalibrationOptions,
    targets: CalibrationTargets,
    weights: BTreeMap<String, f64>,
    params: CalibrationParameters,
    base_rate: Vec<f64>,
    branch_class: Vec<String>,
    base_pmax: Vec<f64>,
    base_qmax: Vec<f64>,
    base_qmin: Vec<f64>,
    base_load: Vec<(f64, f64)>,
    fuels: Vec<Fuel>,
    reserve_margin: f64,
    structural: BTreeMap<String, f64>,
    current: DcOpfRun,
    current_terms: BTreeMap<String, f64>,
    current_objective: f64,
    initial_objective: f64,
    initial_terms: BTreeMap<String, f64>,
    steps: Vec<CalibrationStep>,
    iteration: u32,
    finished: bool,
    status: CalibrationStatus,
}

pub struct IterationOutcome {
    pub finished: bool,
    pub accepted: bool,
}

impl Calibrator {
    /// Start from a network whose DC OPF has run once (`initial`).
    pub fn new(
        net: &BalancedNetwork,
        fuels: &[Fuel],
        stats: &TopologyStats,
        reserve_margin: f64,
        options: &CalibrationOptions,
        initial: DcOpfRun,
    ) -> Self {
        let targets = options.targets.clone().unwrap_or_default();
        let mut weights = BTreeMap::new();
        weights.insert("dc_infeasible".to_owned(), 10.0);
        weights.insert("max_loading_excess".to_owned(), 4.0);
        weights.insert("underutilisation".to_owned(), 1.0);
        weights.insert("binding_fraction".to_owned(), 2.0);
        weights.insert("reserve_margin".to_owned(), 2.0);
        weights.insert("avg_degree".to_owned(), 0.5);
        weights.insert("hv_share".to_owned(), 0.5);
        let branch_class: Vec<String> = net
            .branches()
            .iter()
            .map(|b| {
                b.extras
                    .get(crate::build::EXTRAS_KEY)
                    .and_then(|v| v.get("class_kv"))
                    .and_then(|v| v.as_f64())
                    .map(|kv| format!("{}", kv.round() as i64))
                    .unwrap_or_else(|| "unclassed".to_owned())
            })
            .collect();
        let mut params = CalibrationParameters {
            rating_scale: BTreeMap::new(),
            gen_capacity_scale: BTreeMap::new(),
            load_scale: 1.0,
            branch_rating_notches: BTreeMap::new(),
        };
        for c in &branch_class {
            params.rating_scale.entry(c.clone()).or_insert(1.0);
        }
        for f in fuels {
            params.gen_capacity_scale.entry(*f).or_insert(1.0);
        }
        let mut structural = BTreeMap::new();
        structural.insert(
            "avg_degree".to_owned(),
            range_excess(stats.avg_degree, targets.avg_degree).powi(2),
        );
        structural.insert(
            "hv_share".to_owned(),
            range_excess(stats.hv_share, targets.hv_share).powi(2),
        );
        let mut me = Self {
            options: options.clone(),
            weights,
            params,
            base_rate: net.branches().iter().map(|b| b.rate_a).collect(),
            branch_class,
            base_pmax: net.generators().iter().map(|g| g.pmax).collect(),
            base_qmax: net.generators().iter().map(|g| g.qmax).collect(),
            base_qmin: net.generators().iter().map(|g| g.qmin).collect(),
            base_load: net.loads().iter().map(|l| (l.p, l.q)).collect(),
            fuels: fuels.to_vec(),
            reserve_margin,
            structural,
            current_terms: BTreeMap::new(),
            current_objective: 0.0,
            initial_objective: 0.0,
            initial_terms: BTreeMap::new(),
            steps: Vec::new(),
            iteration: 0,
            finished: false,
            status: CalibrationStatus::Unchanged,
            targets,
            current: initial,
        };
        let (terms, objective) = me.evaluate(net, &me.current.clone());
        me.current_terms = terms.clone();
        me.current_objective = objective;
        me.initial_terms = terms;
        me.initial_objective = objective;
        me.finished = !options.enabled || options.max_iterations == 0;
        if !options.enabled {
            me.status = CalibrationStatus::Disabled;
        }
        me
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// The DC OPF run of the current (accepted) parameters.
    pub fn current_run(&self) -> &DcOpfRun {
        &self.current
    }

    fn evaluate(&self, net: &BalancedNetwork, run: &DcOpfRun) -> (BTreeMap<String, f64>, f64) {
        let mut terms = self.structural.clone();
        let infeasible = !run.feasible();
        terms.insert(
            "dc_infeasible".to_owned(),
            if infeasible { 1.0 } else { 0.0 },
        );
        let max_loading = run.report.max_loading.unwrap_or(0.0);
        terms.insert(
            "max_loading_excess".to_owned(),
            (max_loading - self.targets.loading_target).max(0.0).powi(2),
        );
        let p90 = run.report.loading_p90.unwrap_or(0.0);
        terms.insert(
            "underutilisation".to_owned(),
            if infeasible {
                0.0
            } else {
                (self.targets.loading_p90_min - p90).max(0.0).powi(2)
            },
        );
        terms.insert(
            "binding_fraction".to_owned(),
            if infeasible {
                0.0
            } else {
                range_excess(run.report.binding_fraction, self.targets.binding_fraction).powi(2)
            },
        );
        let load: f64 = net
            .loads()
            .iter()
            .filter(|l| l.in_service)
            .map(|l| l.p)
            .sum();
        let cap: f64 = net
            .generators()
            .iter()
            .filter(|g| g.in_service)
            .map(|g| g.pmax)
            .sum();
        let margin = if load > 0.0 {
            cap / load
        } else {
            f64::INFINITY
        };
        terms.insert(
            "reserve_margin".to_owned(),
            ((1.0 + self.reserve_margin) - margin).max(0.0).powi(2),
        );
        let objective = terms
            .iter()
            .map(|(k, v)| self.weights.get(k).copied().unwrap_or(1.0) * v)
            .sum();
        (terms, objective)
    }

    /// Apply the current parameters onto `net` from the base values.
    pub fn apply(&self, net: &mut BalancedNetwork) {
        let uids: Vec<Option<String>> = net.branches().iter().map(|b| b.uid.clone()).collect();
        for (k, br) in net.branches_mut().iter_mut().enumerate() {
            let scale = self
                .params
                .rating_scale
                .get(&self.branch_class[k])
                .copied()
                .unwrap_or(1.0);
            let notches = uids[k]
                .as_ref()
                .and_then(|u| self.params.branch_rating_notches.get(u))
                .copied()
                .unwrap_or(0);
            let rate = self.base_rate[k] * scale * NOTCH_FACTOR.powi(notches as i32);
            br.rate_a = rate;
            br.rate_b = rate * RATE_B_FACTOR;
            br.rate_c = rate * RATE_C_FACTOR;
            if notches > 0 || (scale - 1.0).abs() > 1e-12 {
                if let Some(obj) = br
                    .extras
                    .get_mut(crate::build::EXTRAS_KEY)
                    .and_then(|v| v.as_object_mut())
                {
                    obj.insert("rating_basis".to_owned(), serde_json::json!("calibrated"));
                    obj.insert("rating_scale".to_owned(), serde_json::json!(scale));
                    obj.insert("rating_notches".to_owned(), serde_json::json!(notches));
                }
            }
        }
        for (k, g) in net.generators_mut().iter_mut().enumerate() {
            let scale = self
                .fuels
                .get(k)
                .and_then(|f| self.params.gen_capacity_scale.get(f))
                .copied()
                .unwrap_or(1.0);
            g.pmax = self.base_pmax[k] * scale;
            g.qmax = self.base_qmax[k] * scale;
            g.qmin = self.base_qmin[k] * scale;
            g.mbase = g.pmax;
            g.pg = g.pg.min(g.pmax);
        }
        for (k, l) in net.loads_mut().iter_mut().enumerate() {
            l.p = self.base_load[k].0 * self.params.load_scale;
            l.q = self.base_load[k].1 * self.params.load_scale;
        }
    }

    /// One recorded step with an exact re-solve. The network leaves this call
    /// at the accepted parameters.
    pub fn iterate(
        &mut self,
        net: &mut BalancedNetwork,
        cancel: Option<Arc<AtomicBool>>,
        clock: Clock,
    ) -> Result<IterationOutcome, SynthError> {
        if self.finished {
            return Ok(IterationOutcome {
                finished: true,
                accepted: false,
            });
        }
        self.iteration += 1;
        let Some(step) = self.choose_step(net) else {
            self.finished = true;
            self.status = self.final_status();
            return Ok(IterationOutcome {
                finished: true,
                accepted: false,
            });
        };
        let before_params = self.params.clone();
        let (parameter, before, after, reason) = step;
        self.apply(net);
        let run = run_dc_opf(net, &self.fuels, cancel, clock)?;
        let (terms, objective) = self.evaluate(net, &run);
        let accepted = objective < self.current_objective - TOLERANCE;
        let dc_status = run.report.status;
        self.steps.push(CalibrationStep {
            iteration: self.iteration,
            parameter,
            before,
            after,
            reason,
            objective_before: self.current_objective,
            objective_after: objective,
            dc_status,
            accepted,
        });
        if accepted {
            self.current = run;
            self.current_terms = terms;
            self.current_objective = objective;
        } else {
            self.params = before_params;
            self.apply(net);
        }
        let within = self.operational_terms_zero();
        if within || self.iteration >= self.options.max_iterations || !accepted {
            self.finished = true;
            self.status = self.final_status();
        }
        Ok(IterationOutcome {
            finished: self.finished,
            accepted,
        })
    }

    fn operational_terms_zero(&self) -> bool {
        [
            "dc_infeasible",
            "max_loading_excess",
            "underutilisation",
            "binding_fraction",
            "reserve_margin",
        ]
        .iter()
        .all(|k| self.current_terms.get(*k).copied().unwrap_or(0.0) <= TOLERANCE)
    }

    fn final_status(&self) -> CalibrationStatus {
        if !self.options.enabled {
            CalibrationStatus::Disabled
        } else if self.operational_terms_zero() {
            CalibrationStatus::Converged
        } else if !self.current.feasible() {
            CalibrationStatus::Infeasible
        } else if self.current_objective < self.initial_objective - TOLERANCE {
            CalibrationStatus::Improved
        } else {
            CalibrationStatus::Unchanged
        }
    }

    /// Pick the next parameter change: `(parameter, before, after, reason)`.
    /// `None` when every term is within tolerance or no bounded move remains.
    fn choose_step(&mut self, net: &BalancedNetwork) -> Option<(String, f64, f64, String)> {
        let run = &self.current;
        if !run.feasible() {
            // Uniform rating relief: nothing in an infeasible solve names a culprit.
            let classes: Vec<String> = self.params.rating_scale.keys().cloned().collect();
            let movable: Vec<&String> = classes
                .iter()
                .filter(|c| self.params.rating_scale[*c] < SCALE_MAX - 1e-12)
                .collect();
            if movable.is_empty() {
                return None;
            }
            let before = self.params.rating_scale[movable[0]];
            for c in &classes {
                let v = self.params.rating_scale.get_mut(c).unwrap();
                *v = (*v * INFEASIBLE_STEP).min(SCALE_MAX);
            }
            let after = self.params.rating_scale[movable[0]];
            return Some((
                "rating_scale[*]".to_owned(),
                before,
                after,
                format!(
                    "DC OPF infeasible: {}; raising every class rating by {:.0}%",
                    run.report.message.clone().unwrap_or_default(),
                    (INFEASIBLE_STEP - 1.0) * 100.0
                ),
            ));
        }
        let max_loading = run.report.max_loading.unwrap_or(0.0);
        if max_loading > self.targets.loading_target + TOLERANCE {
            let mut binding: Vec<(String, f64)> = net
                .branches()
                .iter()
                .enumerate()
                .filter_map(|(k, b)| {
                    let l = run.loading.get(k).copied().flatten()?;
                    (l > self.targets.loading_target)
                        .then(|| (b.uid.clone().unwrap_or_default(), l))
                })
                .filter(|(u, _)| !u.is_empty())
                .collect();
            binding.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            if binding.is_empty() {
                return None;
            }
            let before: u32 = self.params.branch_rating_notches.values().sum();
            for (uid, _) in &binding {
                *self
                    .params
                    .branch_rating_notches
                    .entry(uid.clone())
                    .or_insert(0) += 1;
            }
            let after: u32 = self.params.branch_rating_notches.values().sum();
            let names: Vec<String> = binding
                .iter()
                .take(8)
                .map(|(u, l)| format!("{u} ({l:.3})"))
                .collect();
            return Some((
                "branch_rating_notches".to_owned(),
                before as f64,
                after as f64,
                format!(
                    "max loading {max_loading:.3} above target {:.2}: one rating notch (×{NOTCH_FACTOR}) on {} branch(es): {}",
                    self.targets.loading_target,
                    binding.len(),
                    names.join(", ")
                ),
            ));
        }
        let load: f64 = net
            .loads()
            .iter()
            .filter(|l| l.in_service)
            .map(|l| l.p)
            .sum();
        let cap: f64 = net
            .generators()
            .iter()
            .filter(|g| g.in_service)
            .map(|g| g.pmax)
            .sum();
        let want = (1.0 + self.reserve_margin) * load;
        if load > 0.0 && cap < want - 1e-6 {
            let ratio = want / cap.max(1e-9);
            let fuels: Vec<Fuel> = self.params.gen_capacity_scale.keys().copied().collect();
            let before = self.params.gen_capacity_scale[&fuels[0]];
            let mut moved = false;
            for f in &fuels {
                let v = self.params.gen_capacity_scale.get_mut(f).unwrap();
                let next = (*v * ratio).min(SCALE_MAX);
                if (next - *v).abs() > 1e-12 {
                    moved = true;
                }
                *v = next;
            }
            if !moved {
                return None;
            }
            let after = self.params.gen_capacity_scale[&fuels[0]];
            return Some((
                "gen_capacity_scale[*]".to_owned(),
                before,
                after,
                format!(
                    "capacity {cap:.1} MW below (1 + {:.2}) × load {load:.1} MW",
                    self.reserve_margin
                ),
            ));
        }
        let p90 = run.report.loading_p90.unwrap_or(0.0);
        if p90 < self.targets.loading_p90_min - TOLERANCE {
            // Ratings are generous everywhere: tighten the least loaded class.
            let mut class_max: BTreeMap<String, f64> = BTreeMap::new();
            for (k, l) in run.loading.iter().enumerate() {
                if let Some(l) = l {
                    let e = class_max.entry(self.branch_class[k].clone()).or_insert(0.0);
                    *e = e.max(*l);
                }
            }
            let candidate = class_max
                .iter()
                .filter(|(c, _)| {
                    self.params.rating_scale.get(*c).copied().unwrap_or(1.0) > SCALE_MIN + 1e-12
                })
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(c, l)| (c.clone(), *l));
            let (class, worst) = candidate?;
            let before = self.params.rating_scale[&class];
            let after = (before * 0.9).max(SCALE_MIN);
            self.params.rating_scale.insert(class.clone(), after);
            return Some((
                format!("rating_scale[{class}]"),
                before,
                after,
                format!(
                    "90th percentile loading {p90:.3} below {:.2}; class {class} kV peaks at {worst:.3}",
                    self.targets.loading_p90_min
                ),
            ));
        }
        None
    }

    pub fn report(&self) -> CalibrationReport {
        CalibrationReport {
            status: if self.finished {
                self.status
            } else {
                self.final_status()
            },
            iterations: self.iteration,
            initial_objective: self.initial_objective,
            final_objective: self.current_objective,
            terms_initial: self.initial_terms.clone(),
            terms_final: self.current_terms.clone(),
            weights: self.weights.clone(),
            parameters: self.params.clone(),
            steps: self.steps.clone(),
            targets: self.targets.clone(),
        }
    }
}

/// Distance of `v` outside `[lo, hi]` (0 inside).
pub fn range_excess(v: f64, range: [f64; 2]) -> f64 {
    if v < range[0] {
        range[0] - v
    } else if v > range[1] {
        v - range[1]
    } else {
        0.0
    }
}
