# Validation

tellegen is checked against the published PGLib reference solves. For each case and
variant, `$PGLIB_OPF_PATH/BASELINE.md` tabulates the PowerModels.jl with IPOPT
reference: `DC ($/h)`, `AC ($/h)`, `QC Gap (%)`, and `SOC Gap (%)`. The
[Methodology](methodology.md) chapter describes how the harness produces the
comparison; this chapter records what each comparison asserts. The measured figures are in
[benchmark results](benchmark-results.md), a committed snapshot of a full
harness run with its provenance.

## DC objective

tellegen's declared DC objective, including its constant cost term, is compared
against the published DC baseline. Both values come from the same PGLib cost
data. Per-unit power scaling cancels; tellegen does not assign a currency to
objectives from other sources.

## Relaxation lower bound

The SOCWR objective (`socwr_opf` objective) must be a lower bound on the published
AC optimum (`AC ($/h)`):

$$ \text{socwr} \le \text{AC} + \text{tol}. $$

A bound violation is a correctness failure.

## SOC gap

The gap is

$$ \text{gap} = \frac{\text{AC} - \text{socwr}}{\text{AC}} \cdot 100, $$

compared against the baseline `SOC Gap (%)`. tellegen's SOCWR is the Jabr SOC
relaxation, the same family as the baseline `SOC` column, so a near-zero
difference in gap is the expected result. The published SOC bound is recovered as
$\text{AC} \cdot (1 - \text{SOC gap}/100)$.

All three variants are exercised: typical, congested (API), and small-angle (SAD).
tellegen's `AcNetwork` carries the angle difference limits and the SOCWR enforces
them in W-space, so the SAD relaxation tracks the published SAD SOC. Where a case
shows a large SOC gap, that measures relaxation quality, not a failure.

## Multiconductor IVR OPF

The experimental native [IVR formulation](formulations.md#experimental-native-multiconductor-ivr-opf)
requires both a successful solver status and an independent physical acceptance
check. Tellegen recomputes KCL, KVL, device power links, load/control laws,
selected limits and the objective from the solution and prepared electrical data.
The default maximum normalized residual tolerance is `1e-6` in the model's
working units. Nonfinite values fail acceptance. Local numerical success does
not establish global optimality.

This check is independent of the solver expression graph but shares PowerIO's
preparation. It cannot detect a physical constraint discarded before preparation.
The module-JSON entry point and external-case probe therefore use the same
`validate_mc_opf_input_diagnostics` guard: error diagnostics and codes containing
`UNSUPPORTED`, `MALFORMED`, `FIELD_DROPPED` or `RECORD_DROPPED` reject the input.
Lossless representation-change remarks, such as capacitor-to-shunt lowering,
are permitted. External comparisons also inspect original input bounds, including
scalar inverter availability and zero available active power.

The experimental MC solve APIs return `McOpfError`, which implements
`std::error::Error` and serializes to JSON. Callers match `kind` and `stage`
rather than parsing its display message. Kinds distinguish invalid input,
unsupported input, cancellation, numerical failure, physical-validation failure
and serialization failure. Upstream findings retain their codes, severities,
messages and targets. Numerical failures include the solver status when one is
available; they are not infeasibility certificates.

Physical-validation errors include the finite residual summary and the worst
failing witness in each residual family, with component and axis identities.
A witness whose residual cannot be represented as a finite number uses `null`.
No failure contains an accepted solution. The external-case probe uses the same
structured error object in its `error` field.

Regression coverage combines:

- Hand-computed off-solution KCL/KVL/device-law witnesses, mutual coupling,
  unequal end shunts, and zero/singular branch impedances.
- Quadratic-row classification and forced sparse-AD comparisons, directional
  Jacobian/Hessian checks at multiple points with nonzero multipliers and
  objective factors, and control derivatives at knots and softplus extremes.
- Base/order invariance, cost-driven dispatch, constraint selection, cancellation,
  and deliberately corrupted results including NaN and infinity.
- Feasible and infeasible zero current/apparent-power ratings, partial conductor
  limits, rotated source phasors and both-end shunts. A grounded terminal may
  carry nonzero current under a zero apparent-power limit; fixed nonzero voltage
  makes that same rating require zero current.
- Frozen BMOPFTools component and unbalanced-network solves, comparing objectives
  and complex physical voltages, currents, powers and taps with explicit
  per-fixture tolerances.
- Complete result export/import across delta devices, neutral returns,
  transformer rating ports, mixed windings, shunts and optimized taps. Tests
  reconstruct KCL and complex power from public result identities, preserve
  diagnostic/provenance records, and reject incomplete or mismatched reports.

The inputs, reference JSON, attribution and license files in
`crates/tellegen/tests/data/mc_opf/` are regression assets. Their provenance
records input SHA-256 hashes, oracle revision, units and numerical tolerances;
the component bundles also record Julia/JuMP/Ipopt versions and solver options.
Ordinary Rust tests consume the frozen results without Julia, network access
or an external feeder dataset. Intentional regeneration uses:

```sh
julia --project=/path/to/BMOPFTools.jl/test scripts/mc_opf_oracle.jl
```

The script requires a clean BMOPFTools checkout at
`a8b52e069bfd4a7a57434c91bc0470ac03cfdd55`. Review changed inputs, hashes and
reference values together; do not relax tolerances to hide a changed electrical
problem. The [external study protocol](methodology.md#external-multiconductor-ivr-studies)
adds larger-feeder comparisons without vendoring the dataset.
