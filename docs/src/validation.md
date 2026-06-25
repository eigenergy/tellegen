# Validation

tellegen is checked against the published PGLib reference solves. For each case and
variant, `$PGLIB_OPF_PATH/BASELINE.md` tabulates the PowerModels.jl with IPOPT
reference: `DC ($/h)`, `AC ($/h)`, `QC Gap (%)`, and `SOC Gap (%)`. The
[Methodology](methodology.md) chapter describes how the harness produces the
comparison; this chapter records what each comparison asserts. The measured figures
are whatever the current harness run writes; they are not reproduced here.

## DC objective

tellegen's DC objective (constant cost term included) is compared against the
published `DC ($/h)`. The per-unit cost scaling cancels exactly, so the comparison
is in dollars per hour directly.

## Relaxation lower bound

The SOCWR objective (`socwr_opf` objective) must lower-bound the published AC optimum
(`AC ($/h)`):

$$ \text{socwr} \le \text{AC} + \text{tol}. $$

A bound violation is a correctness failure.

## SOC gap

The gap is

$$ \text{gap} = \frac{\text{AC} - \text{socwr}}{\text{AC}} \cdot 100, $$

compared against the baseline `SOC Gap (%)`. tellegen's SOCWR is the Jabr SOC
relaxation — the same family as the baseline `SOC` column — so a near-zero
difference in gap is the expected result. The published SOC bound is recovered as
$\text{AC} \cdot (1 - \text{SOC gap}/100)$.

All three variants are exercised: typical, congested (API), and small-angle (SAD).
tellegen's `AcNetwork` carries the angle-difference limits and the SOCWR enforces
them in W-space, so the SAD relaxation tracks the published SAD SOC. Where a case
shows a large SOC gap, that is relaxation-quality data, not a failure.
