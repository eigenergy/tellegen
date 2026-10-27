//! The seed matrix: regions × seeds × densities must give an optimal DC OPF
//! with no branch above its rating after calibration, a converged AC power
//! flow inside the emergency band, and a calibration record whose
//! bookkeeping is consistent.

mod common;

use tellegen_synth::{generate, AcPfOutcome, CalibrationStatus, Density, SolveOutcome};

fn regions() -> Vec<(&'static str, tellegen_synth::Region)> {
    vec![
        ("iraq", common::iraq_box()),
        ("rhine", common::rhine_box()),
        ("guinea", common::gulf_of_guinea_box()),
        ("islands", common::two_landmasses()),
    ]
}

#[test]
fn seed_matrix_is_solvable_and_consistent() {
    let mut failures = Vec::new();
    for (name, region) in regions() {
        for seed in [1u64, 2, 3] {
            for density in [Density::Low, Density::High] {
                let mut req = common::request(region.clone(), seed, None);
                req.density = density;
                let label = format!("{name} seed={seed} {density:?}");
                let (_, report) = match generate(req, common::places(), None) {
                    Ok(x) => x,
                    Err(e) => {
                        failures.push(format!("{label}: error {e}"));
                        continue;
                    }
                };
                let dc = report.dc_opf.as_ref().unwrap();
                if dc.status != SolveOutcome::Optimal {
                    failures.push(format!("{label}: DC OPF {:?} {:?}", dc.status, dc.message));
                    continue;
                }
                if dc.max_loading.unwrap() > 1.0 + 1e-6 {
                    failures.push(format!("{label}: max loading {}", dc.max_loading.unwrap()));
                }
                let cal = report.calibration.as_ref().unwrap();
                if cal.final_objective > cal.initial_objective + 1e-12 {
                    failures.push(format!("{label}: calibration objective rose"));
                }
                if (cal.final_objective - cal.initial_objective).abs() <= 1e-12
                    && !matches!(
                        cal.status,
                        CalibrationStatus::Unchanged | CalibrationStatus::Converged
                    )
                {
                    failures.push(format!(
                        "{label}: status {:?} with equal objectives",
                        cal.status
                    ));
                }
                for step in &cal.steps {
                    if step.accepted != (step.objective_after < step.objective_before - 1e-9) {
                        failures.push(format!(
                            "{label}: step {} accepted flag inconsistent",
                            step.iteration
                        ));
                    }
                }
                let ac = report.ac_pf.as_ref().unwrap();
                if ac.status != AcPfOutcome::Converged {
                    failures.push(format!("{label}: AC PF {:?} {:?}", ac.status, ac.message));
                    continue;
                }
                let (vmin, vmax) = (ac.vm_min.unwrap(), ac.vm_max.unwrap());
                if !(0.9..=1.1).contains(&vmin) || !(0.9..=1.1).contains(&vmax) {
                    failures.push(format!("{label}: vm range [{vmin:.3}, {vmax:.3}]"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
