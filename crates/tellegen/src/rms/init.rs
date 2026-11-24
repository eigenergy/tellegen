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
//!
//! @block Build and initialize

use powerio::electrical::{calc_impedance_rebase, RebaseOrder};
use powerio::{BalancedNetwork, DynamicCase, MachineDynamics};

#[cfg(feature = "rms-experimental")]
use super::graph::{CompiledGraph, ModelOverride, Slot};
use super::models::{
    avr_from_psse, governor_from_psse, machine_from_psse, ri_dq, stabilizer_from_psse, Avr,
    Governor, Machine, Stabilizer,
};
use super::system::{
    fd_step, BusLoad, DynSystem, ExpPart, GenDevice, HeldUnit, LowVoltage, Source, C64,
};
use super::{LoadModel, LowVoltageLoad};
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
// @block Build the system from a case
pub(crate) fn build(
    network: &BalancedNetwork,
    dynamics: &DynamicCase,
    #[cfg(feature = "rms-experimental")] models: &[ModelOverride],
    load_model: LoadModel,
    options: super::RmsOptions,
) -> Result<(DynSystem, Vec<f64>, InitReport), String> {
    // @block Solve the power flow
    // The AC power flow the dynamics start from, with the low-voltage load
    // behaviour checked first; notes record voltage-dependent loads and
    // remote regulation, and a power flow that does not converge stops here.
    let mut report = InitReport::default();
    let low_voltage = match options.low_voltage_load {
        LowVoltageLoad::Break { v_break } => {
            // Experimental: a negative value is relative to each bus's power flow voltage
            if !(v_break.is_finite() && v_break != 0.0) {
                return Err(format!(
                    "the load breakpoint v_break must be a positive voltage, not {v_break}"
                ));
            }
            LowVoltage::Break(v_break)
        }
        LowVoltageLoad::Exact => LowVoltage::Floor,
    };
    let (ac, pf, zip_terms, pf_notes) = power_flow(network, options.reactive_limits)?;
    report.notes.extend(pf_notes);
    if zip_terms > 0 {
        report.notes.push(match load_model {
            LoadModel::PowerFlow => format!(
                "voltage-dependent (ZIP/exponential) loads: the power flow solved {zip_terms} voltage-dependent load part(s) exactly, and each keeps its law in the simulation"
            ),
            other => format!(
                "voltage-dependent (ZIP/exponential) loads: the power flow solved {zip_terms} voltage-dependent load part(s) exactly; the {other:?} load model replaces their law with its own from the power flow voltage on"
            ),
        });
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
    if super::not_lt(pf.residual, 1e-6) {
        return Err(format!(
            "the AC power flow did not converge (residual {:.3e})",
            pf.residual
        ));
    }
    // @block Network data
    // Bus numbering, bases, the solved bus voltages and the branch keys that
    // events and flows name.
    let n = ac.n;
    let sbase = network.base_mva();
    let f0 = network.base_frequency();
    let bus_numbers: Vec<usize> = ac.bus_ids.clone();
    let bus_index = super::system::bus_index(&bus_numbers);
    let dense = |number: usize| bus_index.get(&number).copied();
    let v: Vec<C64> = (0..n)
        .map(|b| C64::from_polar(pf.vm[b], pf.va[b]))
        .collect();

    // Branch keys for events and flows.
    let equipment = powerio::PsseEquipmentIndex::new(network);
    let circuits = equipment.circuit_ids();
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

    // @block Loads at the operating point
    // Each bus load at the power flow voltage, split into constant power,
    // current and impedance parts (or one law for the whole bus demand), so
    // the loads draw exactly the power flow demand at `t = 0`.

    // Loads at the power flow voltage.
    let (pd, qd) = ac.demand_at(&pf.vm);
    let mut loads = vec![BusLoad::default(); n];
    for b in 0..n {
        let (p, q) = (pd[b], qd[b]);
        let l = &mut loads[b];
        l.v0 = pf.vm[b];
        match load_model {
            LoadModel::PowerFlow => {
                l.p_p = ac.pd[b];
                l.q_p = ac.qd[b];
            }
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
    // The voltage-dependent parts with their own law: a term
    // `c (V / v_nom)^γ` is `c (V0 / v_nom)^γ (V / V0)^γ`, its power at the
    // power flow voltage `V0` times the law relative to `V0`.
    if load_model == LoadModel::PowerFlow {
        for t in &ac.load_terms {
            let l = &mut loads[t.bus];
            let (s0, _) = t.eval(l.v0);
            match (t.exponent, t.reactive) {
                (1.0, false) => l.p_i += s0,
                (1.0, true) => l.q_i += s0,
                (2.0, false) => l.p_z += s0,
                (2.0, true) => l.q_z += s0,
                (gamma, reactive) => l.exp.push(ExpPart {
                    reactive,
                    s0,
                    gamma,
                }),
            }
        }
    }

    // @block Link the dynamic data
    // Each DYR record is matched to its unit (bus and machine id). Records
    // without a typed view, out-of-service units and unsupported models are
    // counted and named in the notes.

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
    // Experimental: models read from their records as block graphs.
    #[cfg(feature = "rms-experimental")]
    let ext_model = |m: &str| super::ext_models::ext_role(m).is_some();
    #[cfg(not(feature = "rms-experimental"))]
    let ext_model = |_: &str| false;
    for (model, count) in untyped
        .iter()
        .filter(|(m, _)| !(options.renewable_plants && plant_model(m)))
        .filter(|(m, _)| !ext_model(m))
    {
        report.notes.push(format!(
            "dyr: {count} {model} record(s) have no model in tellegen and are not simulated"
        ));
    }
    // The units that name records without a typed view, keyed by bus and
    // machine id (ASCII case folded, as PowerIO falls back to matching it),
    // with those records' models: PowerIO reports a generator-attached
    // record of an unsupported model as `Untyped` with its unit.
    let mut unsupported_units: std::collections::BTreeMap<(usize, String), Vec<&str>> =
        std::collections::BTreeMap::new();
    for issue in &linking.issues {
        if let powerio::LinkIssue::Untyped {
            bus: Some(bus),
            machine_id: Some(id),
            error: powerio::ModelError::Unsupported { model_name },
            ..
        } = issue
        {
            unsupported_units
                .entry((bus.0, id.to_ascii_uppercase()))
                .or_default()
                .push(model_name.as_str());
        }
    }
    // @block Find renewable plants
    // Units with a complete REGCA1 + REECB1 + REPCA1 chain are simulated as
    // plants; other inverter records leave the unit at its power flow output.

    // Renewable plants: REGCA1 + REECB1 + REPCA1 of one unit, simulated
    // ([`super::renewable`]); a unit with other inverter records, or with a
    // record that does not parse, keeps its power flow output. A later record
    // of one model replaces an earlier one, as PSS/E reads a DYR file.
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
        let id = r.device_id().unwrap_or("1").trim().to_ascii_uppercase();
        plant_records.entry((bus as usize, id)).or_default()[slot] = Some(r);
    }
    let mut dynamic_plants = 0usize;
    let simulate_plants = options.renewable_plants;
    let mut static_units = 0usize;
    // @block Dispatch per unit
    // The power each unit carries at the operating point: a bus's generation
    // (net injection plus demand) shared among its units by their scheduled
    // output.
    let mut at_bus: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut entries: Vec<(&MachineDynamics, usize)> = Vec::new();
    for m in &linking.machines {
        let Some(b) = dense(m.bus.0) else {
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
        let p_bus = pf.p[b] + pd[b];
        let q_bus = pf.q[b] + qd[b];
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

    // @block Initialize each unit
    // Every linked unit becomes what its data allows: a simulated machine with
    // its controllers (solved for the states and references that make all
    // derivatives vanish at its power flow current), a renewable plant, a
    // Thevenin source, or a held constant-current injection.
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
        let unit = (m.bus.0, m.machine_id.to_ascii_uppercase());
        if let (None, Some(untyped_models)) = (&m.machine, unsupported_units.get(&unit)) {
            let plant = match plant_records.get(&unit) {
                Some([Some(gc), Some(ec), Some(pc)]) if simulate_plants => {
                    let others = m.exciter.is_some()
                        || m.governor.is_some()
                        || m.stabilizer.is_some()
                        || untyped_models
                            .iter()
                            .any(|model| !plant_model(model.trim().to_ascii_uppercase().as_str()));
                    let built = (|| -> Result<_, String> {
                        if others {
                            return Err("other inverter records of the unit have no model".into());
                        }
                        let mut regc = super::renewable::Regca1::from_tokens(gc.parameters())?;
                        regc.psid_lv_gain = options.psid_lv_gain;
                        regc.psid_ramp = options.psid_converter_ramp;
                        let mut p = super::renewable::Plant {
                            regc,
                            reec: super::renewable::Reecb1::from_tokens(ec.parameters(), m.bus.0)?,
                            repc: super::renewable::Repca1::from_tokens(pc.parameters(), m.bus.0)?,
                            scale,
                            r_source: 0.0,
                            x_source: 1e5,
                            p_ref: 0.0,
                            q_ref: 0.0,
                            v_ref: 1.0,
                            v_ref0: 1.0,
                            q_ref_converter: 0.0,
                            omega_ref: 1.0,
                            q_vexp: super::system::plant_qv_of(m.bus.0),
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
                    loads[b].u_p -= s.re;
                    loads[b].u_q -= s.im;
                    static_units += 1;
                    (None, 0)
                }
            };
            held.push(HeldUnit {
                bus_number: m.bus.0,
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
            // ZSORCE from the machine base to the system base: PowerIO's per
            // unit rebase at one voltage base.
            let rebase = |value: f64| {
                calc_impedance_rebase(
                    value,
                    1.0,
                    m.mbase,
                    1.0,
                    sbase,
                    RebaseOrder::InversePowerRatio,
                )
            };
            let z = C64::new(rebase(m.zsource.0), rebase(m.zsource.1));
            let e = v[b] + z * i_sys;
            sources.push(Source {
                bus_number: m.bus.0,
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
        let unit_err = |e: String| format!("generator {}-{}: {e}", m.bus, m.machine_id);
        let (mut machine, shaft) = machine_from_psse(&model, m.zsource).map_err(unit_err)?;
        let mut avr = m
            .exciter
            .as_ref()
            .map(|l| avr_from_psse(&l.model))
            .transpose()
            .map_err(unit_err)?
            .unwrap_or(Avr::Fixed);
        if options.psid_saturation {
            match &mut machine {
                Machine::RoundRotor(mm) => mm.sat = mm.sat.without_knee(),
                Machine::SalientPole(mm) => mm.sat = mm.sat.without_knee(),
                Machine::Classic { .. } => {}
            }
            if let Avr::Esac1a { sat, .. } = &mut avr {
                *sat = sat.without_knee();
            }
        }
        let gov = m
            .governor
            .as_ref()
            .map(|l| governor_from_psse(&l.model))
            .transpose()
            .map_err(unit_err)?
            .unwrap_or(Governor::Fixed);
        let pss = m
            .stabilizer
            .as_ref()
            .map(|l| stabilizer_from_psse(&l.model))
            .transpose()
            .map_err(unit_err)?
            .unwrap_or(Stabilizer::None);
        // Controller graphs given for this generator replace its records.
        #[cfg(feature = "rms-experimental")]
        let (avr, gov, pss) = {
            let graph = |slot: Slot| -> Result<Option<Box<CompiledGraph>>, String> {
                models
                    .iter()
                    .find(|o| {
                        o.bus == m.bus.0 && o.machine_id == m.machine_id && o.graph.slot == slot
                    })
                    .map(|o| {
                        CompiledGraph::new(&o.graph).map(Box::new).map_err(|e| {
                            format!("generator {}-{} {slot:?} graph: {e}", m.bus, m.machine_id)
                        })
                    })
                    .transpose()
            };
            // Experimental: ESST1A / ESAC6A / PSS2B / OELIT / UELPQ
            // records of the unit, as block graphs; a graph override still wins.
            let ext = super::ext_models::unit_records(dynamics, m.bus.0, &m.machine_id)
                .map_err(&unit_err)?;
            let (avr, pss) = if ext.is_empty() {
                (avr, pss)
            } else {
                let compile = |g: super::graph::BlockGraph| {
                    CompiledGraph::new(&g).map(Box::new).map_err(|e| {
                        format!("generator {}-{} {}: {e}", m.bus, m.machine_id, g.name)
                    })
                };
                let avr = match super::ext_models::exciter_graph(
                    &ext,
                    m.exciter.as_ref().map(|l| &l.model),
                )
                .map_err(&unit_err)?
                {
                    Some(g) => Avr::Graph(compile(g)?),
                    None => avr,
                };
                let pss = match super::ext_models::stabilizer_graph(&ext).map_err(&unit_err)? {
                    Some(g) => Stabilizer::Graph(compile(g)?),
                    None => pss,
                };
                (avr, pss)
            };
            let avr = graph(Slot::Exciter)?.map_or(avr, Avr::Graph);
            let gov = graph(Slot::Governor)?.map_or(gov, Governor::Graph);
            let pss = graph(Slot::Stabilizer)?.map_or(pss, Stabilizer::Graph);
            (avr, gov, pss)
        };
        let mut dev = GenDevice {
            bus_number: m.bus.0,
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
            vs_ext: 0.0,
            offset: nx,
            tripped: None,
            uel: super::system::uel_of(m.bus.0),
            vcomp: super::system::vcomp_of(m.bus.0),
        };
        nx += dev.n_states();
        let (x0, notes) = init_device(&mut dev, v[b], i_sys)?;
        for note in notes {
            report
                .notes
                .push(format!("generator {}: {note}", dev.name()));
        }
        device_x0.push(x0);
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
    // @block Reference frame
    // The network turns at the speed of a dynamic machine at the slack bus;
    // with none there, at the constant synchronous speed.

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

    // @block Assemble the system
    // The `DynSystem` from its parts, with the bus admittance and mass
    // matrices built once (events rebuild them).
    let m_branches = ac.m;
    let branches = ac.branches.clone();
    let mut sys = DynSystem {
        n,
        bus_numbers,
        bus_index,
        sbase,
        f0,
        ac,
        closed: vec![true; m_branches],
        branches,
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
        #[cfg(feature = "protection")]
        bypassed: vec![false; m_branches],
        branch_faults: vec![None; m_branches],
        held,
        low_voltage,
        factorize_by_blocks: options.factorize_by_blocks,
        refactor_values: options.refactor_values,
        keep_factorization: options.keep_factorization,
        threads: options.threads,
        noted_islands: Vec::new(),
    };
    sys.rebuild_ybus();
    sys.rebuild_mass();

    // @block The steady state
    // The initial state: bus voltages from the power flow, then each device's
    // and held unit's own initial states. The residual `f(x0)` must vanish;
    // a case that does not rest at `t = 0` is refused with its residual.
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

/// The AC power flow the dynamic initialization starts from: the static
/// study's ([`AcNetwork::from_network_pf`], [`ac_pf_with_start`]), whose
/// Newton iteration solves voltage-dependent (ZIP and exponential) loads
/// exactly, so the bus demand at the solution is [`AcNetwork::demand_at`] at
/// the solved magnitudes. With `reactive_limits` the generators' reactive
/// limits are enforced as in the static study (PV→PQ switching); without,
/// every generator holds its voltage setpoint whatever reactive power that
/// takes, as PowerSimulationsDynamics.jl initializes. Remote regulation is
/// the one difference to the static study, which refuses it: the
/// regulating terminal holds the case's voltage ([`hold_remote_regulators`]).
///
/// Returns the number of voltage-dependent load terms (0 when every load is
/// constant power) and notes naming every generator bus whose solution and
/// setpoint disagree: a bus released at a reactive limit (its voltage off the
/// setpoint), or, without limits, a bus whose reactive generation exceeds them.
// @block Operating point (AC power flow)
pub(crate) fn power_flow(
    network: &BalancedNetwork,
    reactive_limits: bool,
) -> Result<(AcNetwork, crate::problem::AcPfSolution, usize, Vec<String>), String> {
    // The static study holds in-service HVDC lines as fixed injections; the
    // dynamic network has no HVDC model, so such a case is refused by name.
    let hvdc = network.hvdc().iter().filter(|l| l.in_service).count();
    if hvdc > 0 {
        return Err(format!(
            "HVDC is not supported in RMS simulation: the case has {hvdc} in-service HVDC line(s) (take them out of service, or represent them as loads or generators)"
        ));
    }
    let mut ac = AcNetwork::from_network_pf(network)?;
    hold_remote_regulators(network, &mut ac);
    let (qmin, qmax) = (ac.qmin.clone(), ac.qmax.clone());
    if !reactive_limits {
        // A bus whose generators all have a fixed reactive power (Qmin = Qmax)
        // is no voltage-controlled bus: it keeps its limits and so its
        // reactive power; every other generator holds its setpoint.
        let mut fixed = vec![true; ac.bus_ids.len()];
        for g in 0..ac.k {
            if (qmax[g] - qmin[g]).abs() > 1e-9 {
                fixed[ac.gen_bus[g]] = false;
            }
        }
        for g in 0..ac.k {
            if !fixed[ac.gen_bus[g]] {
                ac.qmin[g] = f64::NEG_INFINITY;
                ac.qmax[g] = f64::INFINITY;
            }
        }
    }
    let pf = ac_pf_with_start(&AcPolar, &ac, AcStart::Case)?;
    let terms = ac.load_terms.len();
    // Per generator bus: the limits, and the reactive generation solved.
    let (_, qd) = ac.demand_at(&pf.vm);
    let mut lim: std::collections::BTreeMap<usize, (f64, f64)> = std::collections::BTreeMap::new();
    for g in 0..ac.k {
        let e = lim.entry(ac.gen_bus[g]).or_insert((0.0, 0.0));
        e.0 += qmin[g];
        e.1 += qmax[g];
    }
    let base = ac.base_mva;
    let mut notes = Vec::new();
    for (&b, &(lo, hi)) in &lim {
        if ac.slacks.contains(&b) {
            continue;
        }
        let qg = pf.q[b] + qd[b];
        let id = ac.bus_ids[b];
        let dv = pf.vm[b] - ac.vm_set[b];
        let released = ac.pf_pv[b] && pf.kinds[b] == crate::problem::BusKind::Pq;
        if reactive_limits && released {
            notes.push(format!(
                "generator bus {id} starts at its reactive limit (Q = {:.2} Mvar): V = {:.5} pu, {dv:+.5} pu from its setpoint {:.5} pu (the static AC power flow's PV→PQ switch; RmsOptions::reactive_limits)",
                qg * base,
                pf.vm[b],
                ac.vm_set[b]
            ));
        } else if !reactive_limits && ac.pf_pv[b] {
            let over = if qg > hi {
                qg - hi
            } else if qg < lo {
                qg - lo
            } else {
                0.0
            };
            if over.abs() > 1e-6 {
                notes.push(format!(
                    "generator bus {id} holds its setpoint {:.5} pu with Q = {:.2} Mvar, {:+.2} Mvar beyond its limits [{:.2}, {:.2}] Mvar (reactive limits not enforced)",
                    ac.vm_set[b],
                    qg * base,
                    over * base,
                    lo * base,
                    hi * base
                ));
            }
        }
    }
    Ok((ac, pf, terms, notes))
}

/// A generator that regulates a remote bus (PSS/E `IREG`) holds its own
/// terminal at the case's voltage there instead of its setpoint `VS`: in a
/// solved case that puts the regulated bus at `VS`, as the case states (and
/// as PowerSystems.jl initializes it). The static AC power flow still
/// refuses remote regulation; the dynamic initialization starts from the
/// case's operating point. Returns the number of such terminals.
pub(crate) fn hold_remote_regulators(net: &BalancedNetwork, ac: &mut AcNetwork) -> usize {
    let mut held = 0;
    // Built on the first remote regulator: most cases have none.
    let mut index = None;
    for g in net.generators().iter().filter(|g| g.in_service) {
        if g.regulated_bus.is_none_or(|r| r == g.bus) {
            continue;
        }
        let (dense, rows) = index.get_or_insert_with(|| {
            let ids: Vec<usize> = net.buses().iter().map(|b| b.id.0).collect();
            (
                super::system::bus_index(&ac.bus_ids),
                super::system::bus_index(&ids),
            )
        });
        let Some(&k) = dense.get(&g.bus.0) else {
            continue;
        };
        let Some(bus) = rows.get(&g.bus.0).map(|&row| &net.buses()[row]) else {
            continue;
        };
        if bus.vm > 0.0 && ac.vm_set[k] != bus.vm {
            ac.vm_set[k] = bus.vm;
            held += 1;
        }
    }
    held
}

/// The step minimizing `|J dz + g|² + λ|dz|²` (λ small relative to `J`).
pub(crate) fn least_squares_step(jac: &[f64], g: &[f64], n: usize) -> Option<Vec<f64>> {
    let scale = jac.iter().fold(0.0_f64, |a, v| a.max(v.abs())).max(1e-300);
    let lambda = 1e-10 * scale * scale;
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| jac[k * n + i] * jac[k * n + j]).sum::<f64>();
        }
        a[i * n + i] += lambda;
        b[i] = -(0..n).map(|k| jac[k * n + i] * g[k]).sum::<f64>();
    }
    solve_small(a, b, n)
}

/// Solve the dense `n × n` system `a x = b` (row-major) by Gaussian
/// elimination with partial pivoting; `None` when singular or not finite.
pub(crate) fn solve_small(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        if super::not_gt(a[p * n + k].abs(), 1e-300) {
            return None;
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            b.swap(k, p);
        }
        for i in k + 1..n {
            let l = a[i * n + k] / a[k * n + k];
            for j in k..n {
                a[i * n + j] -= l * a[k * n + j];
            }
            b[i] -= l * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s: f64 = (i + 1..n).map(|j| a[i * n + j] * x[j]).sum();
        x[i] = (b[i] - s) / a[i * n + i];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
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
// @block Small Newton solver
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
            let keep = u[c];
            let h = fd_step(keep);
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
            if let Some(step) = least_squares_step(&flat, &r, k) {
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
/// `p_ref` (and the classical `e'q`) and returns the local state vector with
/// notes on the limits its exciter rests beyond. The exciter and governor
/// steady states are solved with their limits removed and then checked
/// against them: one that leaves no steady state fails naming the limit and
/// the value the dispatch needs.
// @block Initialize one unit
pub(crate) fn init_device(
    dev: &mut GenDevice,
    v: C64,
    i_sys: C64,
) -> Result<(Vec<f64>, Vec<String>), String> {
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
    let mut notes = Vec::new();
    let mut check = |breaches: Vec<super::models::LimitBreach>| -> Result<(), String> {
        for b in breaches {
            if !b.holds {
                return Err(format!(
                    "{name}: {}, so the unit has no steady state at its power flow dispatch",
                    b.text
                ));
            }
            notes.push(format!(
                "{}: the unit starts, and rests, beyond this limit",
                b.text
            ));
        }
        Ok(())
    };

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
        x[p..p + xp.len()].copy_from_slice(&xp);
    }
    let mut scratch = vec![0.0; dev.pss.n_states()];
    let vs0 = dev.pss.eval(
        &x[p..p + dev.pss.n_states()],
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
            let s_m = v * i_m.conj();
            // the compensator's lag at rest: its state is the reactive current of the start
            let z0 = s_m.im / vt.max(1e-3);
            if dev.lag_states() > 0 {
                x[k - 1] = z0;
            }
            let input = super::models::AvrIn {
                vt: super::system::avr_vt(dev.vcomp, v.re, v.im, i_m.re, i_m.im, z0),
                vs: vs0,
                xad_ifd: alg.xad_ifd,
                v_ref: vt,
                p: s_m.re,
                q: s_m.im,
            };
            let guess = avr_start(avr, input, vf0);
            let limited = avr;
            let avr = avr.unlimited();
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
            check(limited.limit_breaches(
                &ua[..na],
                super::models::AvrIn {
                    v_ref: ua[na],
                    ..input
                },
            ))?;
            x[a..g].copy_from_slice(&ua[..na]);
            dev.v_ref = ua[na];
        }
    }
    dev.avr.rest_at(&x[a..g], 0.0);

    // Stage 2c: governor states and power reference for the torque.
    match &dev.gov {
        Governor::Fixed => dev.p_ref = tm0,
        // GAST sits on the kink of `min(P_ref, AT + KT (AT − x3))` when it runs
        // at its load limit, where the torque does not depend on `P_ref`. As in
        // PowerSimulationsDynamics.jl (init_tg.jl, GasTG), take the `P_ref` branch.
        gast @ Governor::Gast { .. } => {
            check(gast.limit_breaches(&[tm0; 3], tm0))?;
            x[g..p].copy_from_slice(&[tm0, tm0, tm0]);
            dev.p_ref = tm0;
        }
        gov => {
            let ng = gov.n_states();
            let guess = gov_start(gov, tm0);
            let limited = gov;
            let gov = gov.unlimited();
            let ug = newton(
                guess,
                |u, res| {
                    let tm = gov.eval(&u[..ng], 1.0, u[ng], &mut res[..ng]);
                    res[ng] = tm - tm0;
                },
                &name,
            )?;
            check(limited.limit_breaches(&ug[..ng], tm0))?;
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
    if super::not_le(worst, 1e-8) {
        return Err(format!(
            "{name}: device initialization left residual {worst:.3e}"
        ));
    }
    Ok((x, notes))
}

/// Starting point of the stabilizer states at rest.
#[cfg_attr(not(feature = "rms-experimental"), allow(unused_variables))]
pub(crate) fn pss_start(pss: &Stabilizer, pe: f64, vt: f64, scale: f64) -> Vec<f64> {
    match pss {
        #[cfg(feature = "rms-experimental")]
        Stabilizer::Graph(g) => g.initial_guess(&[1.0, pe, vt, pe / scale], 0.0).0,
        p => vec![0.0; p.n_states()],
    }
}

/// Starting point of the exciter states followed by `V_ref`, for the field
/// voltage `vf` at the inputs `u` (its `v_ref` unused): a graph's from its
/// steady state (secant on `V_ref`), a compiled model's from its closed-form
/// guess.
#[cfg_attr(not(feature = "rms-experimental"), allow(unused_variables))]
pub(crate) fn avr_start(avr: &Avr, u: super::models::AvrIn, vf: f64) -> Vec<f64> {
    let (vt, vs, xad_ifd) = (u.vt, u.vs, u.xad_ifd);
    #[cfg(feature = "rms-experimental")]
    if let Avr::Graph(g) = avr {
        let (mut x, v_ref) = g.initial_guess(&[vt, vs, xad_ifd, vt, u.p, u.q], vf);
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
    #[cfg(feature = "rms-experimental")]
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
        Avr::Fixed => vec![],
        #[cfg(feature = "rms-experimental")]
        Avr::Graph(_) => vec![],
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
        Governor::Fixed => vec![],
        #[cfg(feature = "rms-experimental")]
        Governor::Graph(_) => vec![],
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

#[cfg(test)]
mod tests {
    use powerio::LoadVoltageModel;

    use super::*;
    use crate::rms::{LoadModel, RmsRequest, RmsSimulation};

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/smib.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/wecc240.rs"
    ));

    /// A single-machine case from WECC 240 with a ZIP load at the unit's bus.
    fn smib_with_zip_load() -> (BalancedNetwork, powerio::DynamicCase) {
        let case = &wecc240_smib_cases()[0];
        let raw = with_load(
            &case.raw,
            "102,'1 ',1,   1,   1,    40.000,    10.000,    30.000,     5.000,    20.000,     4.000,   1,1",
        );
        let src = powerio::Source::from_memory("case.raw", raw.into_bytes()).unwrap();
        let module = powerio::parse(src).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        (net.clone(), powerio::parse_dyr(&case.dyr).unwrap())
    }

    /// The RMS initialization's power flow and the static AC power flow find
    /// the same operating point with a ZIP load, and at it the load draws its
    /// ZIP power at the solved voltage.
    #[test]
    fn a_zip_load_initializes_at_the_static_operating_point() {
        if shipped_wecc240().is_none() {
            return;
        }
        let (net, _) = smib_with_zip_load();
        assert!(matches!(
            net.loads()[0].voltage_model,
            Some(LoadVoltageModel::Zip { .. })
        ));
        let (ac, pf, terms, notes) = power_flow(&net, true).unwrap();
        assert!(terms > 0);
        // The static study's power flow, reactive limits and all: this unit's
        // limit binds with the load, and the initialization says so.
        let static_ac = AcNetwork::from_network_pf(&net).unwrap();
        let reference = ac_pf_with_start(&AcPolar, &static_ac, AcStart::Case).unwrap();
        assert!(
            notes
                .iter()
                .any(|n| n.contains("generator bus 102 starts at its reactive limit")),
            "{notes:?}"
        );
        for b in 0..ac.n {
            assert!(
                (pf.vm[b] - reference.vm[b]).abs() < 1e-8
                    && (pf.va[b] - reference.va[b]).abs() < 1e-8,
                "bus {}: {} {} vs {} {}",
                ac.bus_ids[b],
                pf.vm[b],
                pf.va[b],
                reference.vm[b],
                reference.va[b]
            );
        }
        let k = ac.bus_ids.iter().position(|&b| b == 102).unwrap();
        let v = pf.vm[k];
        let (pd, qd) = ac.demand_at(&pf.vm);
        let base = net.base_mva();
        let p = (40.0 + 30.0 * v + 20.0 * v * v) / base;
        // PSS/E states YQ positive for a capacitive load: PowerIO reads it as
        // a negative constant-impedance demand.
        let q = (10.0 + 5.0 * v - 4.0 * v * v) / base;
        assert!((pd[k] - p).abs() < 1e-10, "{} vs {p}", pd[k]);
        assert!((qd[k] - q).abs() < 1e-10, "{} vs {q}", qd[k]);
    }

    /// The RMS simulation starts at rest from that operating point.
    #[test]
    fn rms_starts_at_rest_with_a_zip_load() {
        if shipped_wecc240().is_none() {
            return;
        }
        let (net, dyr) = smib_with_zip_load();
        let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
        let r = sim
            .run(&RmsRequest {
                t_end: 1.0,
                ..RmsRequest::default()
            })
            .unwrap();
        assert!(
            r.notes.iter().any(|n| n.contains("voltage-dependent")),
            "{:?}",
            r.notes
        );
        let w = &r.channels["omega:102"];
        assert!(w.iter().all(|x| (x - 1.0).abs() < 1e-8), "not at rest");
    }

    /// The network-device interface frame: turning every bus voltage, every
    /// rotor angle and the infinite bus's internal voltage by `θ` turns the
    /// system onto itself. At a point off the steady state, every device row
    /// keeps its value and every bus's current balance turns by `θ`: the
    /// dq↔RI rotation (`Vd + jVq = (Vr + jVi) e^{−j(δ − π/2)}`) and the
    /// machine-base scaling enter the network as they should.
    #[test]
    fn the_device_frame_turns_with_the_network() {
        if shipped_wecc240().is_none() {
            return;
        }
        for case in wecc240_smib_cases()
            .iter()
            .filter(|c| c.id.starts_with("smib_genrou"))
        {
            let (net, dyr) = smib_study(case);
            let (mut sys, x0, _) = build(
                &net,
                &dyr,
                #[cfg(feature = "rms-experimental")]
                &[],
                LoadModel::PowerFlow,
                crate::rms::RmsOptions::default(),
            )
            .unwrap();
            let n = sys.n;
            let mut f = vec![0.0; sys.nx];
            // The start is a steady state to rounding.
            sys.residual(&x0, &mut f, None);
            let r = f.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
            assert!(r < 1e-8, "{}: {r:e}", case.id);
            // Off the steady state.
            let x: Vec<f64> = x0
                .iter()
                .enumerate()
                .map(|(i, v)| v + 1e-2 * (((i * 7) % 5) as f64 - 2.0))
                .collect();
            let theta = 0.7_f64;
            let turn = C64::from_polar(1.0, theta);
            let mut xr = x.clone();
            for b in 0..n {
                let v = C64::new(x[b], x[n + b]) * turn;
                xr[b] = v.re;
                xr[n + b] = v.im;
            }
            for d in &sys.devices {
                xr[d.offset + d.layout()[1]] += theta;
            }
            sys.residual(&x, &mut f, None);
            for s in &mut sys.sources {
                s.e *= turn;
            }
            let mut fr = vec![0.0; sys.nx];
            sys.residual(&xr, &mut fr, None);
            let mut worst = 0.0_f64;
            for b in 0..n {
                let i = C64::new(f[b], f[n + b]) * turn;
                worst = worst.max((i - C64::new(fr[b], fr[n + b])).norm());
            }
            for k in 2 * n..sys.nx {
                worst = worst.max((f[k] - fr[k]).abs());
            }
            // The bus rows reach 3e3 (the infinite bus's 1e5 pu admittance
            // times the perturbation): rounding there is about 1e-12.
            assert!(worst < 1e-9, "{}: {worst:e}", case.id);
        }
    }

    /// One GENROU unit with the controller `records` (DYR text after the
    /// bus, model and id), initialized at `V = 1 pu` delivering
    /// `0.8 + j0.3 pu`: its states and notes, or the error.
    fn init_genrou_with(records: &[&str]) -> Result<(Vec<f64>, Vec<String>), String> {
        let mut dyr = String::from(
            "102 'GENROU' 1 7.0 0.03 0.75 0.05 4.0 0.0 1.8 1.75 0.3 0.55 0.25 0.2 0.1 0.3 /\n",
        );
        for r in records {
            dyr.push_str(&format!("102 {r} /\n"));
        }
        let case = powerio::parse_dyr(&dyr).unwrap();
        let models: Vec<_> = case
            .records()
            .iter()
            .map(|r| powerio::typed_model(r).unwrap())
            .collect();
        let (machine, shaft) = machine_from_psse(&models[0], (0.0, 0.25)).unwrap();
        let (mut avr, mut gov) = (Avr::Fixed, Governor::Fixed);
        for m in &models[1..] {
            if let Ok(a) = avr_from_psse(m) {
                avr = a;
            } else {
                gov = governor_from_psse(m).unwrap();
            }
        }
        let mut dev = GenDevice {
            bus_number: 102,
            machine_id: "1".into(),
            bus: 0,
            scale: 1.0,
            f0: 60.0,
            machine,
            shaft,
            avr,
            gov,
            pss: Stabilizer::None,
            v_ref: 1.0,
            p_ref: 1.0,
            vs_ext: 0.0,
            offset: 0,
            tripped: None,
            uel: crate::rms::system::uel_of(102),
            vcomp: None,
        };
        let v = C64::new(1.0, 0.0);
        let i = (C64::new(0.8, 0.3) / v).conj();
        init_device(&mut dev, v, i)
    }

    /// An exciter or governor whose steady state at the dispatch stands
    /// beyond a limit is reported by the limit and the value: a note where
    /// the model rests there anyway (SEXS past EMAX, ESAC1A's VA past
    /// VAMAX), an error naming the limit where it cannot (before, a generic
    /// Newton failure, or for SEXS a field voltage above its ceiling with no
    /// word).
    #[test]
    fn controllers_beyond_their_limits_at_rest_are_named() {
        // Within every limit: no note.
        let (_, notes) = init_genrou_with(&[
            "'SEXS' 1 0.1 10 100 0.1 0 5",
            "'TGOV1' 1 0.05 0.5 1.0 0.0 2.0 6.0 0.0",
        ])
        .unwrap();
        assert!(notes.is_empty(), "{notes:?}");

        // SEXS rests above EMAX: a note naming it, and the field voltage the
        // dispatch needs (above the ceiling) is the one it starts with.
        let (x, notes) = init_genrou_with(&["'SEXS' 1 0.1 10 100 0.1 0 1.5"]).unwrap();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].contains("SEXS field voltage EFD") && notes[0].contains("above EMAX = 1.5"),
            "{notes:?}"
        );
        assert!(x[6] > 1.5, "Efd0 {}", x[6]);

        // ESAC1A's VA above VAMAX with VR inside its limits: a note.
        let esac1a = |va_max: f64, vr_max: f64| {
            format!(
                "'ESAC1A' 1 0 0 0 400 0.02 {va_max} -14.5 0.8 0.03 1 0.2 0.38 1 3.1 0.03 4.2 0.1 {vr_max} -5.43"
            )
        };
        let (_, notes) = init_genrou_with(&[&esac1a(1.5, 6.03)]).unwrap();
        assert!(
            notes.len() == 1 && notes[0].contains("ESAC1A regulator voltage VA"),
            "{notes:?}"
        );

        // With a zero time constant the limited lag is algebraic, so past
        // the limit there is no rest: the same breaches fail by name.
        let fails_named = |record: &str, what: &str, limit: &str| {
            let e = init_genrou_with(&[record]).unwrap_err();
            assert!(
                e.starts_with("generator 102-1: ") && e.contains(what) && e.contains(limit),
                "{e}"
            );
        };
        fails_named(
            "'SEXS' 1 0.1 10 100 0 0 1.5",
            "SEXS field voltage EFD",
            "above EMAX = 1.5",
        );
        fails_named(
            "'ESAC1A' 1 0 0 0 400 0 1.5 -14.5 0.8 0.03 1 0.2 0.38 1 3.1 0.03 4.2 0.1 6.03 -5.43",
            "ESAC1A regulator voltage VA",
            "above VAMAX = 1.5",
        );

        // No steady state: an error naming the unit, the quantity and the limit.
        let fails = |records: &[&str], what: &str, limit: &str| {
            let e = init_genrou_with(records).unwrap_err();
            assert!(
                e.starts_with("generator 102-1: ") && e.contains(what) && e.contains(limit),
                "{e}"
            );
        };
        fails(
            &[&esac1a(14.5, 1.5)],
            "ESAC1A regulator output VR",
            "above VRMAX = 1.5",
        );
        fails(
            &["'EXST1' 1 0 0.1 -0.1 1 10 200 0.02 1.5 -5 0.05 0 1"],
            "EXST1 field voltage EFD",
            "above VT·VRMAX − KC·Ifd",
        );
        fails(
            &["'EXST1' 1 0 0.005 -0.1 1 10 200 0.02 5 -5 0.05 0 1"],
            "EXST1 voltage error VI",
            "above VIMAX = 0.005",
        );
        fails(
            &["'TGOV1' 1 0.05 0.5 0.7 0.0 2.0 6.0 0.0"],
            "TGOV1 valve position = 0.8000",
            "above VMAX = 0.7",
        );
        fails(
            &["'GAST' 1 0.05 0.4 0.1 3.0 0.7 2.0 1.2 0.0 0.0"],
            "GAST turbine power = 0.8000",
            "above AT = 0.7",
        );
        fails(
            &["'GAST' 1 0.05 0.4 0.1 3.0 1.0 2.0 0.7 0.0 0.0"],
            "GAST valve position",
            "above VMAX = 0.7",
        );
        fails(
            &["'HYGOV' 1 0.05 0.3 5.0 0.05 0.5 0.2 0.6 0.0 1.2 1.2 0.0 0.08"],
            "HYGOV gate opening",
            "above GMAX = 0.6",
        );
    }

    /// An exciter graph whose lag rests past its limit at the dispatch gives
    /// the note a compiled exciter gives (SEXS's template past EMAX), and
    /// with a zero time constant fails by name.
    #[cfg(feature = "rms-experimental")]
    #[test]
    fn exciter_graphs_beyond_their_limits_at_rest_are_named() {
        let graph = |record: &str| -> Avr {
            let dyr = powerio::parse_dyr(&format!("102 {record} /\n")).unwrap();
            let m = powerio::typed_model(&dyr.records()[0]).unwrap();
            let g = super::super::graph::template(&m).unwrap();
            Avr::Graph(Box::new(
                super::super::graph::CompiledGraph::new(&g).unwrap(),
            ))
        };
        let with_graph = |record: &str| {
            let (_, native) = init_genrou_with(&[record])?;
            let mut dyr = String::from(
                "102 'GENROU' 1 7.0 0.03 0.75 0.05 4.0 0.0 1.8 1.75 0.3 0.55 0.25 0.2 0.1 0.3 /\n",
            );
            dyr.push_str(&format!("102 {record} /\n"));
            let case = powerio::parse_dyr(&dyr).unwrap();
            let m = powerio::typed_model(&case.records()[0]).unwrap();
            let (machine, shaft) = machine_from_psse(&m, (0.0, 0.25)).unwrap();
            let mut dev = GenDevice {
                bus_number: 102,
                machine_id: "1".into(),
                bus: 0,
                scale: 1.0,
                f0: 60.0,
                machine,
                shaft,
                avr: graph(record),
                gov: Governor::Fixed,
                pss: Stabilizer::None,
                v_ref: 1.0,
                p_ref: 1.0,
                vs_ext: 0.0,
                offset: 0,
                tripped: None,
                uel: crate::rms::system::uel_of(102),
            vcomp: None,
            };
            let v = C64::new(1.0, 0.0);
            let i = (C64::new(0.8, 0.3) / v).conj();
            init_device(&mut dev, v, i).map(|(_, notes)| (native, notes))
        };
        // Within its limits: no note from either.
        let (native, notes) = with_graph("'SEXS' 1 0.1 10 100 0.1 0 5").unwrap();
        assert!(
            native.is_empty() && notes.is_empty(),
            "{native:?} {notes:?}"
        );
        // Past EMAX: a note naming the lag and the limit, as the compiled one.
        let (native, notes) = with_graph("'SEXS' 1 0.1 10 100 0.1 0 1.5").unwrap();
        assert_eq!(native.len(), 1, "{native:?}");
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].contains("SEXS lag Vf") && notes[0].contains("above max = 1.5000"),
            "{notes:?}"
        );
        assert!(
            notes[0].contains("starts, and rests, beyond this limit"),
            "{notes:?}"
        );
        // With TE = 0 the lag is algebraic: no rest past the limit.
        let e = with_graph("'SEXS' 1 0.1 10 100 0 0 1.5").unwrap_err();
        assert!(e.starts_with("generator 102-1: "), "{e}");
    }

    /// The SMIB cases of WECC 240 whose SEXS starts above EMAX (the power
    /// flow without reactive limits), as compiled and as a block graph.
    #[cfg(feature = "rms-experimental")]
    fn smib_sexs_past_emax() -> Vec<(String, RmsSimulation)> {
        let options = crate::rms::RmsOptions {
            reactive_limits: false,
            ..crate::rms::RmsOptions::default()
        };
        let mut out = Vec::new();
        for case in wecc240_smib_cases()
            .iter()
            .filter(|c| c.models.iter().any(|m| m == "SEXS"))
        {
            let (net, dyr) = smib_study(case);
            let sim =
                RmsSimulation::with_options(&net, &dyr, LoadModel::ConstantImpedance, options)
                    .unwrap();
            let past = sim.notes.iter().any(|n| n.contains("above EMAX"));
            if !past {
                continue;
            }
            let sexs = dyr
                .records()
                .iter()
                .filter_map(|r| powerio::typed_model(r).ok())
                .find_map(|m| {
                    matches!(m, powerio::DynamicModel::Sexs(_))
                        .then(|| super::super::graph::template(&m))
                        .flatten()
                })
                .unwrap();
            let models = [crate::rms::ModelOverride {
                bus: 102,
                machine_id: "1".into(),
                graph: sexs,
            }];
            let graph = RmsSimulation::with_models_and_options(
                &net,
                &dyr,
                &models,
                LoadModel::ConstantImpedance,
                options,
            )
            .unwrap();
            assert!(
                graph.notes.iter().any(|n| n.contains("SEXS lag Vf")),
                "{}: {:?}",
                case.id,
                graph.notes
            );
            out.push((format!("{} (graph)", case.id), graph));
            out.push((case.id.clone(), sim));
        }
        out
    }

    /// A unit whose exciter starts past its output limit runs with the
    /// clamp at its state, the linearization alone opening it beyond: the
    /// trajectories with the small-signal analysis are those without it, to
    /// 1e-12 in every channel, through a fault and a line trip. (A run with
    /// the opened clamp differs by about 3e-11: its Newton iterations use
    /// the Jacobian the clamp changes.)
    #[cfg(feature = "rms-experimental")]
    #[test]
    fn the_rest_margin_leaves_the_trajectories() {
        use crate::rms::RmsEvent;
        if shipped_wecc240().is_none() {
            return;
        }
        let cases = smib_sexs_past_emax();
        assert!(!cases.is_empty());
        for (id, sim) in &cases {
            for events in [
                vec![
                    RmsEvent::BusFault {
                        time: 0.2,
                        bus: 102,
                        r: 0.0,
                        x: 0.01,
                    },
                    RmsEvent::ClearFault {
                        time: 0.3,
                        bus: 102,
                    },
                ],
                vec![RmsEvent::BranchTrip {
                    time: 0.2,
                    from: 101,
                    to: 102,
                    circuit: "2".into(),
                }],
            ] {
                let request = |small_signal| RmsRequest {
                    t_end: 2.0,
                    dt: 0.002,
                    output_step: 0.01,
                    events: events.clone(),
                    small_signal,
                    ..RmsRequest::default()
                };
                let (a, b) = (
                    sim.run(&request(true)).unwrap(),
                    sim.run(&request(false)).unwrap(),
                );
                assert!(!a.eigenvalues.is_empty() && b.eigenvalues.is_empty());
                assert_eq!(a.channels.len(), b.channels.len());
                for (key, ya) in &a.channels {
                    let d = ya
                        .iter()
                        .zip(&b.channels[key])
                        .map(|(p, q)| (p - q).abs())
                        .fold(0.0_f64, f64::max);
                    assert!(d <= 1e-12, "{id} {key}: {d:e}");
                }
            }
        }
    }

    /// The linearization at a field voltage resting above EMAX keeps its
    /// coupling: none of the eigenvalues is the exciter's lag alone
    /// (`−1/TE = −10`), where the clamp exactly at the state left one, and
    /// they are those with the clamp far beyond the state (the difference
    /// quotients stay inside the margin).
    #[cfg(feature = "rms-experimental")]
    #[test]
    fn a_unit_resting_past_its_limit_keeps_its_small_signal_coupling() {
        if shipped_wecc240().is_none() {
            return;
        }
        let cases = smib_sexs_past_emax();
        assert!(cases.iter().any(|(id, _)| id == "smib_genrou_hygov_sexs"));
        let lone = |e: &[[f64; 2]]| e.iter().any(|z| (z[0] + 10.0).abs() < 1e-6 && z[1] == 0.0);
        for (id, sim) in &cases {
            let eig = sim.eigenvalues().unwrap();
            assert!(!lone(&eig), "{id}: {eig:?}");
            let tight = sim.eigenvalues_at(0.0).unwrap();
            assert!(lone(&tight), "{id}: {tight:?}");
            assert_eq!(eig, sim.eigenvalues_at(1.0).unwrap(), "{id}");
        }
    }
}
