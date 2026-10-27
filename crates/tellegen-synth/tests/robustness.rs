//! A wider sweep than the seed matrix: four regions × ten seeds × three
//! densities, plus low fidelity on the first three seeds (156 runs), twice.
//!
//! - At stated 500/230 kV levels, where the corridor plan may climb to
//!   765 kV, every run must give a converged corridor plan, an optimal DC
//!   OPF and an AC power flow inside the emergency band.
//! - At each region's national ladder (the default), where a country with no
//!   level above its ladder keeps its ceiling and may end the plan
//!   `planning_incomplete`, every run must still give an optimal DC OPF. The
//!   runs whose AC power flow fails or leaves the band, and the incomplete
//!   plans, are counted and printed: at Ghana's 330/161 kV some low-density
//!   grids leave the band, which the evidence reports.
//!
//! Ignored by default because each takes a few seconds in release mode:
//!
//! ```sh
//! cargo test -p tellegen-synth --release --test robustness -- --ignored
//! ```

mod common;

use tellegen_synth::{generate, AcPfOutcome, Density, Fidelity, SolveOutcome};

/// What one sweep found.
struct Sweep {
    /// Runs that broke a requirement, described.
    fails: Vec<String>,
    /// Runs whose AC power flow failed or left the emergency band.
    ac_outside: Vec<String>,
    /// Runs whose corridor plan ended incomplete.
    incomplete: usize,
    total: usize,
}

/// The sweep's runs, each with `levels` stated (or the national ladders when
/// empty). Every run needs an optimal DC OPF; `strict` also needs a
/// converged plan and an AC power flow inside the band.
fn sweep(levels: &[f64], strict: bool) -> Sweep {
    let regions = vec![
        ("iraq", common::iraq_box()),
        ("rhine", common::rhine_box()),
        ("guinea", common::gulf_of_guinea_box()),
        ("islands", common::two_landmasses()),
    ];
    let mut fails = Vec::new();
    let mut ac_outside = Vec::new();
    let mut incomplete = 0;
    let mut total = 0;
    let mut worst_spread: f64 = 0.0;
    let mut ratio_max: f64 = 0.0;
    for (name, region) in &regions {
        for seed in 1u64..=10 {
            for density in [Density::Low, Density::Medium, Density::High] {
                for fidelity in [Fidelity::High, Fidelity::Low] {
                    if fidelity == Fidelity::Low && seed > 3 {
                        continue;
                    }
                    total += 1;
                    let mut req = common::request(region.clone(), seed, None);
                    req.density = density;
                    req.fidelity = fidelity;
                    req.voltage_levels_kv = levels.to_vec();
                    let label = format!("{name} {seed} {density:?} {fidelity:?}");
                    let (_, r) = match generate(req, common::places(), None) {
                        Ok(x) => x,
                        Err(e) => {
                            fails.push(format!("{label}: error {e}"));
                            continue;
                        }
                    };
                    let plan = r.planning.as_ref().unwrap();
                    worst_spread = worst_spread.max(plan.angle_spread_deg);
                    ratio_max = ratio_max
                        .max(plan.branches as f64 / r.stats.as_ref().unwrap().n_buses as f64);
                    let dc = r.dc_opf.as_ref().unwrap();
                    let ac = r.ac_pf.as_ref().unwrap();
                    let bad_dc = dc.status != SolveOutcome::Optimal;
                    let bad_ac = ac.status != AcPfOutcome::Converged
                        || ac.vm_min.unwrap_or(0.0) < 0.9
                        || ac.vm_max.unwrap_or(9.0) > 1.1;
                    incomplete += usize::from(!plan.converged);
                    let line = format!("{label}: dc {:?} ac {:?} vm [{:?},{:?}] plan conv {} spread {:.1} n {} br {}",
                        dc.status, ac.status, ac.vm_min, ac.vm_max, plan.converged, plan.angle_spread_deg, r.stats.as_ref().unwrap().n_buses, plan.branches);
                    if bad_ac {
                        ac_outside.push(line.clone());
                    }
                    if bad_dc || (strict && (bad_ac || !plan.converged)) {
                        fails.push(line);
                    }
                }
            }
        }
    }
    println!(
        "{total} runs, {} failures, {} AC outside the band, {incomplete} incomplete plans, widest planned spread {worst_spread:.1}°, highest branch/bus ratio {ratio_max:.2}",
        fails.len(),
        ac_outside.len()
    );
    Sweep {
        fails,
        ac_outside,
        incomplete,
        total,
    }
}

#[test]
#[ignore = "a few seconds in release mode; run with --ignored"]
fn wide_sweep_plans_solve_and_converge() {
    let found = sweep(&[500.0, 230.0], true);
    assert_eq!(found.total, 156);
    assert!(found.fails.is_empty(), "{}", found.fails.join("\n"));
}

#[test]
#[ignore = "a few seconds in release mode; run with --ignored"]
fn wide_sweep_at_national_ladders_stays_dc_feasible() {
    let found = sweep(&[], false);
    assert_eq!(found.total, 156);
    for line in &found.ac_outside {
        println!("AC outside the band: {line}");
    }
    println!("{} incomplete plans", found.incomplete);
    assert!(found.fails.is_empty(), "{}", found.fails.join("\n"));
}
