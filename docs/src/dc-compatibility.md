# DC OPF variants and optimality compatibility

Tellegen keeps **Legacy Tellegen** as its default. To reproduce a PowerModels
DCP or PGLib **DC** baseline, select **PowerModels DCP / PGLib DC**. To use the
MATPOWER DC equations, select **MATPOWER / PowerModels DCMP**. Loading a PGLib
case does not automatically change the selected variant.

These options select different optimization problems. An optimal solution of
one problem need not have the same cost, dispatch, congestion or prices as
another. None is a guarantee of AC feasibility, AC optimality, or a lower bound
on AC cost. The PGLib AC baseline is a different comparison.

## Compatibility matrix

Let `Δθ = θfrom − θto`, `k = x/(r²+x²)`, `τ` be the effective tap magnitude,
and `φ` the phase shift in radians. Source tap zero means unity. Branch power
is positive from the from-bus to the to-bus. The table describes **equation
compatibility**; optimal-objective agreement also requires the checklist below.

| UI selection / API token | Active branch flow | PowerModels DCP / PGLib DC | MATPOWER DC / PowerModels DCMP | AC OPF objective |
| --- | --- | --- | --- | --- |
| **Legacy Tellegen (default)** — `series_susceptance` | `k(Δθ−φ)` | When all shifts are zero | When `r=0` and `τ=1` | No equality guarantee |
| **PowerModels DCP / PGLib DC** — `series_susceptance_no_phase_shift` | `kΔθ` | Exact equations | When `r=0`, `τ=1`, `φ=0` | No equality guarantee |
| **MATPOWER / PowerModels DCMP** — `tap_adjusted_reactance` | `(Δθ−φ)/(xτ)` | When `r=0`, `τ=1`, `φ=0` | Exact equations | No equality guarantee |
| **Tap-adjusted series susceptance** — `tap_adjusted_series_susceptance` | `k(Δθ−φ)/τ` | When `τ=1`, `φ=0` | When `r=0` | No equality guarantee |
| **Reactance only (1/x)** — `reactance_only` | `Δθ/x` | When `r=0` | When `τ=1`, `φ=0` | No equality guarantee |

Conditional entries are sufficient conditions on every active branch, not an
exhaustive list of cases that happen to yield equal costs. All five options
agree when `r=0`, `τ=1`, `φ=0`. Taps can change radial-network angles without
changing its fixed-injection flows; meshed networks redistribute flows.

The built-in DCP and DCMP equations are checked against **PowerModels 0.19.9**.
MATPOWER compatibility here means its documented DC equations, tested
numerically through PowerModels DCMP; it is not a claim that a separate
MATPOWER executable ran. The other three modes use explicitly labelled,
validation-only PowerModels extensions.

## What must match before comparing optimal objectives

- Use the same case revision and per-unit base, active equipment and island
  references. Resolve bus identities and generator aggregation consistently.
- Match the full generator cost curves, including constant terms, quadratic
  scaling and piecewise-linear segments, and match generation limits.
- Match thermal **active-power** limits, angle-difference limits, shunt active
  withdrawals, fixed HVDC injections and any caller-supplied constraints.
- Match preprocessing: zero-impedance handling, three-winding lowering,
  reference angles, missing ratings and unconstrained angle windows.
  Tellegen's network path applies a ±60° fallback and may synthesize an unrated
  thermal limit; that is not equivalent to an unlimited MATPOWER branch.
- First check primal residuals and solver status, then compare objective using
  an explicit numerical tolerance. Equal objective does not imply unique
  dispatch or prices. Compare dispatch only when uniqueness is established,
  and duals only for a nondegenerate active set, in matching units and signs.

The selector changes neither thermal ratings nor voltage/reactive constraints.
DC models omit the latter. A user wanting an AC-operable dispatch should replay
it with AC power flow and inspect voltages, reactive generation and apparent
power at **both** branch ends. A locally solved AC OPF is a feasible local
reference, not a certified global optimum.

## Selecting and retaining a variant

The browser's **Calculation → DC variant** control rebuilds the study and
invalidates previous results and sensitivities. An imported typed instance owns
its convention; its selector is read-only. A nondefault variant cannot fall
back silently to the legacy-only HTTP calculation service.

The same stable token works in CLI/Python JSON requests and `solveModule`:

```json
{
  "formulation": "dcopf",
  "branch_susceptance_formula": "series_susceptance_no_phase_shift"
}
```

It also applies to `dcpf`. Other formulations reject this option. Omission on a
network preserves the legacy default; omission on a typed instance inherits
its convention. A matching explicit request is accepted; a conflict is an error.
Every new DC result reports the resolved `branch_susceptance_formula`.

```ts
const study = await createStudy(moduleJson, 'dcopf', {
  branch_susceptance_formula: 'tap_adjusted_reactance'
});
```

Rust offers `Study::new_with_options` and `Study::from_module_with_options`
with `StudyOptions`. Durable `CreateStudy` requests accept the same optional
field. Saved instances, solutions and study sessions retain the choice;
network-only exports intentionally contain network data without solver options.
The original constructors retain their behavior.

`SeriesAdmittance(Susceptance)` sensitivities remain derivatives with respect
to raw series `b`, holding raw `g` and transformer settings fixed. Tellegen
applies the selected coefficient's chain rule to both the matrix and its phase
injection. This does not introduce transformer-tap sensitivities.

## Engineering basis and evidence

Ignoring resistance (`1/x`) and ignoring conductance (`x/(r²+x²)`) are distinct
conventional approximations. Transformer ratios and phase shifts are further
choices. At unit bus voltage magnitudes and effective angle difference zero,
the full complex transformer model has active-flow angle derivative
`x/((r²+x²)τ)`. That local derivative justifies the tap-adjusted series option;
it does not establish that it predicts every network's AC flows best.

- [PowerModels formulation details](https://lanl-ansi.github.io/PowerModels.jl/stable/formulation-details/)
  and [v0.19.9 DC implementations](https://github.com/lanl-ansi/PowerModels.jl/blob/v0.19.9/src/form/dcp.jl).
- [MATPOWER DC modeling](https://matpower.app/manual/matpower/DCModeling.html)
  and [PGLib v23.07 baselines](https://github.com/power-grid-lib/pglib-opf/blob/v23.07/BASELINE.md).
- Stott, Jardim and Alsaç, [DC Power Flow Revisited (2009)](https://doi.org/10.1109/TPWRS.2009.2021235).
- Taheri and Molzahn, [Optimizing Parameters of the DC Power Flow (2024)](https://arxiv.org/abs/2310.00447).

The reproducible harness in
[`evals/dc-variants`](https://github.com/eigenergy/tellegen/tree/main/evals/dc-variants)
pins the Julia environment, identifies built-in versus custom models, checks
residuals before objectives, records input hashes, and reports AC failures and
constraint violations. Its small strictly convex congested case is also an
offline Rust regression fixture. See [validation](validation.md) for the wider
solver qualification and its limits.

The recorded small and 300-bus comparisons agree with PowerModels across all
five options. The 13,659-bus stress case still shows numerical discrepancies:
matching DCP equations does **not** currently guarantee PGLib objective parity
on that case. Tellegen's existing `optimal` status also includes Clarabel's
reduced-accuracy termination, so inspect primal residuals when benchmarking.
The harness README records the objective gaps, residuals and AC convergence
failures; this change does not retune the solver or change its status mapping.
