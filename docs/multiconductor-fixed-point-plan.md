# Multiconductor AC power flow with a retained fixed-point factorization

Planning baseline: 7 September 2026. This document is the historical planning
baseline and proposed quality gates. The current implementation and supported
raw-input contract are documented in
[`multiconductor-power-flow.md`](multiconductor-power-flow.md), with numerical
cases in [`multiconductor-qc-cases.md`](multiconductor-qc-cases.md) and reader
loss findings in [`multiconductor-semantic-audit.md`](multiconductor-semantic-audit.md).
The requested method is OpenDSS Normal-style current injection with one global
numeric factorization per prepared, electrically unchanged snapshot.
GPT-5.6 Luna high performs bounded implementation stages. GPT-6 Astra medium
owns planning, supervision, independent numerical review, and acceptance.
This proposal addresses the latest fixed-point requirement and includes
transformers; the superseded Newton proposal has been removed as authorized.

## 1. Deliverable and supported electrical scope

The first complete deliverable accepts BMOPF JSON through the pinned PowerIO
reader, prepares a multiconductor network, solves its steady-state AC power flow,
and returns terminal-aware voltages, currents, powers, and convergence evidence
through native Rust and the browser WASM module transport.

- Lines: full coupled three- and four-conductor series matrices, supported shunt
  matrices, explicit conductor maps, declared lengths and physical units.
- Loads: constant-power WYE and DELTA connections, including explicit neutral
  connections and unbalanced per-branch complex powers.
- Voltage sources: ideal prescribed voltages or finite-impedance Norton models
  according to the actual JSON semantics; the two must not be conflated.
- Transformers: common two-winding YY, YD, DY, DD, and single-phase banks with
  fixed taps, where the canonical schema faithfully represents their parameters.
- Grounding: perfect ground constraints and finite grounding impedances,
  including transformer neutral impedances; explicit neutral voltage is retained.
- Topologies: radial and meshed networks, with conductor-level connectivity and
  reference validation rather than an assumption of one balanced slack bus.

No regulator/capacitor control loops, tap optimization, PV reactive regulation,
OPF, dynamic loads, time series, or MC sensitivities enter the initial release.
Multiwinding transformers are an optional later milestone selected from the
actual corpus. Unsupported active devices or unrepresentable transformer data
cause a precise preflight error, not an electrically incomplete successful solve.
Fixed passive shunts needed by supported lines and transformers remain in scope.
After this profile, extend compatibility through separately reviewed milestones:
general fixed shunts and fixed switches, ZIP loads, supported fixed generators,
then additional winding counts. Controls are a final optional outer-loop project;
each changed electrical snapshot requires its own factorization contract.

## 2. Evidence and reference discipline

Tellegen currently pins registry PowerIO 0.11.0 in `Cargo.lock`. The sibling
PowerIO checkout is useful for inspection but must not substitute silently for
the published reader and matrix implementation used by the build.
Inspect the exact locked/tagged implementation before deciding what to reuse.
Keep source JSON, canonical module, diagnostics, and semantic field mappings
together so a lossless parse can be distinguished from complete solve semantics.

The local BMOPFTools checkout is readable at `../BMOPFTools.jl`; the inspected
HEAD is `8ca84ab12c0c91aaa8ad4c9986d6adbeb969ea0b`, with a clean inspected
working tree. Record fixture-generation commands before using output as evidence.
Its `test/powerflow_comparison_tests.jl` around line 2857 allows a Yprim relative
error of `1e-2` and documents dropped magnetizing susceptance/shunt differences.
Reuse suitable fixtures and extraction helpers, not that loose acceptance gate
or the associated model omissions. In `test/admittance_tests.jl`, an ideal zero-Z
transformer convention returning Y=0 is not a conducting transformer primitive.
Initially reject unsupported ideal transformers; constant MNA is a separately
reviewed extension. Do not insert an undocumented tiny impedance.
The `test/data/pf_comparison/pf_3wdg_unequal_kva.dss` regression documents issue
356: PowerIO 0.11 resistance conversion uses per-winding ratings while DSS uses
the winding-1/common base. Do not copy unresolved expectations. Include unequal
ratings in supported two-winding tests and revisit three-winding cases if added.

The supplied [source archive](/Users/uqfgeth/Downloads/electricdss-code-r4176-trunk.zip)
contains the reference `Version8/Source` tree.
Selected source files are extracted at `/tmp/tellegen-opendss-reference`.
Record archive checksum and selected-file hashes in the implementation evidence.
The archive also contains Version7; explicitly pin the Version8 paths.
Relevant source anchors, whose line numbers belong to this extraction, are:

- `Solution.pas:1036`, `DoNormalSolution`: source and PC injected currents are
  solved against the system matrix; OpenDSS may rebuild it when state changes.
- `Transformer.pas:2116`, `CalcY_Terminal`: construction and inversion of ZB,
  followed by winding-voltage and tap transformations.
- `Transformer.pas:2039`, `BuildYPrimComponent`: terminal-reference stamping.
- `Transformer.pas:2000`, `AddNeutralToY`: neutral grounding behavior.
- `Transformer.pas` around 2230: magnetizing branch treatment on winding 2.
- `Transformer.pas:1173`, `SetTermRef`; 1055, `DeltaDirection`; and 1092,
  `VBase`: connection orientation and winding base-voltage treatment.

These anchors establish a source-level reference, not proof that an arbitrary
installed OpenDSS executable matches r4176. Pin and record the executable/version
used to generate each numerical oracle and document its relationship to the
supplied r4176 Version8 baseline before claiming parity. Source study is read-only
during this planning task.
The EPRI [Normal solution description](https://opendss.epri.com/TheNormalSolutionmethod.html)
provides the primary conceptual reference alongside the pinned Pascal source.
The supplied [algorithm paper](</Users/uqfgeth/Downloads/2305.04405v1 (2).pdf>)
([arXiv](https://arxiv.org/abs/2305.04405)), sections 3 and 5, informs the
iteration only; it is not the transformer derivation reference.
Local [PF comparison tests](/Users/uqfgeth/Documents/GitHub/BMOPFTools.jl/test/powerflow_comparison_tests.jl)
and [admittance tests](/Users/uqfgeth/Documents/GitHub/BMOPFTools.jl/test/admittance_tests.jl)
provide fixture context; their assertions remain subject to the QC caveats above.

## 3. Numerical contract and signs

Use physical complex voltages, currents, admittances, and powers internally for
the first implementation, or introduce an explicitly documented diagonal scaling.
All residuals and returned units must state the conversion. Each terminal index
maps back to its original bus identity and terminal identity, including neutral.

For each load connection, let C map nodal voltage to branch voltage `u = C v`.
For positive consumed complex power s, the physical nodal withdrawal is
`i_load(v) = C^T conjugate(s / (C v))`, with elementwise branch division.
C is a real signed incidence map; WYE-neutral and DELTA use the same mechanism.
Ground terminals are eliminated consistently from C and recovered in outputs.

Choose a fixed nominal load admittance Yref from declared nominal branch voltage
and power, and stamp its nodal form into the system matrix. The compensation
injection is `i_comp(v) = Yref v - i_load(v)`.
With passive/source-Norton terms in Ypassive, the fixed matrix is
`Y = Ypassive + Yref`. Consequently `Y v = b_source + i_comp(v)`.
Partition unknown terminals u and fixed terminals f to obtain
`Y_uu v_u^(k+1) = b_source,u + i_comp,u(v^k) - Y_uf v_f`.
There is no extra minus sign before compensation current.

Derive this independently in tests from physical KCL. The paper's Equation 30
is visibly inconsistent with Equation 29; do not transcribe its sign without
checking the definitions. A one-load analytical fixture must detect reversed
consumption, reactive sign, conjugation, and DELTA incidence errors separately.
For a scalar resistive feeder, use the independent high-voltage root
`V = (E + sqrt(E^2 - 4 R P)) / 2` within its feasible domain; this avoids making
another iterative solver the only evidence that consumption signs are correct.

The initial voltage should come from a documented linear nominal-admittance
solve using the retained factorization, with fixed source phasors preserved.
OpenDSS-style nominal admittance compensation is the baseline, not a silent
replacement with a passive-only Z-bus iteration. Near-zero branch voltage must
produce a controlled diagnostic unless a separately specified load model permits
a different law. Do not silently adopt OpenDSS low-voltage load fallbacks when
the requested model is strictly constant power.

Factor Y_uu once after assembly. Every initialization/iteration thereafter uses
triangular solves against that retained factor. Small dense inversions/solves for
line and transformer primitives are separate preparation operations and excluded
from the global factorization count; report those categories unambiguously.
Changes to topology, impedances, grounding, tap positions, load Yref, or source
impedance invalidate the prepared snapshot. RHS-only source changes may reuse it
only where the preparation contract proves the matrix unchanged.

## 4. Primitive preparation and transformers

Prepare each component locally before global assembly. Validate dimensions,
finite values, units, orientation, terminal mappings, status, and required fields.
Resolve linecodes and materialized electrical data through the canonical semantic
path. An unresolved reference must never create a zero or guessed impedance.
For lines, solve the full coupled series matrix and stamp both ends and supported
shunt terms; retain neutral conductors rather than applying implicit Kron reduction.

Implement a winding-domain transformer primitive modeled on the reference source:

1. Convert winding impedances and ratings to one documented winding-voltage base.
2. Build the leakage ZB matrix; for N windings it has dimension N-1.
3. Solve/invert that small matrix and construct Y_1Volt using the `[-1 I]`
   winding-difference transformation with the correct transpose orientation.
4. Apply real voltage-ratio and fixed-tap transformations to construct Y_Term.
5. Apply WYE/DELTA connection maps and the source's terminal-reference mapping
   to construct the physical terminal primitive and stamp it globally.
6. Include winding losses, declared magnetizing conductance and susceptance,
   and finite/perfect neutral grounding with verified units and base conversion.

For two windings the leakage solve is small, but connection and grounding are
still first-class physics. Never replace the transformer with three independent
balanced ratios when the selected connection couples phase voltages/currents.
Confirm phase shifts using no-load phasors and loaded terminal currents.
Match the common-base source relation `ZBase = 1 / (VABase / Fnphases)` and
test unequal winding ratings explicitly; do not assume each winding's own kVA
is interchangeable with the reference leakage base.
Identify the magnetizing branch's side explicitly; compare its placement with
the r4176 winding-2 implementation rather than matching only total no-load loss.

Define a visible ppm-antifloat policy and reproduce reference behavior only when
requested/represented. A numerical grounding aid changes the model and source
power balance; it cannot silently mask an unanchored system. Record any enabled
antifloat admittance in preparation diagnostics and oracle metadata.
Distinguish a physical ppm-antifloat model term from a compensated numerical
shunt that cancels from final KCL. Match declared oracle profiles and reject
floating rank deficiency when antifloat is disabled; silently grounding a case
cannot be described as preserving exact BMOPF physics.
Zero leakage ideal transformers require constraints, not a zero primitive.
If later implemented through constant MNA, retain one factorization of the fixed
augmented matrix and add dedicated current/output reconstruction and singularity
tests. That extension is not required to finish finite-leakage common transformers.

## 5. Repository integration

Keep balanced `model/ac.rs` and `problem/pf_ac.rs` behavior intact. Natural new
locations are `model/mc_ac.rs` for numerical preparation and terminal mappings,
and `problem/pf_mc_ac.rs` for the fixed-point driver and solve diagnostics.
First assess `powerio-matrix`'s locked multiconductor preparation/assembly API;
reuse correct canonical assembly rather than creating a second interpretation.
The locked v0.11.0 builder has an older permissive ideal-plus-leakage path; the
sibling main `powerio-matrix/src/matrix/multiconductor.rs` around 336 explicitly
rejects leakage, non-WYE connections, floating neutrals, core shunts, and tap
controls. Neither establishes the required full transformer implementation.
Reuse verified node/line utilities and implement the reviewed full Yprim path.
If an upstream gap prevents faithful support, make the dependency change explicit
and ship only after its version and release sequencing are resolved.

`solve.rs::solve_sparse` currently factors on every call and handles real f64
systems. Add a retained complex sparse LU wrapper, or a reviewed real-block
representation if needed for WASM; benchmark memory and verify equivalent results.
Do not use a dense global matrix or an explicit global inverse.
Use general complex sparse LU: reciprocal complex admittance is commonly
complex-symmetric, not Hermitian or positive definite. Existing faer 0.24.4 is
the first candidate; exercise it in actual WASM early. KLU is not a prerequisite
for the fixed-point algorithm or a reason to delay the initial portable solver.
`faer` and `num-complex` currently follow the `sensitivity` feature. Decide whether
MC PF initially shares that feature or receives its own feature, then forward it
through `crates/tellegen-wasm/Cargo.toml` and capability reporting consistently.

Use `McAcPfInstance` and `McAcPfSolution` at the portable boundary. Inspect their
actual field coverage before implementation. Provide terminal-aware result fields
for currents and source reactive powers if the typed solution cannot carry them,
alongside the portable solution; these are user-visible outputs, not only internal
validation evidence. Coordinate a future upstream schema extension if appropriate.
`emit.rs` is the existing pattern for source-row-preserving solution emission.
`api.rs::solve_module_json` needs MC network/instance routing and explicit support
errors. Stored instances must retain their declared interpretation.
Current `SolveResponse` uses balanced numeric IDs and bus-level pu/MW values;
add a terminal-aware response rather than changing the meaning of those fields.
Reject unsupported edit/sensitivity requests explicitly for this new formulation.

The WASM `dist.rs` path is topology ingestion through `to_graph()`, not electrical
preparation. Preserve the canonical module for solving instead of solving the
render graph. `packages/engine/src/protocol.ts` already forwards `solve_module`;
extend `index.ts` types/capabilities and actual browser worker coverage.
An initial MC solve need not extend every Study optimization/intervention flow.

## 6. Eight implementation and review stages

Each stage is a bounded Luna-high implementation assignment followed by an
Astra-medium independent review. Reviewers inspect physical evidence and source
semantics, not only whether tests written by the implementer pass.

1. **Corpus and semantic inventory.** Pin readers/references; inventory the
   selected MV and ENWL DSS corpora below and reproducibly export candidate
   BMOPF JSON/DSS pairs; list every active component and parameter. Produce a support
   matrix, units contract, provenance manifest, and explicit unsupported cases.
   Exit: Astra agrees the selected cases are fully representable and in scope.
2. **Indexing and retained linear solve.** Add terminal/ground/fixed-source maps,
   deterministic assembly, reusable complex factorization, and feature wiring.
   Exit: complex residual tests and native/WASM execution confirm one factorization.
3. **Line/load/source primitives.** Implement coupled lines, WYE/DELTA CP current
   laws, Yref compensation, and source/grounding semantics. Exit: independent
   primitive tests and analytical feeder KCL establish signs and physical units.
4. **Transformer primitives.** Implement the enumerated finite-leakage two-winding
   connections, fixed taps, neutral and magnetizing branches. Exit: connection-by-
   connection Yprim, no-load phasor, and loaded current evidence passes review.
5. **Network assembly and iteration.** Assemble once, partition constraints,
   iterate compensated currents, check convergence, and recover element powers.
   Exit: assembled Ybus and physical KCL pass before large feeder comparisons.
6. **Portable/native/WASM API.** Emit typed solutions and terminal-aware views;
   wire routing, capabilities, errors, and browser transport. Exit: the same
   canonical fixture yields equivalent native and actual browser WASM results.
7. **External comparison and robustness.** Run pinned OpenDSS and BMOPF fixtures,
   adversarial inputs, conditioning diagnostics, and scaling benchmarks. Exit:
   Astra signs off a discrepancy ledger with no unexplained supported-case error.
8. **Documentation and release checks.** Document support, units, settings,
   diagnostics, snapshot invalidation, reference provenance, and limitations.
   Exit: existing balanced/ingest tests and shipping gates remain green; review
   the final diff and evidence before calling the implementation complete.

## 7. Quantitative QC gates

Acceptance proceeds primitive -> assembled Ybus -> currents/KCL -> power flow.
Compare entries using `|actual-reference| <= atol + rtol*|reference|`, with an
absolute floor appropriate to the stated physical unit. Check near-zero entries
explicitly; a large matrix norm must not conceal a missing neutral/shunt branch.
The following are proposed starting gates, subject to corpus and conditioning
review before implementation; they are not measurements of current accuracy.
Establish the final numerical gates before accepting implementation results.

| Quantity | Proposed absolute tolerance | Relative tolerance |
| --- | --- | --- |
| Primitive and assembled admittance | 1e-10 S | 1e-8 |
| Linear solve backward error | dimensionless scaled residual 1e-12 | n/a |
| Terminal voltage against matched oracle | 1e-4 V | 1e-7 |
| Element/source terminal current | 1e-6 A | 1e-7 |
| Element/source complex power | 1e-3 VA | 1e-7 |
| Iteration voltage change | 1e-6 V | 1e-9 |
| Final unknown-terminal physical KCL | 1e-7 A | 1e-8 |

Scale the KCL relative term by incident component-current magnitudes or a declared
current base, never by the near-zero net current being tested. Report raw and
scaled residuals. Both voltage change and final physical KCL must pass on the
returned iterate. Nonfinite values, iteration limit, stagnation, and singular
systems return controlled outcomes, never a feasible flag with invalid numbers.
Distinguish a converged PF equation solution from satisfaction of operational
voltage/thermal limits. Report such violations separately; small KCL residual
does not establish operational feasibility or OPF constraint satisfaction.
Check total source supply = load consumption + passive complex losses/shunts
using consistent port orientations and explicit grounding/antifloat contributions.
Compute complex port power as `sum(V .* conjugate(I))`; use `real(V^H I)` for
its real component only. Avoid the double-conjugation error in a helper that
combines conjugation and a conjugating transpose of voltage.

Ill-conditioned cases need a documented estimate/diagnostic, backward residual,
and sensitivity-informed explanation before tolerances change. Astra reviews
each exception with units and the affected quantities. No blanket `1e-2` escape
gate and no threshold increase to conceal a known omitted physical term.
Record iteration count, matrix dimensions/nonzeros, factorization count, factor
time, repeated-solve time, and memory on a small and representative large feeder.

## 8. Required tests and remaining inputs

Start with `test/data/pf_comparison/` fixtures `pf_1ph_line.dss`,
`pf_3ph_line.dss`, `pf_delta_load.dss`, `pf_1ph_freeneutral.dss`,
`pf_1ph_impedanceneutral.dss`, `pf_dy_xfmr.dss`, `pf_yd_xfmr.dss`,
`pf_dy_xfmr_tap.dss`, and `pf_dy_xfmr_rneut.dss`. Audit each before promotion.
The user has selected these source corpora; corpus selection is resolved:

- BMOPFTools [MV corpus](https://github.com/frederikgeth/BMOPFTools.jl/tree/main/test/data/MV),
  locally at [test/data/MV](/Users/uqfgeth/Documents/GitHub/BMOPFTools.jl/test/data/MV).
- BMOPFTools [ENWL corpus](https://github.com/frederikgeth/BMOPFTools.jl/tree/main/test/data/ENWL),
  locally at [test/data/ENWL](/Users/uqfgeth/Documents/GitHub/BMOPFTools.jl/test/data/ENWL).
- The original [ENWL four-wire directory](/Users/uqfgeth/Documents/Data/ENWL/data/Four-wire).

These are OpenDSS source inputs; the user does not need to supply additional
BMOPF JSON. Stage 1 inventories circuit entry points, recursive includes, active
devices and parameters, conductor/neutral representations, and required ancillary
files, then exports chosen cases to BMOPF JSON through pinned conversion tooling.
Record exact exporter/runtime/dependency versions, checkout revision, commands,
settings, and hashes of the full source closure and generated JSON so the export
is reproducible. The `main` URLs identify the selected collections; evidence must
also identify the exact commit used. Preserve original DSS files for the oracle
and distinguish any generated or comparison-profile copies from those originals.
Treat the original four-wire directory as a separate provenance source: compare
its file hashes and circuit semantics with the repository ENWL copy before
claiming either identical bytes or equivalent networks.

Initial local inventory found 128 `Master.dss` entry points and no JSON in each
ENWL root. Of 642 shared case-insensitive relative paths, 641 were byte-identical;
the sole differing shared file was `.DS_Store`. Repository-only `License.md` and
original-only `network_1/.DS_Store` account for the other path differences. The
electrical source files therefore match in this inspected local snapshot; freeze
their hashes in the evidence manifest rather than assuming future copies match.
File presence is insufficient to establish active physics: ENWL network 23,
feeder 1 redirects linecodes, lines, and loads, but not its `Transformers.txt`.
Its master also declares short-circuit source parameters, a grounding reactor,
and an energy meter. Inventory the compiled circuit and classify these semantics.

The MV directory has nine files, no JSON, and no standalone master; its lexical
inventory contains 327 line and 29 linecode declarations, with empty load and
transformer files. The parent `test/data/Master.dss` supplies the B1726, 11 kV,
-30-degree ideal source, also includes many LV networks, and finally applies
`batchedit load..* kw=1`. It is not a standalone three-wire MV benchmark. Prepare
a reproducible MV-only entry point and explicitly specified load scenario using
that source boundary; preserve the original combined master and document every
generated scenario change. Confirm conductor counts through semantic inventory.

Selection does not establish solver support. Keep the initial support inventory
as an acceptance gate and verify that conversion retains every active electrical
term. Preserve each term or classify the case unsupported instead of trimming it
to pass; report conversion/schema gaps explicitly before promoting a fixture.

The oracle protocol pins frequency, Normal algorithm, snapshot mode, controls
off, fixed taps, source impedance/phasors, grounding, and antifloat settings.
Inspect load model, Vminpu, Vmaxpu, and Vlowpu behavior and disable voltage-dependent
fallback over the tested operating range for strict constant-power comparisons;
otherwise classify the model mismatch explicitly. Assert reference convergence,
record iteration limits/tolerances, and compare exact expected terminal identities
with no hidden phase rotation. Preserve original and generated files plus settings.

Use analytical one-phase cases; coupled unbalanced three-wire and four-wire
feeders; explicit neutral displacement; WYE/DELTA loads; all supported transformer
connections; fixed tap changes; finite neutral grounding; meshed topology; and
terminal/bus permutation invariance. Test unresolved linecodes, malformed matrix
sizes, unsupported devices, conflicting sources, floating islands, zero branch
voltages, singular systems, and deterministic nonconvergence reporting.
Require convergence flags from both Tellegen and the reference, and exact,
nonempty expected node sets before comparing arrays. BMOPF's `_ods_volts` reads
circuit arrays; reading numbers alone is not proof of reference convergence.
The existing WASM `micro_bmopf.json` references undefined linecode `lc1`; preserve
it as an ingest test and create electrically complete PF fixtures separately.

Extend `.github/workflows/gates-rust.yml` shipping feature coverage and
`.github/workflows/gates-js.yml` actual WASM/browser solves. Native adapter tests
alone do not exercise WASM sparse-complex kernels. Retain the existing balanced
solver, malformed-ingest, typed-module round-trip, formatting, and Clippy gates.
Use three CI tiers: frozen small fixtures on every change; a required dedicated
live small-oracle gate with pinned runtime; and a release/nightly broader corpus
gate. Missing oracle dependencies fail their required tier rather than silently
skipping tests. Record input/output hashes, generator/tool versions, commands,
settings, source provenance and fixture licenses in each evidence manifest.

Corpus selection requires no further user input or re-upload. Use the supplied
r4176 Version8 source as the planned reference baseline and record the exact
numerical oracle runtime/revision and its relationship to that source in oracle
metadata. Reader/source inspection, the support inventory, and export planning
can proceed against the selected local corpora without another confirmation.
This document authorizes no solver coding by itself.
