# Multiconductor PF numerical oracle

The checked-in files under `crates/tellegen/tests/data/mc_pf` are nine small
BMOPF snapshots and their OpenDSSDirect references. They cover a coupled
three-phase line, unbalanced four-wire loads, delta loads, single-phase
neutral variants, and grounded, floating-neutral, tapped, delta-wye, and
wye-delta transformers. The Rust integration test checks every bus terminal,
complex voltage and retained factor count. Line and load terminal currents
use the actual OpenDSS conductor NodeOrder and are aggregated by conductor
and compared numerically, along with total absorbed complex power. The test
also checks total complex source injection against all element absorption. Transformer and reactor ports are required to be
present and finite; this harness does not claim direct numerical current
parity for those two classes. Finite grounding reactors remain physical
shunts, while exact ground constraints require a different reference mapping.
Separate independent transformer primitive and loaded YY/DD evidence is
recorded in `multiconductor-qc-cases.md` and `evidence/mc-pf-yprim`.

The references were generated from the BMOPFTools.jl comparison cases at
commit `8ca84ab12c0c91aaa8ad4c9986d6adbeb969ea0b` with OpenDSSDirect.py
`0.9.4`. They use `controlmode=off`, `algorithm=normal`, a `1e-11`
solution tolerance, `maxiterations=1000`, strict constant-power load limits,
`vsource model=ideal puZideal=[1e-12,0]`, and `transformer ppm_antifloat=0`.
The source profile is intentional: it matches BMOPF's prescribed terminal
voltages and does not represent a default OpenDSS finite-source run.

The solver accepts a result when the voltage update is within its tolerance
and the physical KCL residual is within `1e-6 A + 1e-8` times prepared
incident current. Loads must carry explicit nominal branch voltages; the
solver reports an error instead of inferring a low-voltage base from an
unrelated source or transformer.

To regenerate references, install the pinned Python package and provide the
upstream comparison directory:

```text
python -m pip install opendssdirect.py==0.9.4
python crates/tellegen/tests/generate_mc_pf_oracle.py \
  --dss-dir /path/to/BMOPFTools.jl/test/data/pf_comparison
```

The generator records input and DSS source SHA-256, source commit, actual backend version, solver configuration,
and upstream fixture attribution in every reference. The fixture files are
and upstream fixture attribution in every reference. The exact upstream
license text is retained with the Rust fixtures at
`crates/tellegen/tests/data/mc_pf/LICENSE.md` and with the browser fixture at
`apps/web/tests/fixtures/mc-pf-browser/LICENSE.md`; it governs redistribution
of this test data.
