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

Remaining #338 decisions: PowerIO's legacy generator column is still in
terminal-map order, neutral included, and uses the existing injection-positive
terminal powers. Coil powers in the MC report retain generation-positive
values and their own axes. The canonical constructor enforces terminal count;
changing that meaning requires an upstream compatibility design. PF and OPF
also still disagree on DELTA maps containing neutral/earth, and a domain
expert's review remains outstanding. These checks do not close #338.
