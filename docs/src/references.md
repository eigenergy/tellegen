# References

## PGLib-OPF

The benchmark corpus and the reference solves.

- **IEEE PES Power Grid Library — Optimal Power Flow**, v23.07.
  <https://github.com/power-grid-lib/pglib-opf>
- Archive report: S. Babaeinejadsarookolaee et al., *The Power Grid Library for
  Benchmarking AC Optimal Power Flow Algorithms*, arXiv:1908.02788.
- **License:** the PGLib data is **CC BY 4.0**; the software is MIT. The corpus is
  read from `$PGLIB_OPF_PATH` and never vendored. Per-file header attribution is
  preserved for any case quoted in these docs.
- Reference solves: `$PGLIB_OPF_PATH/BASELINE.md` — PowerModels.jl with IPOPT.

## Formulation references

- **Jabr SOCWR**: R. A. Jabr, *Radial distribution load flow using conic
  programming*, IEEE Transactions on Power Systems, 21(3), 2006 — the W-space
  second-order-cone relaxation, as also implemented in
  [PowerModels.jl](https://github.com/lanl-ansi/PowerModels.jl) (BSD-3-Clause). The
  conic implementation in tellegen is independent.

## Solvers and linear algebra

- [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs) — the convex conic
  and quadratic-program solver.
- [faer](https://docs.rs/faer/latest/faer/) — the dense linear algebra used by the
  sensitivity driver.
