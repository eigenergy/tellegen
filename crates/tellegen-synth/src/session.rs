//! The staged generation session. Each `step()` runs exactly one stage
//! (`Calibrate` runs one iteration per step), so a host gets a progress
//! event per stage, can read a map preview between stages, and can stop
//! between them by not calling again. Nothing here reads a clock or the
//! filesystem: timings come from a host supplied clock.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use powerio::geo::CoordsKind;
use powerio::{BalancedNetwork, HistoryEntry, HistoryId, HistoryKind, PioModule, Producer};

use crate::build::{build_network, BuiltNetwork};
use crate::calibrate::Calibrator;
use crate::context::{resolve_context, ContextData};
use crate::demand::{CountryAreas, NationalDemand};
use crate::electrical::branches::{draw_corridors, expand_circuits, spread_sigma, DraftBranch};
use crate::electrical::generators::{place_generators, DraftGenerator};
use crate::electrical::loads::assign_loads;
use crate::electrical::params::{
    MAX_ANGLE_SPREAD_RAD, MAX_CIRCUITS_PER_CORRIDOR, PLANNING_UTILISATION, UPGRADE_CIRCUITS,
};
use crate::electrical::planning::{plan_corridors, PlanMode};
use crate::electrical::reactive::{place_voltage_support, plan_shunts};
use crate::error::SynthError;
use crate::geom::wrap_lon;
use crate::places::Places;
use crate::report::{
    AcPfOutcome, DemandBasis, DemandTableProvenance, GenerationReport, PlacesProvenance,
    PlanningReport, Preview, PreviewArrays, PreviewBranch, PreviewBus, Provenance, SolveOutcome,
    Stage, StageRecord, StepOutcome, VoltageBasis, VoltageTableProvenance, Warning, REPORT_SCHEMA,
};
use crate::request::{DemandSpec, Source, SynthRequest};
use crate::rng::{Stream, StreamTag};
use crate::site::{site_buses, DraftBus};
use crate::topology::{build_topology, Corridor};
use crate::validate::{apply_dispatch, run_ac_pf, run_dc_opf, Clock, DcOpfRun};
use crate::voltage::VoltageLadders;

/// Producer name written into generated modules.
pub const PRODUCER_NAME: &str = "tellegen-synth";

/// Name (and id) of the history entry a generated module records. Readers
/// take it as the sign that the module's `synthetic` coordinates are sites
/// the generator chose on real geography, not a topology layout.
pub const HISTORY_NAME: &str = "tellegen-synth.generate";

#[derive(Default)]
struct State {
    context: Option<ContextData>,
    buses: Vec<DraftBus>,
    corridors: Vec<Corridor>,
    interconnects: usize,
    generators: Vec<DraftGenerator>,
    branches: Vec<DraftBranch>,
    built: Option<BuiltNetwork>,
    dc: Option<DcOpfRun>,
    calibrator: Option<Calibrator>,
    module: Option<PioModule<BalancedNetwork>>,
    osm: Option<crate::osm::OsmBuild>,
}

/// A generation session.
pub struct Generator {
    request: SynthRequest,
    places: Arc<Places>,
    osm_json: Option<String>,
    stage: Stage,
    calibration_iteration: u32,
    report: GenerationReport,
    state: State,
    clock: Clock,
    cancel: Arc<AtomicBool>,
}

impl Generator {
    /// Validate the request and open a session at the `Context` stage. The
    /// Overpass document (`osm`) is only read when `request.source` is `Osm`.
    pub fn new(
        request: SynthRequest,
        places: Arc<Places>,
        osm: Option<String>,
    ) -> Result<Self, SynthError> {
        request.validate()?;
        if request.source == Source::Osm && osm.is_none() {
            return Err(SynthError::InvalidRequest(
                "source is osm but no Overpass document was supplied".to_owned(),
            ));
        }
        let mut warnings = Vec::new();
        if let Some(fallback) = &request.fallback_from {
            warnings.push(Warning::new(
                "osm_fallback",
                format!(
                    "generated synthetically after the OpenStreetMap attempt ({:?}){}",
                    fallback.reason,
                    fallback
                        .message
                        .as_ref()
                        .map(|m| format!(": {m}"))
                        .unwrap_or_default()
                ),
            ));
        }
        let provenance = Provenance {
            generator: PRODUCER_NAME.to_owned(),
            seed: request.seed,
            request_sha256: request.sha256(),
            tellegen_synth_version: crate::VERSION.to_owned(),
            tellegen_version: tellegen::VERSION.to_owned(),
            powerio_version: powerio::VERSION.to_owned(),
            parameter_table_version: crate::PARAMETER_TABLE_VERSION.to_owned(),
            spread_sigma: spread_sigma(request.fidelity),
            places: Some(PlacesProvenance {
                count: places.len(),
                sha256: places.sha256().to_owned(),
                source: places.source().map(str::to_owned),
                source_sha256: places.source_sha256().map(str::to_owned),
            }),
            demand_table: None,
            voltage_table: None,
            osm: None,
            fallback_from: request.fallback_from.clone(),
        };
        let report = GenerationReport {
            schema: REPORT_SCHEMA.to_owned(),
            request: request.clone(),
            context: None,
            stages: Vec::new(),
            stats: None,
            planning: None,
            dc_opf: None,
            ac_pf: None,
            calibration: None,
            elements: None,
            provenance,
            warnings,
        };
        Ok(Self {
            request,
            places,
            osm_json: osm,
            stage: Stage::Context,
            calibration_iteration: 0,
            report,
            state: State::default(),
            clock: None,
            cancel: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Install a millisecond clock for stage timings.
    pub fn set_clock(&mut self, clock: fn() -> f64) {
        self.clock = Some(clock);
    }

    /// A flag the host sets to stop the next stage (and a running DC OPF).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// The stage the next `step()` runs.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn is_done(&self) -> bool {
        self.stage == Stage::Done
    }

    pub fn request(&self) -> &SynthRequest {
        &self.request
    }

    pub fn report(&self) -> &GenerationReport {
        &self.report
    }

    /// The network, from `Electrify` on.
    pub fn network(&self) -> Option<&BalancedNetwork> {
        self.state.built.as_ref().map(|b| &b.net)
    }

    /// Run one stage. Returns `Err(State)` after `Done`.
    pub fn step(&mut self) -> Result<StepOutcome, SynthError> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(SynthError::Cancelled);
        }
        let stage = self.stage;
        let started = self.clock.map(|f| f());
        let mut warnings = Vec::new();
        let mut counts = BTreeMap::new();
        let mut preview_changed = false;
        let mut iteration = None;
        let next = match stage {
            Stage::Context => {
                self.run_context(&mut warnings, &mut counts)?;
                Stage::SiteBuses
            }
            Stage::SiteBuses => {
                self.run_site(&mut warnings, &mut counts)?;
                preview_changed = true;
                Stage::Topology
            }
            Stage::Topology => {
                self.run_topology(&mut warnings, &mut counts)?;
                preview_changed = true;
                Stage::Electrify
            }
            Stage::Electrify => {
                self.run_electrify(&mut warnings, &mut counts)?;
                preview_changed = true;
                Stage::DcOpf
            }
            Stage::DcOpf => {
                self.run_dc(&mut warnings, &mut counts)?;
                Stage::Calibrate
            }
            Stage::Calibrate => {
                let finished = self.run_calibrate(&mut warnings, &mut counts)?;
                iteration = Some(self.calibration_iteration);
                if finished {
                    Stage::AcPf
                } else {
                    Stage::Calibrate
                }
            }
            Stage::AcPf => {
                self.run_ac(&mut warnings, &mut counts)?;
                Stage::Finalize
            }
            Stage::Finalize => {
                self.run_finalize(&mut counts)?;
                Stage::Done
            }
            Stage::Done => {
                return Err(SynthError::State(
                    "the session is done; no further stage to run".to_owned(),
                ))
            }
        };
        let elapsed_ms = started.and_then(|s| self.clock.map(|f| f() - s));
        self.report.stages.push(StageRecord {
            stage,
            iteration,
            elapsed_ms,
            counts,
        });
        self.report.warnings.extend(warnings.iter().cloned());
        self.stage = next;
        let progress = if stage == Stage::Calibrate && next == Stage::Calibrate {
            let max = self.request.calibration.max_iterations.max(1) as f64;
            Stage::DcOpf.progress()
                + (Stage::Calibrate.progress() - Stage::DcOpf.progress())
                    * (self.calibration_iteration as f64 / max).min(1.0)
        } else {
            stage.progress()
        };
        Ok(StepOutcome {
            stage,
            iteration,
            next: (next != Stage::Done || stage != Stage::Done)
                .then_some(next)
                .filter(|_| stage != Stage::Finalize),
            progress,
            done: next == Stage::Done,
            warnings,
            preview_changed,
            elapsed_ms,
        })
    }

    /// Copy the OSM build's counters into the provenance block.
    fn sync_osm_counts(&mut self) {
        if let (Some(osm), Some(provenance)) =
            (self.state.osm.as_ref(), self.report.provenance.osm.as_mut())
        {
            provenance.counts.extend(osm.counts.as_map());
        }
    }

    fn run_context(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<(), SynthError> {
        let (context, data) = resolve_context(&self.request, &self.places, warnings)?;
        if let (DemandBasis::National, DemandSpec::Population { peak_factor, .. }) =
            (context.demand_basis, &self.request.demand)
        {
            let table = NationalDemand::bundled();
            let areas = CountryAreas::bundled();
            self.report.provenance.demand_table = Some(DemandTableProvenance {
                schema: crate::demand::SCHEMA.to_owned(),
                year: table.year,
                source: table.source.clone(),
                licence: table.licence.clone(),
                sha256: table.sha256.clone(),
                source_sha256: table.source_sha256.clone(),
                peak_factor: *peak_factor,
                areas_sha256: areas.sha256.clone(),
                areas_source_sha256: areas.source_sha256.clone(),
            });
        }
        if context.voltage_basis != VoltageBasis::Request {
            let table = VoltageLadders::bundled();
            self.report.provenance.voltage_table = Some(VoltageTableProvenance {
                schema: crate::voltage::SCHEMA.to_owned(),
                version: table.version.clone(),
                sha256: table.sha256.clone(),
            });
        }
        counts.insert("places_in_region".into(), data.cities.len() as u64);
        counts.insert("landmasses".into(), context.landmasses as u64);
        counts.insert("islands".into(), data.group_count as u64);
        counts.insert("target_bus_count".into(), context.target_bus_count as u64);
        if self.request.source == Source::Osm {
            let doc = self.osm_json.take().unwrap_or_default();
            let parsed = crate::osm::parse_document(&doc)?;
            self.report.provenance.osm = Some(parsed.provenance.clone());
            counts.insert("osm_elements".into(), parsed.provenance.elements as u64);
            self.state.osm = Some(crate::osm::OsmBuild::new(parsed));
        }
        self.report.context = Some(context);
        self.state.context = Some(data);
        Ok(())
    }

    fn run_site(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<(), SynthError> {
        let ctx = self.report.context.as_ref().expect("context ran");
        let data = self.state.context.as_ref().expect("context ran");
        if let Some(osm) = self.state.osm.as_mut() {
            let mut rng = Stream::new(self.request.seed, StreamTag::Osm);
            let sited = osm.site(&self.request, ctx, data, &mut rng, warnings)?;
            self.state.buses = sited;
        } else {
            let mut rng = Stream::new(self.request.seed, StreamTag::Site);
            self.state.buses = site_buses(ctx, data, &mut rng);
        }
        let mut rng = Stream::new(self.request.seed, StreamTag::Loads);
        let loads = assign_loads(
            &mut self.state.buses,
            ctx,
            data,
            self.request.fidelity,
            self.request.power_factor,
            self.state.osm.is_none(),
            &mut rng,
        );
        counts.insert("buses_promoted".into(), loads.promoted as u64);
        counts.insert("buses_load_capped".into(), loads.capped as u64);
        counts.insert("load_spilled_mw".into(), loads.spilled_mw.round() as u64);
        if loads.uniform
            && ctx.places_in_region > 0
            && self.request.fidelity == crate::request::Fidelity::High
        {
            warnings.push(Warning::new(
                "no_population_in_range",
                "no populated place lies within 25 km of any bus; loads spread with a floor and a random weight",
            ));
        }
        counts.insert("buses".into(), self.state.buses.len() as u64);
        counts.insert(
            "city_anchored".into(),
            self.state
                .buses
                .iter()
                .filter(|b| b.place.is_some())
                .count() as u64,
        );
        self.sync_osm_counts();
        Ok(())
    }

    fn run_topology(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<(), SynthError> {
        let ctx = self.report.context.as_ref().expect("context ran");
        let data = self.state.context.as_ref().expect("context ran");
        let mut rng = Stream::new(self.request.seed, StreamTag::Topology);
        let out = if let Some(osm) = self.state.osm.as_mut() {
            osm.topology(&mut self.state.buses, data, warnings)?
        } else {
            build_topology(&self.state.buses, data, ctx.branch_factor, &mut rng)
        };
        counts.insert("corridors".into(), out.corridors.len() as u64);
        counts.insert(
            "branches".into(),
            out.corridors.iter().map(|c| c.circuits as u64).sum(),
        );
        counts.insert("interconnects".into(), out.interconnects as u64);
        self.state.corridors = out.corridors;
        self.state.interconnects = out.interconnects;
        self.sync_osm_counts();
        Ok(())
    }

    fn run_electrify(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<(), SynthError> {
        let ctx = self.report.context.as_ref().expect("context ran");
        let mut rng = Stream::new(self.request.seed, StreamTag::Generators);
        let generators = if let Some(osm) = self.state.osm.as_mut() {
            osm.generators(
                &mut self.state.buses,
                &self.state.corridors,
                ctx,
                &self.request,
                &mut rng,
                warnings,
            )
        } else {
            place_generators(
                &mut self.state.buses,
                &self.state.corridors,
                ctx,
                &self.request,
                &mut rng,
                warnings,
            )
        };
        let mut rng = Stream::new(self.request.seed, StreamTag::Electrical);
        let draws = draw_corridors(&self.state.corridors, self.request.fidelity, &mut rng);
        let branch_target = (ctx.branch_factor * self.state.buses.len() as f64).round() as usize;
        let mode = if self.state.osm.is_some() {
            PlanMode::Observe
        } else {
            PlanMode::Expand
        };
        let (designs, plan) = plan_corridors(
            &mut self.state.buses,
            &ctx.voltage_levels_kv,
            &ctx.promotion_levels_kv,
            &mut self.state.corridors,
            &draws,
            &generators,
            branch_target,
            mode,
        );
        // Mapped (OSM) networks keep their real equipment; only a synthetic
        // plan adds condensers.
        let condensers = if mode == PlanMode::Expand {
            place_voltage_support(&mut self.state.buses, &self.state.corridors, &generators)
        } else {
            Vec::new()
        };
        counts.insert("condensers".into(), condensers.len() as u64);
        let mut generators = generators;
        let mut dispatch = plan.dispatch.clone();
        dispatch.extend(std::iter::repeat_n(0.0, condensers.len()));
        generators.extend(condensers);
        let branches = expand_circuits(&self.state.corridors, &designs);
        let reactive = plan_shunts(
            &self.state.buses,
            &branches,
            &generators,
            &dispatch,
            &plan.grounds,
        );
        if !reactive.angles_converged {
            warnings.push(Warning::new(
                "reactive_plan_unconverged",
                "the AC angle solve of the reactive plan did not meet its tolerance at the planned voltage profile; the shunt banks are sized from its last iterate",
            ));
        }
        let shunts = reactive.shunts;
        if mode == PlanMode::Observe && plan.overloaded_corridors > 0 {
            warnings.push(Warning::new(
                "osm_corridors_overloaded",
                format!(
                    "{} mapped corridor(s) run above their modelled rating at the peak economic dispatch (highest loading {:.2}); the DC OPF has to redispatch around them or report infeasibility",
                    plan.overloaded_corridors, plan.max_loading
                ),
            ));
        }
        if mode == PlanMode::Expand && branches.len() > branch_target {
            warnings.push(Warning::new(
                "branch_target_exceeded",
                format!(
                    "the branch factor asks for {branch_target} branches; the corridor plan needed {} so that the peak economic dispatch loads no circuit above {:.0}% of its rating and no island spans more than {:.1}° of angle",
                    branches.len(),
                    PLANNING_UTILISATION * 100.0,
                    MAX_ANGLE_SPREAD_RAD.to_degrees()
                ),
            ));
        }
        if mode == PlanMode::Expand && (!plan.converged || plan.overloaded_corridors > 0) {
            warnings.push(Warning::new(
                "planning_incomplete",
                format!(
                    "the corridor plan stopped at its round or circuit cap after {} round(s): {} corridor(s) above their rating in the planning case (highest loading {:.2}), {} corridor(s) at the {MAX_CIRCUITS_PER_CORRIDOR}-circuit cap still short of capacity, widest island angle spread {:.1}°",
                    plan.rounds,
                    plan.overloaded_corridors,
                    plan.max_loading,
                    plan.capped_corridors,
                    plan.angle_spread_deg
                ),
            ));
        }
        if let Some(&kv) = plan.added_levels_kv.first() {
            let added = plan.added_levels_kv.len();
            let raised = self.state.buses.iter().filter(|b| b.tier < added).count();
            warnings.push(Warning::new(
                "voltage_level_added",
                format!(
                    "{raised} bus(es) were raised to {kv} kV, a level in service above the modelled ones: their corridors would otherwise need more than {UPGRADE_CIRCUITS} circuits at the highest modelled level"
                ),
            ));
            if let Some(ctx) = self.report.context.as_mut() {
                for (k, &level) in plan.added_levels_kv.iter().enumerate() {
                    ctx.voltage_levels_kv.insert(k, level);
                    ctx.voltage_classes_kv
                        .insert(k, crate::electrical::params::line_class_for(level).kv);
                }
            }
        }
        let name = format!(
            "tellegen_synth_{}",
            &self.report.provenance.request_sha256[..16]
        );
        let (hvdc, coords_kind, osm_ids, makeup) = match self.state.osm.as_ref() {
            Some(osm) => (
                osm.hvdc_links(),
                CoordsKind::Source,
                Some(osm.branch_osm_ids(&branches)),
                Some(osm.makeup_flags(&generators)),
            ),
            None => (Vec::new(), CoordsKind::Synthetic, None, None),
        };
        let built = build_network(
            &name,
            &self.state.buses,
            &branches,
            &generators,
            &shunts,
            &hvdc,
            coords_kind,
            osm_ids.as_deref(),
            makeup.as_deref(),
        )?;
        let capacitors: Vec<f64> = shunts.iter().map(|s| s.mvar).filter(|q| *q > 0.0).collect();
        let reactors: Vec<f64> = shunts
            .iter()
            .map(|s| -s.mvar)
            .filter(|q| *q > 0.0)
            .collect();
        // Banks are rated to 0.1 MVAr; `+ 0.0` turns an empty sum's -0.0 into 0.0.
        let total = |v: &[f64]| (v.iter().sum::<f64>() * 10.0).round() / 10.0 + 0.0;
        counts.insert("generators".into(), generators.len() as u64);
        counts.insert("branches".into(), branches.len() as u64);
        counts.insert("loads".into(), built.net.loads().len() as u64);
        counts.insert("hvdc".into(), hvdc.len() as u64);
        counts.insert(
            "transformers".into(),
            branches.iter().filter(|b| b.is_transformer).count() as u64,
        );
        counts.insert("planning_rounds".into(), plan.rounds as u64);
        counts.insert("promoted_buses".into(), plan.promoted_buses as u64);
        counts.insert("capacity_circuits".into(), plan.capacity_circuits as u64);
        counts.insert(
            "upsized_transformers".into(),
            plan.upsized_transformers as u64,
        );
        counts.insert(
            "stiffening_circuits".into(),
            plan.stiffening_circuits as u64,
        );
        counts.insert("filler_circuits".into(), plan.filler_circuits as u64);
        counts.insert("shunts".into(), shunts.len() as u64);
        self.report.planning = Some(PlanningReport {
            dispatch: "economic_dispatch_at_peak".to_owned(),
            utilisation_target: PLANNING_UTILISATION,
            rounds: plan.rounds,
            converged: plan.converged,
            promoted_buses: plan.promoted_buses,
            added_level_kv: plan.added_levels_kv.first().copied(),
            max_parallel_circuits: plan.max_parallel_circuits,
            circuit_cap: MAX_CIRCUITS_PER_CORRIDOR,
            capped_corridors: plan.capped_corridors,
            branch_target,
            branches: branches.len(),
            capacity_circuits: plan.capacity_circuits,
            upsized_transformers: plan.upsized_transformers,
            stiffening_circuits: plan.stiffening_circuits,
            angle_spread_deg: plan.angle_spread_deg,
            filler_circuits: plan.filler_circuits,
            max_planning_loading: plan.max_loading,
            overloaded_corridors: plan.overloaded_corridors,
            capacitor_banks: capacitors.len(),
            capacitor_mvar: total(&capacitors),
            reactor_banks: reactors.len(),
            reactor_mvar: total(&reactors),
            max_angle_difference_deg: reactive.max_angle_difference_deg,
        });
        let mut stats = crate::stats::measure(&built.net, None);
        let targets = self.request.calibration.targets.clone().unwrap_or_default();
        crate::stats::compare(&mut stats, &targets);
        self.report.stats = Some(stats);
        self.report.elements = Some(built.annotations.clone());
        self.state.generators = generators;
        self.state.branches = branches;
        self.state.built = Some(built);
        self.sync_osm_counts();
        Ok(())
    }

    fn run_dc(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<(), SynthError> {
        if !self.request.validation.run_dc_opf {
            self.report.dc_opf = Some(crate::validate::skipped_dc());
            return Ok(());
        }
        let built = self.state.built.as_mut().expect("electrify ran");
        let run = run_dc_opf(
            &built.net,
            &built.gen_fuels,
            Some(self.cancel.clone()),
            self.clock,
        )?;
        if run.feasible() {
            apply_dispatch(&mut built.net, &run.dispatch);
        } else {
            warnings.push(Warning::new(
                "dc_opf_infeasible",
                format!(
                    "the DC optimal power flow is infeasible before calibration: {}",
                    run.report.message.clone().unwrap_or_default()
                ),
            ));
        }
        counts.insert("islands_solved".into(), run.report.islands.len() as u64);
        counts.insert(
            "binding_branches".into(),
            run.report.binding_branches.len() as u64,
        );
        if let Some(stats) = self.report.stats.as_mut() {
            stats.max_line_loading = run.report.max_loading;
        }
        self.report.dc_opf = Some(run.report.clone());
        self.state.dc = Some(run);
        Ok(())
    }

    /// One calibration iteration; returns true when calibration is finished.
    fn run_calibrate(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<bool, SynthError> {
        let built = self.state.built.as_mut().expect("electrify ran");
        if self.state.calibrator.is_none() {
            let Some(initial) = self.state.dc.clone() else {
                // No DC OPF to work with: record a disabled calibration.
                let mut options = self.request.calibration.clone();
                options.enabled = false;
                let placeholder = DcOpfRun {
                    report: crate::validate::skipped_dc(),
                    dispatch: Vec::new(),
                    loading: vec![None; built.net.branches().len()],
                    branch_flow_mw: vec![None; built.net.branches().len()],
                };
                let cal = Calibrator::new(
                    &built.net,
                    &built.gen_fuels,
                    self.report.stats.as_ref().expect("stats"),
                    self.request.reserve_margin,
                    &options,
                    placeholder,
                );
                self.report.calibration = Some(cal.report());
                return Ok(true);
            };
            self.state.calibrator = Some(Calibrator::new(
                &built.net,
                &built.gen_fuels,
                self.report.stats.as_ref().expect("stats"),
                self.request.reserve_margin,
                &self.request.calibration,
                initial,
            ));
        }
        let cal = self.state.calibrator.as_mut().expect("just set");
        let finished = if cal.finished() {
            true
        } else {
            let outcome = cal.iterate(&mut built.net, Some(self.cancel.clone()), self.clock)?;
            self.calibration_iteration = cal.iteration();
            counts.insert("accepted".into(), outcome.accepted as u64);
            outcome.finished
        };
        counts.insert("iteration".into(), self.calibration_iteration as u64);
        if finished {
            let run = cal.current_run().clone();
            let report = cal.report();
            if run.feasible() {
                apply_dispatch(&mut built.net, &run.dispatch);
            } else if self.request.calibration.enabled {
                warnings.push(Warning::new(
                    "dc_opf_infeasible_after_calibration",
                    "the DC optimal power flow stayed infeasible through calibration",
                ));
            }
            if let Some(stats) = self.report.stats.as_mut() {
                stats.max_line_loading = run.report.max_loading;
                stats.total_generation_capacity_mw = built
                    .net
                    .generators()
                    .iter()
                    .filter(|g| g.in_service)
                    .map(|g| g.pmax)
                    .sum();
                if stats.total_load_mw > 0.0 {
                    stats.gen_load_margin =
                        stats.total_generation_capacity_mw / stats.total_load_mw;
                }
            }
            if self.request.validation.run_dc_opf {
                self.report.dc_opf = Some(run.report.clone());
            }
            self.state.dc = Some(run);
            self.report.calibration = Some(report);
        }
        Ok(finished)
    }

    fn run_ac(
        &mut self,
        warnings: &mut Vec<Warning>,
        counts: &mut BTreeMap<String, u64>,
    ) -> Result<(), SynthError> {
        if !self.request.validation.run_ac_pf {
            self.report.ac_pf = Some(crate::validate::skipped_ac());
            return Ok(());
        }
        let built = self.state.built.as_ref().expect("electrify ran");
        let report = run_ac_pf(&built.net, &self.request.validation, self.clock);
        if report.status == AcPfOutcome::NotConverged {
            warnings.push(Warning::new(
                "ac_pf_not_converged",
                format!(
                    "the AC power flow did not converge: {}",
                    report.message.clone().unwrap_or_default()
                ),
            ));
        }
        counts.insert(
            "buses_outside_planning_band".into(),
            report.buses_outside_planning_band as u64,
        );
        self.report.ac_pf = Some(report);
        Ok(())
    }

    fn run_finalize(&mut self, counts: &mut BTreeMap<String, u64>) -> Result<(), SynthError> {
        let built = self.state.built.as_ref().expect("electrify ran");
        let module = self.make_module(&built.net)?;
        counts.insert("history_entries".into(), module.history().len() as u64);
        if let Some(dc) = &self.report.dc_opf {
            if dc.status == SolveOutcome::Infeasible {
                counts.insert("dc_opf_infeasible".into(), 1);
            }
        }
        self.state.module = Some(module);
        Ok(())
    }

    fn make_module(&self, net: &BalancedNetwork) -> Result<PioModule<BalancedNetwork>, SynthError> {
        let producer = Producer::new(PRODUCER_NAME, crate::VERSION)
            .map_err(|e| SynthError::Build(e.to_string()))?;
        let mut module = PioModule::new(net.clone()).with_producer(producer);
        let id = HistoryId::new(HISTORY_NAME).map_err(|e| SynthError::Build(e.to_string()))?;
        let mut parameters = BTreeMap::new();
        parameters.insert(
            "request_sha256".to_owned(),
            serde_json::Value::String(self.report.provenance.request_sha256.clone()),
        );
        parameters.insert("seed".to_owned(), serde_json::json!(self.request.seed));
        parameters.insert("source".to_owned(), serde_json::json!(self.request.source));
        let entry = HistoryEntry::new(id, HistoryKind::Transform, HISTORY_NAME)
            .and_then(|entry| entry.with_parameters(parameters))
            .map_err(|e| SynthError::Build(e.to_string()))?;
        module
            .add_history_entry(entry)
            .map_err(|e| SynthError::Build(e.to_string()))?;
        Ok(module)
    }

    /// The module: the network with its producer and generation history,
    /// from `Electrify` on. Before `Finalize` it reflects the network as the
    /// stages so far left it. Idempotent.
    pub fn module(&self) -> Result<PioModule<BalancedNetwork>, SynthError> {
        match &self.state.module {
            Some(m) => Ok(m.clone()),
            None => {
                let net = self
                    .network()
                    .ok_or_else(|| SynthError::State("no network before Electrify".to_owned()))?;
                self.make_module(net)
            }
        }
    }

    /// The network as GridFM graphkit JSON, from `Electrify` on, with the
    /// latest DC OPF flows when one has run.
    pub fn export_gridfm(&self) -> Result<String, SynthError> {
        let net = self
            .network()
            .ok_or_else(|| SynthError::State("no network before Electrify".to_owned()))?;
        let flows = self
            .state
            .dc
            .as_ref()
            .filter(|run| run.feasible() && run.branch_flow_mw.len() == net.branches().len())
            .map(|run| run.branch_flow_mw.as_slice());
        crate::export::gridfm_graphkit_json(net, flows)
    }

    /// The module as PowerIO IR text, from `Electrify` on. Idempotent.
    pub fn module_json(&self) -> Result<String, SynthError> {
        tellegen::ir::serialize_module(&self.module()?).map_err(SynthError::Build)
    }

    /// The typed module and the report; `Done` only.
    pub fn into_parts(self) -> Result<(PioModule<BalancedNetwork>, GenerationReport), SynthError> {
        if self.stage != Stage::Done {
            return Err(SynthError::State(format!(
                "into_parts before Done (next stage {})",
                self.stage.as_str()
            )));
        }
        let module = self
            .state
            .module
            .ok_or_else(|| SynthError::State("finalize produced no module".to_owned()))?;
        Ok((module, self.report))
    }

    /// The map preview: buses from `SiteBuses` on, branches from `Topology` on.
    pub fn preview(&self) -> Preview {
        let buses = self
            .state
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| PreviewBus {
                id: i + 1,
                lon: wrap_lon(b.lon),
                lat: b.lat,
                base_kv: b.base_kv,
                kind: b.kind,
            })
            .collect();
        let branches = if !self.state.branches.is_empty() {
            self.state
                .branches
                .iter()
                .map(|br| PreviewBranch {
                    from: br.from + 1,
                    to: br.to + 1,
                    base_kv: br.kv,
                    route: br
                        .route
                        .iter()
                        .map(|&(lon, lat)| [wrap_lon(lon), lat])
                        .collect(),
                })
                .collect()
        } else {
            self.state
                .corridors
                .iter()
                .flat_map(|c| {
                    (0..c.circuits).map(move |_| PreviewBranch {
                        from: c.a + 1,
                        to: c.b + 1,
                        base_kv: c.kv,
                        route: c
                            .route
                            .iter()
                            .map(|&(lon, lat)| [wrap_lon(lon), lat])
                            .collect(),
                    })
                })
                .collect()
        };
        Preview {
            stage: self.stage,
            buses,
            branches,
        }
    }

    /// The preview as flat typed arrays.
    pub fn preview_arrays(&self) -> PreviewArrays {
        let p = self.preview();
        let mut out = PreviewArrays::default();
        for b in &p.buses {
            out.bus_positions.push(b.lon);
            out.bus_positions.push(b.lat);
            out.bus_kinds.push(b.kind as u8);
            out.bus_kv.push(b.base_kv as f32);
        }
        for br in &p.branches {
            out.branch_start_indices
                .push((out.branch_positions.len() / 2) as u32);
            for pt in &br.route {
                out.branch_positions.push(pt[0]);
                out.branch_positions.push(pt[1]);
            }
            out.branch_kv.push(br.base_kv as f32);
        }
        out.branch_start_indices
            .push((out.branch_positions.len() / 2) as u32);
        out
    }
}

/// Run every stage and return the module and report.
pub fn generate(
    request: SynthRequest,
    places: Arc<Places>,
    osm: Option<String>,
) -> Result<(PioModule<BalancedNetwork>, GenerationReport), SynthError> {
    let mut session = Generator::new(request, places, osm)?;
    while !session.is_done() {
        session.step()?;
    }
    session.into_parts()
}
