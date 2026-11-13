//! Building the RMS system from a PSS/E-style case and initializing it from
//! the AC power flow.
//!
//! The device mapping follows PowerSystems.jl's DYR reader
//! (`src/parsers/psse_dynamic_data.jl`): a generator without a machine
//! record, or with a `GENCLS` of zero inertia, becomes a Thevenin source
//! behind its `ZSORCE`; a missing exciter holds the field voltage, a missing
//! governor holds the mechanical torque. Initialization reaches the same
//! steady state as PowerSimulationsDynamics.jl (`src/initialization/*`): each
//! generator is solved for its states and references such that all
//! derivatives vanish and it injects exactly its power flow current.

use powerio::{BalancedNetwork, DynamicCase, MachineDynamics};

use super::graph::{CompiledGraph, ModelOverride, Slot};
use super::models::{
    avr_from_psse, governor_from_psse, machine_from_psse, ri_dq, stabilizer_from_psse, Avr,
    Governor, Machine, Stabilizer,
};
use super::system::{BusLoad, DynSystem, GenDevice, HeldUnit, Source, C64};
use super::LoadModel;
use crate::formulation::AcPolar;
use crate::model::AcNetwork;
use crate::problem::{ac_pf_with_start, AcStart};

/// Initialization diagnostics shown to the caller.
#[derive(Clone, Debug, Default)]
pub(crate) struct InitReport {
    pub notes: Vec<String>,
    pub pf_iterations: usize,
    pub pf_residual: f64,
    pub steady_residual: f64,
}

/// Build the system and its initial state.
pub(crate) fn build(
    network: &BalancedNetwork,
    dynamics: &DynamicCase,
    models: &[ModelOverride],
    load_model: LoadModel,
    options: super::RmsOptions,
) -> Result<(DynSystem, Vec<f64>, InitReport), String> {
    let mut report = InitReport::default();
    let (ac, pf, zip_iterations) = power_flow(network)?;
    if zip_iterations > 0 {
        report.notes.push(format!(
            "voltage-dependent (ZIP/exponential) loads: power flow converged with them in {zip_iterations} fixed-point iteration(s); the dynamic load model applies to their power at the power flow voltage"
        ));
    }
    let remote = network
        .generators()
        .iter()
        .filter(|g| g.in_service && g.regulated_bus.is_some_and(|r| r != g.bus))
        .count();
    if remote > 0 {
        report.notes.push(format!(
            "{remote} generator(s) regulate a remote bus: their terminals are held at the case's voltage, which puts the regulated buses at their setpoints in a solved case"
        ));
    }
    report.pf_iterations = pf.iterations;
    report.pf_residual = pf.residual;
    if !(pf.residual < 1e-6) {
        return Err(format!(
            "the AC power flow did not converge (residual {:.3e})",
            pf.residual
        ));
    }
    let n = ac.n;
    let sbase = network.base_mva();
    let f0 = network.base_frequency();
    let bus_numbers: Vec<usize> = ac.bus_ids.clone();
    let dense = |number: usize| bus_numbers.iter().position(|&b| b == number);
    let v: Vec<C64> = (0..n)
        .map(|b| C64::from_polar(pf.vm[b], pf.va[b]))
        .collect();

    // Branch keys for events and flows.
    let circuits = powerio::psse_branch_circuit_ids(network);
    let branch_keys: Vec<(usize, usize, String)> = ac
        .branch_ids
        .iter()
        .map(|&id| {
            let row = id - 1;
            let br = &network.branches()[row];
            (
                br.from.0,
                br.to.0,
                circuits.get(row).cloned().unwrap_or_else(|| "1".into()),
            )
        })
        .collect();

    // Loads at the power flow voltage.
    let mut loads = vec![BusLoad::default(); n];
    for b in 0..n {
        let (p, q) = (ac.pd[b], ac.qd[b]);
        let l = &mut loads[b];
        l.v0 = pf.vm[b];
        match load_model {
            LoadModel::ConstantImpedance => {
                l.p_z = p;
                l.q_z = q;
            }
            LoadModel::ConstantCurrent => {
                l.p_i = p;
                l.q_i = q;
            }
            LoadModel::ConstantPower => {
                l.p_p = p;
                l.q_p = q;
            }
        }
    }

    // Generator dispatch from the power flow: the bus generation is the net
    // injection plus the bus demand, shared among the bus's generators.
    let linking = powerio::link_machines(network, dynamics);
    // Records without a typed view are summarized per model; the generators
    // they belong to are remembered so they are not turned into stiff
    // voltage sources below.
    let mut untyped: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut out_of_service = 0usize;
    for issue in &linking.issues {
        match issue {
            powerio::LinkIssue::Untyped {
                error: powerio::ModelError::Unsupported { model_name },
                ..
            } => *untyped.entry(model_name.clone()).or_default() += 1,
            powerio::LinkIssue::OutOfService { .. } => out_of_service += 1,
            other => report.notes.push(format!("dyr {other}")),
        }
    }
    if out_of_service > 0 {
        report.notes.push(format!(
            "dyr: {out_of_service} record(s) name out-of-service generators and are not simulated"
        ));
    }
    let plant_model = |m: &str| matches!(m, "REGCA1" | "REECB1" | "REPCA1");
    for (model, count) in untyped
        .iter()
        .filter(|(m, _)| !(options.renewable_plants && plant_model(m)))
    {
        report.notes.push(format!(
            "dyr: {count} {model} record(s) have no model in tellegen and are not simulated"
        ));
    }
    let unsupported_units: std::collections::BTreeSet<(usize, String)> = dynamics
        .records()
        .iter()
        .filter(|r| {
            matches!(
                powerio::typed_model(r),
                Err(powerio::ModelError::Unsupported { .. })
            )
        })
        .filter_map(|r| {
            Some((
                r.bus_id()? as usize,
                r.device_id().unwrap_or("1").trim().to_owned(),
            ))
        })
        .collect();
    // Renewable plants: REGCA1 + REECB1 + REPCA1 of one unit, simulated
    // ([`super::renewable`]); a unit with other inverter records, or with a
    // record that does not parse, keeps its power flow output.
    let mut plant_records: std::collections::BTreeMap<
        (usize, String),
        [Option<&powerio::DynamicRecord>; 3],
    > = std::collections::BTreeMap::new();
    for r in dynamics.records() {
        let slot = match r.model_name().trim().to_ascii_uppercase().as_str() {
            "REGCA1" => 0,
            "REECB1" => 1,
            "REPCA1" => 2,
            _ => continue,
        };
        let Some(bus) = r.bus_id() else { continue };
        let id = r.device_id().unwrap_or("1").trim().to_owned();
        plant_records.entry((bus as usize, id)).or_default()[slot] = Some(r);
    }
    let mut dynamic_plants = 0usize;
    let simulate_plants = options.renewable_plants;
    let mut static_units = 0usize;
    let mut at_bus: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut entries: Vec<(&MachineDynamics, usize)> = Vec::new();
    for m in &linking.machines {
        let Some(b) = dense(m.bus) else {
            report.notes.push(format!(
                "generator {}-{} sits on a bus outside the energized network; skipped",
                m.bus, m.machine_id
            ));
            continue;
        };
        at_bus[b].push(entries.len());
        entries.push((m, b));
    }
    let mut dispatch: Vec<C64> = vec![C64::new(0.0, 0.0); entries.len()];
    for b in 0..n {
        let gens = &at_bus[b];
        if gens.is_empty() {
            continue;
        }
        let p_bus = pf.p[b] + ac.pd[b];
        let q_bus = pf.q[b] + ac.qd[b];
        let sched: Vec<f64> = gens
            .iter()
            .map(|&k| network.generators()[entries[k].0.generator].pg / sbase)
            .collect();
        let total: f64 = sched.iter().sum();
        let qcap: Vec<f64> = gens
            .iter()
            .map(|&k| {
                let g = &network.generators()[entries[k].0.generator];
                (g.qmax - g.qmin).abs().max(1e-6)
            })
            .collect();
        let qtotal: f64 = qcap.iter().sum();
        // Reactive power. A solved case states every unit's reactive power, and
        // its units keep that dispatch, scaled to the solved bus total (as
        // PowerSimulationsDynamics.jl and PSS/E start from it). Only where the
        // case's values do not add up to the bus total (an unsolved case, or
        // none given) is the total shared in proportion to the reactive range.
        let qsched: Vec<f64> = gens
            .iter()
            .map(|&k| network.generators()[entries[k].0.generator].qg / sbase)
            .collect();
        let qsum: f64 = qsched.iter().sum();
        let qabs: f64 = qsched.iter().map(|q| q.abs()).sum();
        let case_dispatch = qsum.abs() > 1e-9 && (q_bus - qsum).abs() <= 0.05 * qabs;
        // The mismatch goes to the units in proportion to their |Q|: the same
        // as scaling when all have one sign, and bounded when they do not.
        let mut q_unit: Vec<f64> = if case_dispatch {
            qsched
                .iter()
                .map(|q| q + (q_bus - qsum) * q.abs() / qabs)
                .collect()
        } else {
            qcap.iter().map(|c| q_bus * c / qtotal).collect()
        };
        if case_dispatch {
            // A unit the scaling pushes past a reactive limit stays at the
            // limit; the others take the rest in proportion to their dispatch.
            let lim: Vec<(f64, f64)> = gens
                .iter()
                .map(|&k| {
                    let g = &network.generators()[entries[k].0.generator];
                    (g.qmin.min(g.qmax) / sbase, g.qmin.max(g.qmax) / sbase)
                })
                .collect();
            let scaled = q_unit.clone();
            let mut fixed = vec![false; gens.len()];
            for _ in 0..gens.len() {
                let mut clipped = false;
                for j in 0..gens.len() {
                    if !fixed[j] && (q_unit[j] < lim[j].0 || q_unit[j] > lim[j].1) {
                        q_unit[j] = q_unit[j].clamp(lim[j].0, lim[j].1);
                        fixed[j] = true;
                        clipped = true;
                    }
                }
                if !clipped {
                    break;
                }
                let held: f64 = (0..gens.len())
                    .filter(|&j| fixed[j])
                    .map(|j| q_unit[j])
                    .sum();
                let free: f64 = (0..gens.len())
                    .filter(|&j| !fixed[j])
                    .map(|j| qsched[j])
                    .sum();
                let free_abs: f64 = (0..gens.len())
                    .filter(|&j| !fixed[j])
                    .map(|j| qsched[j].abs())
                    .sum();
                if free_abs <= 1e-9 {
                    break;
                }
                for j in (0..gens.len()).filter(|&j| !fixed[j]) {
                    q_unit[j] = qsched[j] + (q_bus - held - free) * qsched[j].abs() / free_abs;
                }
            }
            // No unit left to take the rest (a single unit, or all at a limit):
            // the bus total comes first, the units keep their scaled dispatch.
            if (q_unit.iter().sum::<f64>() - q_bus).abs() > 1e-9 * (1.0 + q_bus.abs()) {
                q_unit = scaled;
            }
        }
        for (j, &k) in gens.iter().enumerate() {
            let share_p = if total.abs() > 1e-12 {
                sched[j] / total
            } else {
                1.0 / gens.len() as f64
            };
            dispatch[k] = C64::new(p_bus * share_p, q_unit[j]);
        }
    }

    let mut devices: Vec<GenDevice> = Vec::new();
    let mut device_x0: Vec<Vec<f64>> = Vec::new();
    let mut sources: Vec<Source> = Vec::new();
    let mut held: Vec<HeldUnit> = Vec::new();
    let mut held_x0: Vec<(usize, Vec<f64>)> = Vec::new();
    let mut nx = 2 * n;
    for (k, &(m, b)) in entries.iter().enumerate() {
        let s = dispatch[k];
        let i_sys = (s / v[b]).conj();
        let scale = m.mbase / sbase;
        // A unit whose only dynamic records are models tellegen does not
        // simulate (an inverter chain such as REGCA1/REECB1/REPCA1) keeps its
        // power flow output as a constant-current injection.
        if m.machine.is_none() && unsupported_units.contains(&(m.bus, m.machine_id.clone())) {
            let unit = (m.bus, m.machine_id.clone());
            let plant = match plant_records.get(&unit) {
                Some([Some(gc), Some(ec), Some(pc)]) if simulate_plants => {
                    let others = dynamics.records().iter().any(|r| {
                        r.bus_id().map(|b| b as usize) == Some(m.bus)
                            && r.device_id().unwrap_or("1").trim() == m.machine_id
                            && !matches!(
                                r.model_name().trim().to_ascii_uppercase().as_str(),
                                "REGCA1" | "REECB1" | "REPCA1"
                            )
                    });
                    let built = (|| -> Result<_, String> {
                        if others {
                            return Err("other inverter records of the unit have no model".into());
                        }
                        let mut regc = super::renewable::Regca1::from_tokens(gc.parameters())?;
                        regc.psid_lv_gain = options.psid_lv_gain;
                        let mut p = super::renewable::Plant {
                            regc,
                            reec: super::renewable::Reecb1::from_tokens(ec.parameters())?,
                            repc: super::renewable::Repca1::from_tokens(pc.parameters())?,
                            scale,
                            r_source: 0.0,
                            x_source: 1e5,
                            p_ref: 0.0,
                            q_ref: 0.0,
                            v_ref: 1.0,
                            v_ref0: 1.0,
                            q_ref_converter: 0.0,
                            omega_ref: 1.0,
                        };
                        let (x, notes) = p.initialize(s, v[b])?;
                        Ok((p, x, notes))
                    })();
                    match built {
                        Ok((p, x, notes)) => {
                            for note in notes {
                                report.notes.push(format!(
                                    "renewable plant {}-{}: {note}",
                                    m.bus, m.machine_id
                                ));
                            }
                            Some((p, x))
                        }
                        Err(e) => {
                            report.notes.push(format!(
                                "renewable plant {}-{} is held at its power flow output: {e}",
                                m.bus, m.machine_id
                            ));
                            None
                        }
                    }
                }
                _ => None,
            };
            let (plant, offset) = match plant {
                Some((p, x)) => {
                    let off = nx;
                    nx += x.len();
                    held_x0.push((off, x));
                    dynamic_plants += 1;
                    (Some(Box::new(p)), off)
                }
                None => {
                    loads[b].p_i -= s.re;
                    loads[b].q_i -= s.im;
                    static_units += 1;
                    (None, 0)
                }
            };
            held.push(HeldUnit {
                bus_number: m.bus,
                machine_id: m.machine_id.clone(),
                bus: b,
                s,
                v0: pf.vm[b],
                mbase: m.mbase,
                tripped: false,
                plant,
                offset,
            });
            continue;
        }
        let as_source = match m.machine.as_ref().map(|l| &l.model) {
            None => true,
            Some(powerio::DynamicModel::Gencls(g)) => g.h == 0.0,
            Some(_) => false,
        };
        if as_source {
            let z = C64::new(m.zsource.0, m.zsource.1) / scale;
            let e = v[b] + z * i_sys;
            sources.push(Source {
                bus_number: m.bus,
                machine_id: m.machine_id.clone(),
                bus: b,
                y: C64::new(1.0, 0.0) / z,
                e,
            });
            if m.machine.is_none() {
                report.notes.push(format!(
                    "generator {}-{} has no machine record and is held as a voltage source behind ZSORCE",
                    m.bus, m.machine_id
                ));
            }
            continue;
        }
        let model = m
            .machine
            .as_ref()
            .map(|l| l.model.clone())
            .expect("checked above");
        let (machine, shaft) = machine_from_psse(&model, m.zsource)?;
        let avr = m
            .exciter
            .as_ref()
            .map(|l| avr_from_psse(&l.model))
            .transpose()?
            .unwrap_or(Avr::Fixed);
        let gov = m
            .governor
            .as_ref()
            .map(|l| governor_from_psse(&l.model))
            .transpose()?
            .unwrap_or(Governor::Fixed);
        let pss = m
            .stabilizer
            .as_ref()
            .map(|l| stabilizer_from_psse(&l.model))
            .transpose()?
            .unwrap_or(Stabilizer::None);
        // Controller graphs given for this generator replace its records.
        let graph = |slot: Slot| -> Result<Option<Box<CompiledGraph>>, String> {
            models
                .iter()
                .find(|o| o.bus == m.bus && o.machine_id == m.machine_id && o.graph.slot == slot)
                .map(|o| {
                    CompiledGraph::new(&o.graph).map(Box::new).map_err(|e| {
                        format!("generator {}-{} {slot:?} graph: {e}", m.bus, m.machine_id)
                    })
                })
                .transpose()
        };
        let avr = graph(Slot::Exciter)?.map_or(avr, Avr::Graph);
        let gov = graph(Slot::Governor)?.map_or(gov, Governor::Graph);
        let pss = graph(Slot::Stabilizer)?.map_or(pss, Stabilizer::Graph);
        let mut dev = GenDevice {
            bus_number: m.bus,
            machine_id: m.machine_id.clone(),
            bus: b,
            scale,
            f0,
            machine,
            shaft,
            avr,
            gov,
            pss,
            v_ref: 1.0,
            p_ref: 1.0,
            offset: nx,
            tripped: None,
        };
        nx += dev.n_states();
        device_x0.push(init_device(&mut dev, v[b], i_sys)?);
        devices.push(dev);
    }

    if static_units > 0 {
        report.notes.push(format!(
            "{static_units} generator(s) with only unsupported dynamic models are held as constant-current injections at their power flow output"
        ));
    }
    if dynamic_plants > 0 {
        report.notes.push(format!(
            "{dynamic_plants} renewable plant(s) simulated as REGCA1 + REECB1 + REPCA1"
        ));
    }
    // Reference frame: the speed of a dynamic machine at the slack bus, else
    // the constant synchronous frame.
    let omega_ref = ac.slacks.first().and_then(|&slack| {
        if sources.iter().any(|s| s.bus == slack) {
            None
        } else {
            devices
                .iter()
                .find(|d| d.bus == slack)
                .map(|d| d.offset + d.layout()[1] + 1)
        }
    });

    let m_branches = ac.m;
    let mut sys = DynSystem {
        n,
        bus_numbers,
        sbase,
        f0,
        ac,
        branch_keys,
        devices,
        sources,
        loads,
        fault_y: vec![C64::new(0.0, 0.0); n],
        ybus: Vec::new(),
        nx,
        mass: Vec::new(),
        omega_ref,
        epoch: 0,
        end_open: vec![[false; 2]; m_branches],
        branch_faults: vec![None; m_branches],
        held,
        factorize_by_blocks: options.factorize_by_blocks,
    };
    sys.rebuild_ybus();
    sys.rebuild_mass();

    let mut x0 = vec![0.0; nx];
    for b in 0..n {
        x0[b] = v[b].re;
        x0[n + b] = v[b].im;
    }
    for (d, xd) in sys.devices.iter().zip(&device_x0) {
        x0[d.offset..d.offset + xd.len()].copy_from_slice(xd);
    }
    for (off, xh) in &held_x0 {
        x0[*off..*off + xh.len()].copy_from_slice(xh);
    }
    let mut f = vec![0.0; nx];
    sys.residual(&x0, &mut f, None);
    report.steady_residual = f.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
    if report.steady_residual > 1e-6 {
        return Err(format!(
            "initialization left a steady-state residual of {:.3e}",
            report.steady_residual
        ));
    }
    Ok((sys, x0, report))
}

/// The AC power flow, honoring voltage-dependent loads: tellegen's AC power
/// flow takes constant-power loads, so ZIP and exponential loads are
/// evaluated at the current voltages and the flow is re-solved until the
/// voltages settle ([`crate::model::load_voltage::settle`]). Returns the
/// number of fixed-point iterations (0 when every load is constant power).
pub(crate) fn power_flow(
    network: &BalancedNetwork,
) -> Result<(AcNetwork, crate::problem::AcPfSolution, usize), String> {
    let ((ac, pf), iterations) = crate::model::load_voltage::settle(network, |net| {
        let mut ac = AcNetwork::from_network(net)?;
        hold_remote_regulators(net, &mut ac);
        // The dynamic initialization starts from the case as solved: a
        // generator keeps its voltage setpoint whatever reactive power that
        // takes (no PV→PQ switching at its limits), as PSS/E initializes a
        // dynamic run from a solved case and PowerSimulationsDynamics.jl does.
        ac.qmin.iter_mut().for_each(|q| *q = f64::NEG_INFINITY);
        ac.qmax.iter_mut().for_each(|q| *q = f64::INFINITY);
        let pf = ac_pf_with_start(&AcPolar, &ac, AcStart::Case)?;
        let vm = ac
            .bus_ids
            .iter()
            .copied()
            .zip(pf.vm.iter().copied())
            .collect();
        Ok(((ac, pf), vm))
    })?;
    Ok((ac, pf, iterations))
}

/// A generator that regulates a remote bus (PSS/E `IREG`) holds its own
/// terminal at the case's voltage there instead of its setpoint `VS`: in a
/// solved case that puts the regulated bus at `VS`, as the case states (and
/// as PowerSystems.jl initializes it). The static AC power flow still
/// refuses remote regulation; the dynamic initialization starts from the
/// case's operating point. Returns the number of such terminals.
pub(crate) fn hold_remote_regulators(net: &BalancedNetwork, ac: &mut AcNetwork) -> usize {
    let mut held = 0;
    for g in net.generators().iter().filter(|g| g.in_service) {
        if g.regulated_bus.is_none_or(|r| r == g.bus) {
            continue;
        }
        let Some(k) = ac.bus_ids.iter().position(|&b| b == g.bus.0) else {
            continue;
        };
        let Some(bus) = net.buses().iter().find(|b| b.id == g.bus) else {
            continue;
        };
        if bus.vm > 0.0 && ac.vm_set[k] != bus.vm {
            ac.vm_set[k] = bus.vm;
            held += 1;
        }
    }
    held
}

/// Infinity norm that propagates NaN (`f64::max` would drop it).
fn inf_norm(r: &[f64]) -> f64 {
    r.iter().fold(0.0_f64, |a, v| {
        if v.is_nan() || a.is_nan() {
            f64::NAN
        } else {
            a.max(v.abs())
        }
    })
}

/// Dense Newton with a forward-difference Jacobian on a small square system.
pub(crate) fn newton(
    mut u: Vec<f64>,
    mut g: impl FnMut(&[f64], &mut [f64]),
    what: &str,
) -> Result<Vec<f64>, String> {
    let k = u.len();
    let mut r = vec![0.0; k];
    let mut r1 = vec![0.0; k];
    for _ in 0..60 {
        g(&u, &mut r);
        let norm = inf_norm(&r);
        if !norm.is_finite() {
            return Err(format!("{what}: initialization diverged"));
        }
        if norm < 1e-12 {
            return Ok(u);
        }
        let mut j = faer::Mat::<f64>::zeros(k, k);
        for c in 0..k {
            let h = 1e-7 * u[c].abs().max(1.0);
            let keep = u[c];
            u[c] += h;
            g(&u, &mut r1);
            u[c] = keep;
            for row in 0..k {
                j[(row, c)] = (r1[row] - r[row]) / h;
            }
        }
        let rhs = faer::Mat::<f64>::from_fn(k, 1, |i, _| -r[i]);
        let lu = j.full_piv_lu();
        use faer::linalg::solvers::Solve;
        let mut du = lu.solve(&rhs);
        // A singular Jacobian (an unknown the residual does not depend on,
        // as a block-graph governor at its load limit) takes the regularized
        // least-squares step instead.
        if (0..k).any(|i| !du[(i, 0)].is_finite()) {
            let flat: Vec<f64> = (0..k * k).map(|i| j[(i / k, i % k)]).collect();
            if let Some(step) = super::graph::least_squares_step(&flat, &r, k) {
                du = faer::Mat::<f64>::from_fn(k, 1, |i, _| step[i]);
            }
        }
        let mut step = 1.0;
        let base = u.clone();
        loop {
            for i in 0..k {
                u[i] = base[i] + step * du[(i, 0)];
            }
            g(&u, &mut r1);
            let n1 = inf_norm(&r1);
            // A non-finite trial residual halves the step too; below the
            // minimum the iteration goes on (and reports the divergence).
            if (n1.is_finite() && n1 < norm) || step < 1e-3 {
                break;
            }
            step *= 0.5;
        }
    }
    g(&u, &mut r);
    let norm = inf_norm(&r);
    if norm < 1e-8 {
        Ok(u)
    } else {
        Err(format!(
            "{what}: initialization did not converge (residual {norm:.3e})"
        ))
    }
}

/// Solve one generator's steady state: all derivatives zero at `ω = 1` and
/// its injection equal to the power flow current `i_sys`. Sets `v_ref`,
/// `p_ref` (and the classical `e'q`) and returns the local state vector.
pub(crate) fn init_device(dev: &mut GenDevice, v: C64, i_sys: C64) -> Result<Vec<f64>, String> {
    let name = format!("generator {}", dev.name());
    let k = dev.n_states();
    let [m, s, a, g, p] = dev.layout();
    let nm = dev.machine.n_states();
    let i_m = i_sys / dev.scale;

    // Stage 1: machine and shaft with the field voltage and torque free.
    let (r, xq) = match &dev.machine {
        Machine::Classic { r, xd_p, .. } => (*r, *xd_p),
        Machine::RoundRotor(mm) => (mm.r, mm.xq),
        Machine::SalientPole(mm) => (mm.r, mm.xq),
    };
    let e_q = v + C64::new(r, xq) * i_m;
    let delta0 = e_q.arg();
    let (id, iq) = ri_dq(delta0, i_m.re, i_m.im);
    let (vd, vq) = ri_dq(delta0, v.re, v.im);
    let mut guess = vec![0.0; nm + 3];
    match &dev.machine {
        Machine::Classic { .. } => {
            guess[nm + 1] = e_q.norm();
        }
        Machine::RoundRotor(mm) => {
            let eq_p = vq + mm.r * iq + mm.xd_p * id;
            let ed_p = vd + mm.r * id - mm.xq_p * iq;
            guess[0] = eq_p;
            guess[1] = ed_p;
            guess[2] = eq_p - (mm.xd_p - mm.xl) * id;
            guess[3] = ed_p + (mm.xq_p - mm.xl) * iq;
            guess[nm + 1] = eq_p + (mm.xd - mm.xd_p) * id;
        }
        Machine::SalientPole(mm) => {
            let eq_p = vq + mm.r * iq + mm.xd_p * id;
            guess[0] = eq_p;
            guess[1] = eq_p - (mm.xd_p - mm.xl) * id;
            guess[2] = if mm.exponential {
                (mm.xq - mm.xd_pp) * iq
            } else {
                -(mm.xq - mm.xd_pp) * iq
            };
            guess[nm + 1] = eq_p + (mm.xd - mm.xd_p) * id;
        }
    }
    guess[nm] = delta0;
    guess[nm + 2] = (v * i_m.conj()).re;
    let machine = dev.machine.clone();
    let shaft = dev.shaft;
    let classic = matches!(machine, Machine::Classic { .. });
    let u = newton(
        guess,
        |u, res| {
            let mut mach = machine.clone();
            let vf = if classic {
                if let Machine::Classic { eq_p, .. } = &mut mach {
                    *eq_p = u[nm + 1];
                }
                0.0
            } else {
                u[nm + 1]
            };
            let delta = u[nm];
            let alg = mach.algebraic(&u[..nm], delta, v.re, v.im);
            mach.ode(&u[..nm], &alg, vf, &mut res[..nm]);
            let (_, fw) = shaft.ode(1.0, 1.0, alg.te, u[nm + 2], dev.f0);
            res[nm] = fw;
            let (ir, ii) = super::models::dq_ri(delta, alg.id, alg.iq);
            res[nm + 1] = ir - i_m.re;
            res[nm + 2] = ii - i_m.im;
        },
        &name,
    )?;
    if let Machine::Classic { eq_p, .. } = &mut dev.machine {
        *eq_p = u[nm + 1];
    }
    let vf0 = if classic { 0.0 } else { u[nm + 1] };
    let tm0 = u[nm + 2];
    let mut x = vec![0.0; k];
    x[m..s].copy_from_slice(&u[..nm]);
    x[s] = u[nm];
    x[s + 1] = 1.0;
    let alg = dev.machine.algebraic(&x[m..s], x[s], v.re, v.im);
    let vt = v.norm();

    // Stage 2a: stabilizer at rest.
    if dev.pss.n_states() > 0 {
        let pss = dev.pss.clone();
        let pe = alg.te * dev.scale;
        let xp = newton(
            pss_start(&pss, pe, vt, dev.scale),
            |u, res| {
                pss.eval(u, 1.0, pe, vt, dev.scale, res);
            },
            &name,
        )?;
        x[p..].copy_from_slice(&xp);
    }
    let mut scratch = vec![0.0; dev.pss.n_states()];
    let vs0 = dev.pss.eval(
        &x[p..],
        1.0,
        alg.te * dev.scale,
        vt,
        dev.scale,
        &mut scratch,
    );

    // Stage 2b: exciter states and voltage reference for the field voltage.
    match &dev.avr {
        Avr::Fixed => dev.v_ref = vf0,
        avr => {
            let na = avr.n_states();
            let input = super::models::AvrIn {
                vt,
                vs: vs0,
                xad_ifd: alg.xad_ifd,
                v_ref: vt,
            };
            let guess = avr_start(avr, input, vf0);
            let avr = avr.clone();
            let ua = newton(
                guess,
                |u, res| {
                    let vf = avr.eval(
                        &u[..na],
                        super::models::AvrIn {
                            v_ref: u[na],
                            ..input
                        },
                        &mut res[..na],
                    );
                    res[na] = vf - vf0;
                },
                &name,
            )?;
            x[a..g].copy_from_slice(&ua[..na]);
            dev.v_ref = ua[na];
        }
    }

    // Stage 2c: governor states and power reference for the torque.
    match &dev.gov {
        Governor::Fixed => dev.p_ref = tm0,
        // GAST sits on the kink of `min(P_ref, AT + KT (AT − x3))` when it runs
        // at its load limit, where the torque does not depend on `P_ref`. As in
        // PowerSimulationsDynamics.jl (init_tg.jl, GasTG), take the `P_ref` branch.
        Governor::Gast { .. } => {
            x[g..p].copy_from_slice(&[tm0, tm0, tm0]);
            dev.p_ref = tm0;
        }
        gov => {
            let ng = gov.n_states();
            let guess = gov_start(gov, tm0);
            let gov = gov.clone();
            let ug = newton(
                guess,
                |u, res| {
                    let tm = gov.eval(&u[..ng], 1.0, u[ng], &mut res[..ng]);
                    res[ng] = tm - tm0;
                },
                &name,
            )?;
            x[g..p].copy_from_slice(&ug[..ng]);
            dev.p_ref = ug[ng];
        }
    }

    // Stage 3: polish the whole device against its own residual.
    let mut f = vec![0.0; k];
    let out = dev.eval(&x, v.re, v.im, 1.0, &mut f);
    let worst = inf_norm(&f)
        .max((out.ir - i_sys.re).abs())
        .max((out.ii - i_sys.im).abs());
    if !(worst <= 1e-8) {
        return Err(format!(
            "{name}: device initialization left residual {worst:.3e}"
        ));
    }
    Ok(x)
}

/// Starting point of the stabilizer states at rest.
pub(crate) fn pss_start(pss: &Stabilizer, pe: f64, vt: f64, scale: f64) -> Vec<f64> {
    match pss {
        Stabilizer::Graph(g) => g.initial_guess(&[1.0, pe, vt, pe / scale], 0.0).0,
        p => vec![0.0; p.n_states()],
    }
}

/// Starting point of the exciter states followed by `V_ref`, for the field
/// voltage `vf` at the inputs `u` (its `v_ref` unused): a graph's from its
/// steady state (secant on `V_ref`), a compiled model's from its closed-form
/// guess.
pub(crate) fn avr_start(avr: &Avr, u: super::models::AvrIn, vf: f64) -> Vec<f64> {
    let (vt, vs, xad_ifd) = (u.vt, u.vs, u.xad_ifd);
    if let Avr::Graph(g) = avr {
        let (mut x, v_ref) = g.initial_guess(&[vt, vs, xad_ifd, vt], vf);
        x.push(v_ref);
        return x;
    }
    let mut guess = avr_guess(avr, vt, vf, xad_ifd);
    guess.push(vt + 0.01);
    guess
}

/// Starting point of the governor states followed by `P_ref`, for the
/// torque `tm` at `ω = 1`.
pub(crate) fn gov_start(gov: &Governor, tm: f64) -> Vec<f64> {
    if let Governor::Graph(g) = gov {
        let (mut x, p_ref) = g.initial_guess(&[1.0, tm], tm);
        x.push(p_ref);
        return x;
    }
    let mut guess = gov_guess(gov, tm);
    guess.push(tm);
    guess
}

pub(crate) fn avr_guess(avr: &Avr, vt: f64, vf: f64, xad_ifd: f64) -> Vec<f64> {
    match avr {
        Avr::Fixed | Avr::Graph(_) => vec![],
        Avr::Sexs { k, ta_tb, .. } => vec![vf, (1.0 - ta_tb) * vf / k],
        Avr::Esac1a {
            kf, tf, ke, kd, ka, ..
        } => {
            let ve = vf;
            let v_fe = kd * xad_ifd + ke * ve;
            vec![vt, v_fe / ka, v_fe, ve, -kf / tf * v_fe]
        }
        Avr::Exst1 {
            kf, tf, ka, tc, tb, ..
        } => {
            let t1_t2 = if *tb > 0.0 { tc / tb } else { 0.0 };
            // Without a feedback time constant the feedback is off.
            let xf = if *tf < f64::EPSILON {
                0.0
            } else {
                -kf / tf * vf
            };
            vec![vt, (1.0 - t1_t2) * vf / ka, vf, xf]
        }
    }
}

pub(crate) fn gov_guess(gov: &Governor, tm: f64) -> Vec<f64> {
    match gov {
        Governor::Fixed | Governor::Graph(_) => vec![],
        Governor::Tgov1 { t2, t3, .. } => vec![tm, (1.0 - t2 / t3) * tm],
        Governor::Gast { .. } => vec![tm, tm, tm],
        Governor::Hygov {
            at,
            q_nl,
            r_temp,
            tr,
            ..
        } => {
            let q = tm / at + q_nl;
            vec![0.0, q * r_temp * tr, q, q]
        }
    }
}
