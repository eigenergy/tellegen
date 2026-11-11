# DC variants: independent PowerModels and AC validation

This nonshipping harness addresses Tellegen #206. All source code and the
synthetic three-bus cases are MIT licensed, written for this change. The
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
flows (25 branches on case13659, all with unity taps). Reversing a transformer
also scales `r,x` by `τ²`, inverts `τ`, negates `φ`, and reverses angle bounds.
Both tap-adjusted equations are invariant under this transformation; their
flows are sign-corrected before comparison. For the other three conventions,
a reversed nonunity transformer is reported as `incompatible_preprocessing`:
no objective or flow parity comparison is made for those equations. This does
not abort comparison of the invariant modes.

The synthetic `case3_reversed_transformer.m` adds an antiparallel nonunity
transformer with resistance, a shift and asymmetric angle limits. Run it with
`run.sh`, then run `compare.py` with `--check --expect-reversed-nonunity`.
The latter flag requires an actual PowerModels parser reversal and permits
only the three explicitly incompatible modes to be skipped. The two invariant
modes must still meet every small-case numerical gate. The generated
`case3-reversed-reference.json` keeps reference direction signs for the offline
Rust regression. Comparator tests run with:

```sh
python3 -m unittest discover -s evals/dc-variants -v
```

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
case300, the maximum Tellegen-versus-PowerModels relative objective difference,
comparing each convention with its matching reference, is 2.40e-11. The largest
nodal-balance residual is 5.02e-10 pu. The conventions themselves have different
objectives (USD/h):

| Convention | Tellegen | PowerModels |
| --- | ---: | ---: |
| Legacy series susceptance | 517852.439484 | 517852.439485 |
| PowerModels DCP | 517851.075202 | 517851.075202 |
| MATPOWER / PowerModels DCMP | 517585.534856 | 517585.534856 |
| Tap-adjusted series susceptance | 518068.867776 | 518068.867769 |
| Reactance only | 517358.815068 | 517358.815056 |

The legacy, DCP and tap-adjusted series values also agree at their reported
precision with the independent values in [#206](https://github.com/eigenergy/tellegen/issues/206).
These are DC comparisons, not published PGLib AC costs. The compact reports
include both unrounded objectives, absolute and relative gaps, statuses,
residuals and units. The November 11 enrichment recovered the original raw
outputs and verified every existing report metric before adding fields; it
does not represent a fresh large-case solver run. Case and raw-output SHA-256
hashes and Julia/PowerModels/Ipopt versions identify the numerical evidence.

Case13659 is a stress test, **not a passed optimality qualification**:

| Convention | Relative objective difference | Max nodal balance residual (pu) |
| --- | ---: | ---: |
| Legacy series susceptance | 5.40e-5 | 2.18e-3 |
| PowerModels DCP | 7.59e-5 | 2.27e-3 |
| MATPOWER / PowerModels DCMP | 1.21e-10 | 5.82e-7 |
| Tap-adjusted series susceptance | 2.64e-5 | 1.35e-3 |
| Reactance only | 3.07e-5 | 1.78e-4 |

The several-hundred-dollar-per-hour solver gaps exceed the 26.4 USD/h
phase-shift effect reported in #206 (about 3e-6 relative), so these OPF runs
cannot reliably distinguish that small physical-model effect. The historical
26.4 USD/h observation should itself not be treated as qualified evidence.
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

## Diagnostics at the converged local AC OPF

`ac_opf_operating_point` evaluates all five equations at the **same** converged
local AC OPF voltages/angles in PowerModels' parsed coordinates, comparing
with signed symmetric transfer `(Pfrom-Pto)/2`. It records per-transformer
`τ`, `ρ = vf vt`, `δ = θf − θt − φ`, signed and absolute MW errors, plus
aggregate distributions. The selected rows have nonunity taps or nonzero
shifts. Case300 has 63 such rows: `ρ` ranges from 0.9415 to 1.1182 and `δ`
from −0.1798 to 0.1597 radians. Mean absolute transfer error is 10.9814 MW
for legacy series, 8.8968 MW for tap-adjusted series, and 8.5811 MW for DCMP.
These are observations at one operating point, not a universal model ranking
or a comparison of separately optimized DC dispatches.

With `g = r/(r²+x²)`, the general from-end angle derivative is
`ρ/τ (k cos δ + g sin δ)`; the symmetric-transfer derivative is
`ρ k cos δ/τ`. Only at `δ=0` does either reduce to `ρ k/τ`. Thus the
`ρ`-versus-`τ` midpoint argument assumes this expansion point (or a stated
small-angle approximation). The renamed
`unit_voltage_phase_compensated_ac_slope_max_error` is a separate identity
check at unit voltage and `δ=0`, not an AC accuracy metric. Unit tests check
the general derivatives against the independent complex pi-model at nonzero
angles and unequal voltages. Failed AC iterates never enter these diagnostics.
