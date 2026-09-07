# Multiconductor input semantic audit

Inspected read-only on 2026-09-07: PowerIO tag `v0.11.0`
`cf204b69d7df32db5cf4b3e4ff711384f7586b93`, specifically
`powerio-dist/src/{model.rs,bmopf/read.rs,bmopf/write.rs,dss/read.rs}` and the
vendored `tests/data/dist/bmopf/bmopf-0.2.0.schema.json`; BMOPFTools HEAD
`8ca84ab12c0c91aaa8ad4c9986d6adbeb969ea0b` (clean).
This audit records interpretation requirements, not solver validation.

## Required input profiles

BMOPFTools' current calculation semantics and the PowerIO-tagged BMOPF schema
conflict for legacy transformer core losses and neutral defaults. A solver must
record its selected interpretation in preparation diagnostics and outputs.
Recommended raw-input profile is explicitly named `bmopftools_8ca84ab`, matching
the user's selected producer. Do not infer that profile merely from a schema
version: raw PowerIO emission of the same version has different legacy core-loss
conventions. An API setting or input marker is more reliable than heuristics.
`_meta.powerio_source`, `powerio_source_mapping`, and `powerio_source_semantics`
are provenance evidence; persisted files carry these under `meta.provenance`.
They do not alone prove a particular BMOPFTools commit or successful conversion.

For this producer profile:

- Conventional single_phase/wye_delta/delta_wye legacy `g_no_load+j*b_no_load`
  is total admittance across winding 2 coils, divided by coil count. n_winding
  legacy g/b is per coil on winding 2. BMOPFTools `to_ybus.jl` implements this.
- `from_dss.jl:1020` normalizes conventional g/b using PMD winding-1 power and
  winding-2 voltage; susceptance is negative for inductive excitation. It skips
  already materialized explicit shunts. Failure to get PMD only warns and leaves
  legacy data unchanged: provenance must record this failure.
- BMOPFTools `_xfmr_yn` stamps no neutral branch for absent or both-zero r/x;
  nonzero r+jx gives 1/(r+jx). Perfect grounding is in bus flags.
- Modern explicit `no_load_shunt={winding,g,b}` is per coil at operating tap.
  `parse_bmopf.jl:396` materializes it as a top-level fixed shunt, retaining
  ownership in `_meta.explicit_transformer_core_shunts`. These passive shunts
  must be accepted to support producer core losses faithfully.

By contrast the tagged schema defines conventional legacy g/b on winding 1,
n_winding g/b referenced to winding 1, and absent/zero conventional neutral
impedance as solid grounding. Typed canonical inputs should obey their documented
PowerIO voltage/rating semantics; explicit core placement removes ambiguity.
DSS-origin canonical inputs carry `%noloadloss`/`%imag`, which require DSS
winding-2 conversion, and negative rneut means floating. Never change all inputs
to a DSS default merely because the algorithm follows OpenDSS.

## Reader normalization and losses

- Lines: linecode matrices are per metre; inline line matrices are absolute.
  v0.11 correctly synthesizes inline length=1, ignoring descriptive raw length.
  Missing matrix entries become zero, one triangular entry mirrors to its
  counterpart, smaller matrices pad with a diagnostic. Reject contradictory
  nonsymmetric matrices, malformed entries and unresolved references upstream.
- Sources: BMOPF schema sources prescribe ideal per-terminal-to-ground volts and
  radians. DSS vsource reader retains unused impedance parameters in `extras`
  but canonical voltages represent internal EMF and neutral is appended zero.
  Do not silently treat those finite-source parameters as ideal. A finite-source
  profile must reconstruct declared sequence Z or reject unsupported parameter
  combinations. BMOPF-vs-DSS oracle cases must share actual source physics.
- Loads: p/q are W/var per branch. Current BMOPFTools and the DSS canonical
  reader use branch v_nom (WYE LN, DELTA LL); the tagged BMOPF schema text
  instead says LL for three-phase configurations. This discrepancy affects
  nominal-admittance preparation even for CP and belongs in profile diagnostics.
  Unknown `model`
  strings silently become ConstantPower. Raw validation is required. ZIP or
  exponential fields select those models even if `model` says POWER.
- Conventional three-phase transformer v_nom/from/to and canonical v_ref are
  line-to-line. n_winding raw WYE v_nom is phase-to-neutral coil voltage, and
  the reader multiplies by sqrt(3) for canonical polyphase v_ref; DELTA stays
  line-to-line. n_winding resistance base is n_ph*v_coil^2/S.
- n_winding reader discards nested taps, neutral impedances, and unknown nested
  fields rather than retaining extras. Its per-winding physical resistance and
  rating fields are read, while unsupported metadata must still be rejected by
  preflight. Two-winding n_winding records are supported by the MC primitive;
  records with more than two windings remain rejected. Delta rolls are retained
  in `extras.bmopf_delta_rolls` and drive each DELTA winding's coil rotation.
- Conventional tap and neutral arrays collapse to first entry. Raw scalar
  validation must prevent silently losing per-phase values.
- Unknown transformer subtype becomes single-phase with diagnostic; reject it.
  Both lumped and split impedance fields require explicit precedence validation.
- Transformer extensions under extras.transformer are merged before parsing,
  with primary object fields winning. Raw preflight must inspect this overlay.
- terminal_conventions survives as network extras.bmopf_terminal_conventions.
  Labels are exact/case-sensitive. Absent declarations infer n/N as neutral,
  and 4 only for a bus whose terminal set is exactly 1,2,3,4. Ground g is implicit.

## Reference caveats

The v0.11 DSS-to-BMOPF writer converts `%imag` with positive sign on the from
voltage base; current BMOPFTools from_dss corrects sign and side as described
above. Direct Rust BMOPF export is therefore not interchangeable with current
BMOPFTools output for transformer core losses. The writer also drops negative
neutral resistance, so floating defaults cannot be recovered from raw exported
fields alone. Preserve original DSS/canonical provenance alongside oracle data.

The repository-local Julia test environment pins an incompatible PowerIO.jl API.
An isolated `/tmp/tellegen-bmopf-export` environment subsequently generated
`/tmp/tellegen-dy.json` with current BMOPFTools and PowerIO.jl 0.11 main. The
inspected Dy result materializes its core as `shunt.powerio_core_delta_wye_t1`,
with per-coil g=0.008709536942952532 S and b=-0.04354768471476266 S on winding 2,
plus an explicit 1/0.3 S neutral grounding shunt. No legacy g/b remain on its
transformer. Therefore explicit passive shunt support eliminates the core-loss
profile ambiguity for this actual producer output. Generation alone is not
numerical comparison evidence; retain exact Julia/Rust revisions in provenance.
