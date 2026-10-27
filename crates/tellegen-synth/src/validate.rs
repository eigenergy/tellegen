//! Validation through the tellegen engine. The DC optimal power flow and the
//! AC power flow run on a validation copy of the network in which every
//! in-service HVDC link is replaced by a fixed transfer (a load at the sending
//! end, a fixed-output generator at the receiving end), because the engine's
//! models do not read HVDC rows. Each electrical island is solved on its own
//! with its own reference bus and the verdicts are aggregated. An infeasible
//! or non-convergent solve is reported as such; nothing is shed, capped or
//! scaled to make a number look better.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use powerio::{BalancedNetwork, BusId, BusType, GenCost, Generator, Load};
use powerio_prob::{AcPfInstance, DcOpfInstance};
use tellegen::{Iterations, Problem, SolveRequest, SolveResponse};

use crate::error::SynthError;
use crate::geom::delaunay::UnionFind;
use crate::report::{
    AcPfOutcome, AcPfReport, DcOpfReport, IslandOutcome, LmpSummary, SolveOutcome,
};
use crate::request::{Fuel, ValidationOptions};

/// A host supplied millisecond clock (wasm32 has no `Instant`).
pub type Clock = Option<fn() -> f64>;

/// Loading at or above this counts as binding.
pub const BINDING_LOADING: f64 = 0.999;

/// The DC OPF outcome with the per-row data later stages use.
#[derive(Clone, Debug)]
pub struct DcOpfRun {
    pub report: DcOpfReport,
    /// `(generator row, pg MW)` for every dispatched generator.
    pub dispatch: Vec<(usize, f64)>,
    /// Loading per branch row (`None` for rows no island solved).
    pub loading: Vec<Option<f64>>,
    /// From-end active flow per branch row, MW (`None` for rows no island solved).
    pub branch_flow_mw: Vec<Option<f64>>,
}

impl DcOpfRun {
    pub fn feasible(&self) -> bool {
        self.report.status == SolveOutcome::Optimal
    }
}

/// Replace in-service HVDC links by fixed transfers. Returns the copy and
/// the number of links substituted.
pub fn validation_copy(net: &BalancedNetwork) -> (BalancedNetwork, usize) {
    let mut copy = net.clone();
    let links: Vec<(BusId, BusId, f64, f64)> = copy
        .hvdc()
        .iter()
        .filter(|h| h.in_service)
        .map(|h| (h.from, h.to, h.pf, h.pt))
        .collect();
    if links.is_empty() {
        return (copy, 0);
    }
    for link in copy.hvdc_mut() {
        link.in_service = false;
    }
    for (k, (from, to, pf, pt)) in links.iter().enumerate() {
        let mut load = Load::new(*from, *pf, 0.0);
        load.uid = Some(format!("hvdc_transfer_load:{k}"));
        load.extras.insert(
            crate::build::EXTRAS_KEY.to_owned(),
            serde_json::json!({ "hvdc_as_fixed_transfer": true }),
        );
        copy.loads_mut().push(load);
        let mut gen = Generator::new(*to);
        gen.pmax = pt.max(0.0);
        gen.pmin = pt.max(0.0);
        gen.pg = pt.max(0.0);
        gen.vg = 1.0;
        gen.mbase = pt.abs().max(1.0);
        gen.cost = Some(GenCost::new(2, 0.0, 0.0, vec![0.0, 0.0, 0.0]));
        gen.uid = Some(format!("hvdc_transfer_gen:{k}"));
        copy.generators_mut().push(gen);
    }
    (copy, links.len())
}

/// Bus index sets of the electrical islands (in-service branches only),
/// largest first.
pub fn islands(net: &BalancedNetwork) -> Vec<Vec<usize>> {
    let n = net.buses().len();
    let index: HashMap<usize, usize> = net
        .buses()
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.0, i))
        .collect();
    let mut uf = UnionFind::new(n);
    for br in net.branches().iter().filter(|b| b.in_service) {
        if let (Some(&a), Some(&b)) = (index.get(&br.from.0), index.get(&br.to.0)) {
            uf.union(a, b);
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        groups.entry(uf.find(i)).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    out
}

/// One island as its own network, with exactly one reference bus. Returns
/// the sub-network and, in order, the full-network rows of its branches and
/// generators.
pub struct IslandNetwork {
    pub net: BalancedNetwork,
    pub branch_rows: Vec<usize>,
    pub gen_rows: Vec<usize>,
    pub bus_rows: Vec<usize>,
    pub load_mw: f64,
}

pub fn island_network(net: &BalancedNetwork, members: &[usize]) -> Option<IslandNetwork> {
    let in_island: std::collections::HashSet<usize> =
        members.iter().map(|&i| net.buses()[i].id.0).collect();
    let mut sub = BalancedNetwork::new(net.name(), net.base_mva());
    let mut buses: Vec<powerio::Bus> = members.iter().map(|&i| net.buses()[i].clone()).collect();
    let gen_rows: Vec<usize> = net
        .generators()
        .iter()
        .enumerate()
        .filter(|(_, g)| g.in_service && in_island.contains(&g.bus.0))
        .map(|(k, _)| k)
        .collect();
    if gen_rows.is_empty() {
        return None;
    }
    // Reference: keep a declared Ref if the island has exactly one; otherwise
    // the bus of the largest generator.
    let refs: Vec<usize> = buses
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BusType::Ref)
        .map(|(i, _)| i)
        .collect();
    if refs.len() != 1 {
        let best = gen_rows
            .iter()
            .max_by(|&&a, &&b| {
                net.generators()[a]
                    .pmax
                    .partial_cmp(&net.generators()[b].pmax)
                    .unwrap()
            })
            .map(|&k| net.generators()[k].bus.0)
            .unwrap();
        for b in buses.iter_mut() {
            if b.kind == BusType::Ref {
                b.kind = BusType::Pv;
            }
            if b.id.0 == best {
                b.kind = BusType::Ref;
            }
        }
    }
    *sub.buses_mut() = buses;
    let branch_rows: Vec<usize> = net
        .branches()
        .iter()
        .enumerate()
        .filter(|(_, b)| in_island.contains(&b.from.0) && in_island.contains(&b.to.0))
        .map(|(k, _)| k)
        .collect();
    *sub.branches_mut() = branch_rows
        .iter()
        .map(|&k| net.branches()[k].clone())
        .collect();
    *sub.generators_mut() = gen_rows
        .iter()
        .map(|&k| net.generators()[k].clone())
        .collect();
    let loads: Vec<Load> = net
        .loads()
        .iter()
        .filter(|l| in_island.contains(&l.bus.0))
        .cloned()
        .collect();
    let load_mw = loads.iter().filter(|l| l.in_service).map(|l| l.p).sum();
    *sub.loads_mut() = loads;
    *sub.shunts_mut() = net
        .shunts()
        .iter()
        .filter(|s| in_island.contains(&s.bus.0))
        .cloned()
        .collect();
    *sub.geo_mut() = net.geo().clone();
    Some(IslandNetwork {
        net: sub,
        branch_rows,
        gen_rows,
        bus_rows: members.to_vec(),
        load_mw,
    })
}

fn now(clock: Clock) -> Option<f64> {
    clock.map(|f| f())
}

/// Run the DC OPF on every island of `net` and aggregate.
pub fn run_dc_opf(
    net: &BalancedNetwork,
    fuels: &[Fuel],
    cancel: Option<Arc<AtomicBool>>,
    clock: Clock,
) -> Result<DcOpfRun, SynthError> {
    let started = now(clock);
    let (copy, hvdc_count) = validation_copy(net);
    let branch_uid_row: HashMap<String, usize> = copy
        .branches()
        .iter()
        .enumerate()
        .filter_map(|(k, b)| b.uid.clone().map(|u| (u, k)))
        .collect();
    let mut loading: Vec<Option<f64>> = vec![None; net.branches().len()];
    let mut branch_flow_mw: Vec<Option<f64>> = vec![None; net.branches().len()];
    let mut dispatch = Vec::new();
    let mut lmps = Vec::new();
    let mut objective = 0.0;
    let mut iterations = 0usize;
    let mut outcomes = Vec::new();
    let mut any_infeasible = false;
    let mut first_message = None;

    for (idx, members) in islands(&copy).iter().enumerate() {
        if let Some(flag) = &cancel {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(SynthError::Cancelled);
            }
        }
        let Some(island) = island_network(&copy, members) else {
            let load: f64 = copy
                .loads()
                .iter()
                .filter(|l| l.in_service && members.iter().any(|&i| copy.buses()[i].id == l.bus))
                .map(|l| l.p)
                .sum();
            let status = if members.len() == 1 && load <= 0.0 {
                "isolated_bus"
            } else {
                "no_generation"
            };
            if status == "no_generation" {
                any_infeasible = true;
                first_message.get_or_insert(format!(
                    "island {idx} ({} bus(es), {load:.1} MW) has no generation",
                    members.len()
                ));
            }
            outcomes.push(IslandOutcome {
                index: idx,
                buses: members.len(),
                branches: 0,
                generators: 0,
                load_mw: load,
                status: status.to_owned(),
                message: None,
            });
            continue;
        };
        let branch_count = island.branch_rows.len();
        let instance = DcOpfInstance::from_network(island.net.clone())
            .map_err(|e| SynthError::Engine(e.to_string()))?;
        let request = SolveRequest {
            formulation: Problem::DcOpf,
            ..Default::default()
        };
        match tellegen::solve_instance_cancellable(&instance, &request, cancel.clone()) {
            Ok(resp) => {
                objective += resp.objective.unwrap_or(0.0);
                if let Some(Iterations::Ipm(trace)) = &resp.iterations {
                    iterations += trace.len();
                }
                if let Some(flows) = &resp.flows {
                    for f in flows {
                        let row = f
                            .uid
                            .as_ref()
                            .and_then(|u| branch_uid_row.get(u).copied())
                            .or_else(|| island.branch_rows.get(f.branch.wrapping_sub(1)).copied());
                        if let Some(row) = row {
                            if row < loading.len() {
                                loading[row] = Some(f.loading);
                                branch_flow_mw[row] = Some(f.pf);
                            }
                        }
                    }
                }
                if let Some(d) = &resp.dispatch {
                    for g in d {
                        if let Some(&row) = island.gen_rows.get(g.gen.wrapping_sub(1)) {
                            if row < net.generators().len() {
                                dispatch.push((row, g.pg));
                            }
                        }
                    }
                }
                if let Some(l) = &resp.lmp {
                    lmps.extend(l.iter().map(|b| b.value));
                }
                outcomes.push(IslandOutcome {
                    index: idx,
                    buses: members.len(),
                    branches: branch_count,
                    generators: island.gen_rows.len(),
                    load_mw: island.load_mw,
                    status: "optimal".to_owned(),
                    message: None,
                });
            }
            Err(message) => {
                if message.contains("cancel") {
                    return Err(SynthError::Cancelled);
                }
                any_infeasible = true;
                first_message.get_or_insert(message.clone());
                outcomes.push(IslandOutcome {
                    index: idx,
                    buses: members.len(),
                    branches: branch_count,
                    generators: island.gen_rows.len(),
                    load_mw: island.load_mw,
                    status: "infeasible".to_owned(),
                    message: Some(message),
                });
            }
        }
    }

    let solved: Vec<f64> = loading.iter().flatten().copied().collect();
    let mut histogram = vec![0u32; 10];
    for &l in &solved {
        let bin = ((l * 10.0).floor() as usize).min(9);
        histogram[bin] += 1;
    }
    let mut sorted = solved.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p90 = if sorted.is_empty() {
        None
    } else {
        Some(sorted[((sorted.len() - 1) as f64 * 0.9).round() as usize])
    };
    let binding: Vec<String> = loading
        .iter()
        .enumerate()
        .filter(|(_, l)| l.is_some_and(|l| l >= BINDING_LOADING))
        .filter_map(|(k, _)| net.branches()[k].uid.clone())
        .collect();
    let mut branch_loading = BTreeMap::new();
    for (k, l) in loading.iter().enumerate() {
        if let (Some(l), Some(uid)) = (l, &net.branches()[k].uid) {
            branch_loading.insert(uid.clone(), *l);
        }
    }
    let lmp = if lmps.is_empty() {
        None
    } else {
        let min = lmps.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = lmps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Some(LmpSummary {
            min,
            mean: lmps.iter().sum::<f64>() / lmps.len() as f64,
            max,
            spread: max - min,
        })
    };
    let status = if any_infeasible {
        SolveOutcome::Infeasible
    } else {
        SolveOutcome::Optimal
    };
    let report = DcOpfReport {
        status,
        message: first_message,
        objective: (status == SolveOutcome::Optimal).then_some(objective),
        iterations: (iterations > 0).then_some(iterations),
        max_loading: sorted.last().copied(),
        loading_p90: p90,
        loading_mean: (!solved.is_empty())
            .then(|| solved.iter().sum::<f64>() / solved.len() as f64),
        loading_histogram: histogram,
        binding_fraction: if solved.is_empty() {
            0.0
        } else {
            binding.len() as f64 / solved.len() as f64
        },
        binding_branches: binding,
        lmp,
        dispatch_by_fuel_mw: crate::build::dispatch_by_fuel(fuels, &dispatch),
        unserved_mw: (status == SolveOutcome::Optimal).then_some(0.0),
        islands: outcomes,
        hvdc_as_fixed_transfer: hvdc_count,
        solve_ms: started.and_then(|s| now(clock).map(|e| e - s)),
        branch_loading,
    };
    Ok(DcOpfRun {
        report,
        dispatch,
        loading,
        branch_flow_mw,
    })
}

/// Run the AC power flow on every island of `net` (generator `pg` set points
/// as stored) and aggregate.
pub fn run_ac_pf(net: &BalancedNetwork, options: &ValidationOptions, clock: Clock) -> AcPfReport {
    let started = now(clock);
    let (copy, _) = validation_copy(net);
    let mut outcomes = Vec::new();
    let mut vm_min = f64::INFINITY;
    let mut vm_max = f64::NEG_INFINITY;
    let mut outside_planning = 0usize;
    let mut outside_emergency = 0usize;
    let mut losses = 0.0;
    let mut newton = 0usize;
    let mut residual: f64 = 0.0;
    let mut any_failed = false;
    let mut first_message = None;
    let mut any_solved = false;
    for (idx, members) in islands(&copy).iter().enumerate() {
        let Some(island) = island_network(&copy, members) else {
            outcomes.push(IslandOutcome {
                index: idx,
                buses: members.len(),
                branches: 0,
                generators: 0,
                load_mw: 0.0,
                status: if members.len() == 1 {
                    "isolated_bus".to_owned()
                } else {
                    "no_generation".to_owned()
                },
                message: None,
            });
            continue;
        };
        let result = AcPfInstance::from_network(island.net.clone())
            .map_err(|e| e.to_string())
            .and_then(|instance| {
                let request = SolveRequest {
                    formulation: Problem::AcPf,
                    ..Default::default()
                };
                tellegen::solve_ac_pf_instance(&instance, &request)
            });
        match result {
            Ok(resp) => {
                any_solved = true;
                summarise_ac(
                    &resp,
                    options,
                    &mut vm_min,
                    &mut vm_max,
                    &mut outside_planning,
                    &mut outside_emergency,
                    &mut losses,
                );
                if let Some(Iterations::Newton { count, residual: r }) = &resp.iterations {
                    newton = newton.max(*count);
                    residual = residual.max(*r);
                }
                outcomes.push(IslandOutcome {
                    index: idx,
                    buses: members.len(),
                    branches: island.branch_rows.len(),
                    generators: island.gen_rows.len(),
                    load_mw: island.load_mw,
                    status: "converged".to_owned(),
                    message: None,
                });
            }
            Err(message) => {
                any_failed = true;
                first_message.get_or_insert(message.clone());
                outcomes.push(IslandOutcome {
                    index: idx,
                    buses: members.len(),
                    branches: island.branch_rows.len(),
                    generators: island.gen_rows.len(),
                    load_mw: island.load_mw,
                    status: "not_converged".to_owned(),
                    message: Some(message),
                });
            }
        }
    }
    let status = if any_failed {
        AcPfOutcome::NotConverged
    } else {
        AcPfOutcome::Converged
    };
    AcPfReport {
        status,
        message: first_message,
        newton_iterations: any_solved.then_some(newton),
        residual: any_solved.then_some(residual),
        vm_min: any_solved.then_some(vm_min),
        vm_max: any_solved.then_some(vm_max),
        buses_outside_planning_band: outside_planning,
        buses_outside_emergency_band: outside_emergency,
        losses_mw: any_solved.then_some(losses),
        islands: outcomes,
        solve_ms: started.and_then(|s| now(clock).map(|e| e - s)),
    }
}

fn summarise_ac(
    resp: &SolveResponse,
    options: &ValidationOptions,
    vm_min: &mut f64,
    vm_max: &mut f64,
    outside_planning: &mut usize,
    outside_emergency: &mut usize,
    losses: &mut f64,
) {
    if let Some(vm) = &resp.vm {
        for b in vm {
            *vm_min = vm_min.min(b.value);
            *vm_max = vm_max.max(b.value);
            if b.value < options.voltage_band[0] || b.value > options.voltage_band[1] {
                *outside_planning += 1;
            }
            if b.value < options.emergency_band[0] || b.value > options.emergency_band[1] {
                *outside_emergency += 1;
            }
        }
    }
    if let Some(inj) = &resp.injections {
        *losses += inj.iter().map(|i| i.p).sum::<f64>();
    }
}

/// Write a DC dispatch back onto the generator set points.
pub fn apply_dispatch(net: &mut BalancedNetwork, dispatch: &[(usize, f64)]) {
    let gens = net.generators_mut();
    for &(row, pg) in dispatch {
        if let Some(g) = gens.get_mut(row) {
            g.pg = pg.clamp(g.pmin.min(g.pmax), g.pmax);
        }
    }
}

/// A skipped DC OPF block.
pub fn skipped_dc() -> DcOpfReport {
    DcOpfReport {
        status: SolveOutcome::Skipped,
        message: None,
        objective: None,
        iterations: None,
        max_loading: None,
        loading_p90: None,
        loading_mean: None,
        loading_histogram: vec![0; 10],
        binding_branches: Vec::new(),
        binding_fraction: 0.0,
        lmp: None,
        dispatch_by_fuel_mw: BTreeMap::new(),
        unserved_mw: None,
        islands: Vec::new(),
        hvdc_as_fixed_transfer: 0,
        solve_ms: None,
        branch_loading: BTreeMap::new(),
    }
}

/// A skipped AC PF block.
pub fn skipped_ac() -> AcPfReport {
    AcPfReport {
        status: AcPfOutcome::Skipped,
        message: None,
        newton_iterations: None,
        residual: None,
        vm_min: None,
        vm_max: None,
        buses_outside_planning_band: 0,
        buses_outside_emergency_band: 0,
        losses_mw: None,
        islands: Vec::new(),
        solve_ms: None,
    }
}
