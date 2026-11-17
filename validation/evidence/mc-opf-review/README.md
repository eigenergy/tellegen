# MC OPF reporting and independent checks

The canonical solution now reports maximum terminal active/reactive power
mismatch in MW/MVAr, matching PowerIO's `Residuals` field documentation.
These values come from independently accumulated KCL currents and terminal
voltages. They do not replace the stricter current, voltage, device-power,
limit, and objective checks in the MC report.

POUNCE's acceptable-level stop retains `Termination::Converged` for API
compatibility and now adds `MC_OPF.SOLVED_TO_ACCEPTABLE_LEVEL`. Both success
levels still require the same independent physical validation.

`check_opendss.py` copies the existing MIT-licensed synthetic `unbalanced.json`
fixture, removes its asymmetric line shunts, and checks four cases: WYE or
DELTA generation, with a grounded or floating receiving neutral. Both WYE and
DELTA constant-power loads remain. Each optimized coil dispatch becomes a
fixed negative constant-power load in a fresh OpenDSS model. The script emits
the complete modified inputs, DSS commands, engine version, and measured
voltage/current differences. It does not use PowerIO's DSS reader or writer.
The source uses 1e-8 ohm sequence impedance to approximate the ideal reference.

Run with OpenDSSDirect.py 0.9.4:

```sh
cargo build -p tellegen --features mc-opf --example mc_opf_case_probe
python check_opendss.py /absolute/path/to/target/debug/examples/mc_opf_case_probe > opendss-results.json
```

The recorded four cases agree within 7.6e-8 V and 1.2e-10 A; the script rejects
errors above 2e-5 V or A. This qualifies independent feasibility on these
small circuits. It does not establish global optimality, every equipment
model, or a domain expert's formulation review.

Existing `mc_opf::rank_tests::compact_rank_reports_keep_legacy_verification_and_tamper_checks_in_ci`
checks versions 6, 7, 8 and 9: old rank claims are verified under their original
rules before upgrade, and modified claims are rejected. The unreleased jump
therefore has a tested read path; report version is not the package version.

## Formulation review

The review checked the expression compiler, separate physical validation and
the regression cases below. It found no additional defect in these equations.
The OpenDSS comparisons above independently check feasibility; they do not
certify optimality or every supported equipment combination.

| Question | Reviewed behavior and evidence |
| --- | --- |
| Coil and terminal incidence | `mc_opf_results::assert_ledgers` reconstructs terminal currents from each coil's positive and negative terminals, including the return current. It checks terminal and coil powers with complex arithmetic and accumulates KCL independently. The results round-trip test covers the saved axes; permutation tests retain their equipment identities. |
| KCL, KVL and power signs | `equation_rows_match_independent_complex_arithmetic_away_from_solution` checks line voltage drops and coil powers at a perturbed point, where an error cannot hide behind a solved residual of zero. The separate validator adds generator injections, subtracts loads and passive-device withdrawals, and evaluates the declared objective from coil powers. The resistive two-wire test checks a closed-form solution and a nonzero neutral voltage. |
| Zero ratings | Tests cover current and apparent-power limits, required versus zero flow, both line-end shunts, source-angle changes, open switches and zero inverter ratings. `zero_apparent_rating_at_ground_does_not_imply_zero_current` specifically retains return current at a zero-voltage terminal. |
| Signed fixed power factor | `original_pf_validation_detects_wrong_control_and_qualifies_tiny_dispatch` checks both signs at ordinary and near-zero power factor, rejects a wrong control equation, and reports unqualified relative accuracy at tiny or zero dispatch. `tiny_pf_solve_reports_relative_accuracy_and_preserves_it_on_import` checks a solved example and refuses a saved report with its warning removed. Absolute power tolerance is not presented as relative power-factor accuracy. |
| Result acceptance | Both successful solver stopping levels pass the same independent current, voltage, power, limit and objective checks. A local nonlinear-solver failure is not reported as proof of infeasibility. Canonical power residuals now use MW/MVAr; the report retains its separate dimensionless residual families. |

For the patch release, PowerIO's existing generator column remains in
terminal-map order, neutral included, with injection-positive terminal powers.
Use `devices[].coil_power_va` together with `devices[].coils` for generation
costs, coil bounds and coil dispatch. DELTA coil powers and terminal powers
are different quantities; multiplying costs by the legacy terminal column is
incorrect. Changing that column's meaning requires a separately reviewed
upstream compatibility change.

The OPF preparation contract rejects DELTA maps containing neutral or earth
terminals before optimization, including direct instances and edited sessions.
Power flow supports a broader set of two-terminal connections. This patch
preserves both contracts and their explicit refusal behavior instead of silently
rewriting a device's terminal roles. `mc_opf_terminal_roles` tests the OPF
boundary and atomic rollback of an invalid edit. PowerIO #742 has merged;
the remaining distinction is intentional scope in the current calculations,
not an unmerged prerequisite. PowerIO #700 is a broader terminal-power sign
discussion and does not by itself approve changing MC generator fields.
