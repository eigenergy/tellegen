---
name: SDP relaxation of AC OPF
about: Tracked feature. Add the semidefinite relaxation.
title: 'Add the SDP (semidefinite) relaxation of AC OPF'
labels: enhancement
---

**Summary.** The engine solves the SOCWR (Jabr) second order cone relaxation. Add the tighter SDP relaxation (PowerModels `SDPWRMPowerModel`): same W space, one global positive semidefinite constraint replacing the per branch Jabr cones. Convex, solved by Clarabel, a strictly tighter lower bound on AC OPF. Purely additive.

**References.**
- Relax `W = V V*` to `W ⪰ 0` (`W_ii = |V_i|^2`, `W_ij = wr_ij + i wi_ij`). Real form: `[WR WI; -WI WR] ⪰ 0`.
- PowerModels `src/form/wrm.jl`: `@constraint(pm.model, [WR WI; -WI WR] in PSDCone())`. `SDPWRMPowerModel` is dense; `SparseSDPWRMPowerModel` is the chordal (scalable) form.
- Clarabel cone: `SupportedConeT::PSDTriangleConeT(2n)` (triangle vectorization of the `2n x 2n` real block; note its off diagonal `sqrt(2)` scaling).
- Ordering: `socwr <= sdp <= ac_opf`. SDP is the tighter bound.

**Mirror.** `problem/conic.rs` (`SocWrLayout`, `assemble_conic_opf`, `socwr_opf`, `SocWrSolution`); `sens/conic.rs` (`ConicKkt`); `formulation.rs`; `api.rs` (`Problem`, `solve_network`, `socwr_solved` / `socwr_assemble`, `capabilities_json`); `study.rs` (`ConicState`); `lib.rs`; `apps/web/src/lib/wasm.ts` (`FORMULATIONS`).

**Phase A: forward bound (moderate).**
1. Add `Problem::Sdp` (tag `"sdp"`) and an `Sdp` formulation behind `conic`.
2. New `problem/sdp.rs`: same balance, Ohm, bound, and angle rows as SOCWR, but add the full off diagonal `wr_ij, wi_ij` for every pair `i<j` (dense `W`), and replace the per branch `SecondOrderConeT(4)` Jabr cones with one `PSDTriangleConeT(2n)` for `[WR WI; -WI WR] ⪰ 0`. Keep the apparent power `SecondOrderConeT(3)` cones.
3. `sdp_opf(net) -> Result<SdpSolution, String>` plus `SdpSolution` (mirror `SocWrSolution`). Wire `sdp_solved` / `sdp_assemble`, the `ConicState` path, the `lib.rs` export, and the capabilities entry.
4. Cap: dense SDP is O(n^2) variables and a `2n` cone, so solve only for `n <= SDP_MAX_BUS` (start at 60) and return `Err("SDP is limited to <=N buses in this build")` above it. Stretch: chordal decomposition per `SparseSDPWRMPowerModel`.

Acceptance: on `case9`, `sdp_opf` solves, `W ⪰ 0` (min eigenvalue `>= -1e-7`), objective `>= socwr_opf` and `<=` the published `AC ($/h)`; the cap errors cleanly. Green: `cargo test -p tellegen --features conic`, and `cargo build -p tellegen-wasm --target wasm32-unknown-unknown --features conic` (confirm Clarabel's PSD cone compiles to wasm).

**Phase B: differentiable (the hard part, can land later).** Extend `ConicKkt` to the PSD cone. The PSD complementarity `X Z = 0` linearizes with the symmetric cone (Nesterov and Todd) scaling, not the SOC arrow form. Reference: Agrawal et al., "Differentiating through a cone program" (the PSD derivative); Clarabel's `PSDTriangleCone` KKT. Leave the SOC, DC, and AC Newton impls untouched.

Acceptance: finite difference parity of the SDP sensitivity columns in `crates/benchmarks/src/parity.rs` (alongside `conic_parity`) within the current tolerance on `case9`; `Study::preview` and `commit` work under `sdp`.

**Phase C: UI and docs.** Add `{ id: 'sdp', label: 'SDP', hint: 'semidefinite relaxation of AC OPF (small cases)' }`; gate it to the cap; if Phase B is undone, route `sdp` commit only (no sensitivity overlay). Add an SDP subsection to `docs/src/formulations.md` and a line in `validation.md`.

**Do not break.** DC, AC power flow, and SOCWR and their sensitivities stay identical.

**Verify.** `cargo test --workspace`; `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo deny check`; `cargo build -p tellegen-wasm --target wasm32-unknown-unknown --features conic`; `cd apps/web && npm run check && npm run build && npm run smoke:build`.

**Staging.** Land Phase A first and verify the tighter bound. Phase B is independent. Do not block A on it.
