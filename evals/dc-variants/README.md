# DC variants: independent PowerModels and AC validation

This nonshipping harness addresses Tellegen #206. All source code and the
synthetic `case3_variants.m` are MIT licensed, written for this change. The
small reference JSON was generated from that case; no external case is
vendored here. `Manifest.toml` pins Julia dependencies (PowerModels 0.19.9,
Ipopt.jl 1.4.1; Ipopt_jll 300.1400.1300+0, i.e. Ipopt 3.14.13). The recorded
runs used Julia 1.12.6. PGLib's published baseline used the same PowerModels
release but an older Ipopt binary; numerical compatibility is tolerance-based.

## Reproduce

Instantiate the pinned environment once:

```sh
julia --project=evals/dc-variants -e 'using Pkg; Pkg.instantiate()'
bash evals/dc-variants/run.sh crates/tellegen/tests/data/dc_variants/case3_variants.m /tmp/dc-case3
python3 evals/dc-variants/compare.py crates/tellegen/tests/data/dc_variants/case3_variants.m \
  /tmp/dc-case3/tellegen.json /tmp/dc-case3/reference.json \
  /tmp/dc-case3/report.json --check --golden /tmp/dc-case3/reference-small.json
```

`--check` is for the strictly convex, nondegenerate small fixture. The committed
`crates/tellegen/tests/data/dc_variants/case3-reference.json` powers offline Rust
regression tests and ships with the crate alongside the synthetic case. `reference.jl`
uses unmodified **built-in DCP and DCMP** for those two conventions. Its three
`ReferenceDC` types are validation-only custom models, with independent branch
equations; they are not built-in PowerModels formulations.

Use PGLib v23.07's `pglib_opf_case300_ieee.m` and
`pglib_opf_case13659_pegase.m` as external inputs to the same runner. Obtain
them from https://github.com/power-grid-lib/pglib-opf/tree/v23.07 (MIT).
They are about 154 KB and 7.5 MB respectively and remain outside this repository.
Each report records its exact SHA-256. Full solver output goes to the requested
output directory; the repository carries compact reports only.

## Contract

Use per-unit data, all active equipment, case costs including constants,
generator limits and shunts, case thermal active-power limits, one stated
reference per connected component, and finite angle bounds clamped to ±60°.
The fixtures used here have explicit positive ratings: the comparison does not
silently equate Tellegen's synthesized unrated limits with unlimited branches.
PowerModels parses and normalizes the MATPOWER source independently of PowerIO.
Its opposite-parallel-branch reorientation is accounted for before comparing
flows (25 branches on case13659, all with unity taps). The comparator rejects
reversed nonunity transformers, whose impedance transformation would require
a further source-coordinate audit for tap-ignoring models.

Check equation and balance residuals before comparing objective. Small-case
limits are 1e-8 pu residual/PF flow, 1e-5 pu OPF flow/dispatch, 1e-6 relative
objective, and 1e-4 currency/MWh price. PowerModels balance duals are negated
and divided by baseMVA to match Tellegen's price convention. Large-case dispatch
and price errors are descriptive because uniqueness/active-set regularity is
not asserted there. Solver failures are retained, not discarded.

The independent complex pi-model finite difference uses unit voltages and
`theta_from - theta_to = shift`. It verifies the hybrid angle slope, including
both tap magnitude and phase. It is not an assertion of universal AC accuracy.

AC PF fixes each nonreference generator's active dispatch and PV/reference
voltage setpoint, leaves reactive limits unenforced, and lets reference
injection absorb real losses. Report voltage and reactive-generation violations,
branch apparent power at both ends, signed endpoint active-power errors,
symmetric transfer `(Pfrom-Pto)/2`, losses, absolute MW errors and rating-normalized
errors; avoid percentages of near-zero flow. DC OPF dispatches are each replayed
through AC PF under the same policy. For cases up to 300 buses the report also
includes an ACP/Ipopt **local** AC OPF result, not a global bound.

The tests do not require the hybrid to improve AC accuracy. The small case is
an intentional example: every DC dispatch violates an AC apparent-power limit.
See the user-facing [compatibility matrix](../../docs/src/dc-compatibility.md)
for the relationship between selectable conventions and optimal objectives.

## Recorded results

All five conventions pass the small fixture's numerical gates. On PGLib
case300, the largest relative objective difference across the five conventions
is 2.40e-11 and the largest nodal-balance residual is 5.02e-10 pu.
These are numerical comparisons against PowerModels, not published PGLib AC costs.

Case13659 is a stress test, **not a passed optimality qualification**:

| Convention | Relative objective difference | Max nodal balance residual (pu) |
| --- | ---: | ---: |
| Legacy series susceptance | 5.40e-5 | 2.18e-3 |
| PowerModels DCP | 7.59e-5 | 2.27e-3 |
| MATPOWER / PowerModels DCMP | 1.21e-10 | 5.82e-7 |
| Tap-adjusted series susceptance | 2.64e-5 | 1.35e-3 |
| Reactance only | 3.07e-5 | 1.78e-4 |

The DC PF flows agree within 1.11e-8 pu on case13659. Thus the OPF objective
gaps alone are not evidence of different branch equations. The existing
Clarabel driver accepts `AlmostSolved` as well as `Solved` and the API reports
both as `optimal`. Temporary solver-status instrumentation confirmed reduced
accuracy on this stress case; the shipping solver settings and status mapping
are unchanged by this PR. A lower-regularization experiment did not uniformly
improve the five variants, so no unqualified numerical tuning was included.
The raw primal residuals above take precedence over the coarse API status.
Solver status fidelity and large-case numerical robustness need separate
qualification before claiming general PGLib optimality parity.

For AC validation, the synthetic case's PF and all five dispatch replays
converge; every DC dispatch violates one AC apparent-power limit. Its local
AC OPF converges without measured violations, with objective 736.74955.
The case300 local AC OPF also converges without measured violations
(objective 565219.99089). Its fixed-setpoint PF and DC-dispatch replays return
`LOCALLY_INFEASIBLE`; case13659's fixed-setpoint PF reaches the 300-second
CPU limit and its dispatch replays return `LOCALLY_INFEASIBLE`. These local
solver outcomes do not prove physical infeasibility. Failed iterates are
retained in raw outputs but excluded from AC accuracy/violation metrics.
No case13659 AC OPF was attempted.
