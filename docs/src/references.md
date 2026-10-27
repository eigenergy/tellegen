# References

## Citing tellegen

- S. Talkington, F. Geth, Q. Zhang, L. Xie, and S. Liu, *Interactive Power Flow
  in the Browser*, The Inaugural ACM Conference on Digital Transformation (ACM
  DXConf 2026), Ann Arbor, MI, USA, October 2026. ACM, New York, NY, USA, 9 pages.
  [doi:10.1145/3847238.3849038](https://doi.org/10.1145/3847238.3849038),
  [arXiv:2610.01922](https://arxiv.org/abs/2610.01922). The repository's
  `CITATION.cff` and README carry the BibTeX entry.

## PGLib-OPF

The benchmark corpus and the reference solves.

- **IEEE PES Power Grid Library — Optimal Power Flow**, v23.07.
  <https://github.com/power-grid-lib/pglib-opf>
- Archive report: S. Babaeinejadsarookolaee et al., *The Power Grid Library for
  Benchmarking AC Optimal Power Flow Algorithms*, arXiv:1908.02788.
- **License:** the PGLib data is **CC BY 4.0**; the software is MIT. The corpus is
  read from `$PGLIB_OPF_PATH` and never vendored. Per-file header attribution is
  preserved for any case quoted in these docs.
- Reference solves: `$PGLIB_OPF_PATH/BASELINE.md` (PowerModels.jl with IPOPT).

## Formulation references

- **Jabr SOCWR**: R. A. Jabr, *Radial distribution load flow using conic
  programming*, IEEE Transactions on Power Systems, 21(3), 2006. The paper gives
  the W-space second-order cone relaxation, also implemented in
  [PowerModels.jl](https://github.com/lanl-ansi/PowerModels.jl) (BSD-3-Clause); the
  conic implementation in tellegen is independent.

## Solvers and linear algebra

- [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs): the convex conic
  and quadratic program solver.
- [faer](https://docs.rs/faer/latest/faer/): the dense linear algebra used by the
  sensitivity driver.
