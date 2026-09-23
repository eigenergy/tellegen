# Python package and local MCP server — design

**Date:** 2026-09-17
**Status:** for review (revision 2)
**Scope:** ship `pip install tellegen` as a Python library over the Rust engine, and
settle how it relates to the Tellegen MCP server already open as PowerMCP #67.

Written for a reviewer who knows the repository but not the discussion behind it. Every
claim about current code carries a `file:line` citation and was verified against the
working tree at `3130e92` and against the live PR branches, not from memory.

> **Revision 2 changed the shape of this plan.** Revision 1 proposed building an MCP
> server inside the wheel using the WebMCP browser tool vocabulary. That was written
> without knowing PowerMCP #67 already implements a Tellegen MCP server over the native
> CLI — which is the approach revision 1 had rejected. #67 is now kept as the MCP
> surface and this plan narrows to the Python library. Revision 1's §3.3 and Q8 were
> factually wrong and are corrected in §3.3. §7 is replaced. Blocker B is now verified
> by compilation rather than assumed.

---

## 1. Decisions taken

| Decision | Choice |
| --- | --- |
| Binding strategy | PyO3/maturin native extension over the `tellegen` crate — a new `crates/tellegen-py` cdylib, **not** a subprocess wrapper |
| Layering | Thin JSON-in/JSON-out PyO3 seam → Pythonic API. (Revision 1's third layer, an MCP server, is dropped — see below.) |
| v1 capabilities | DC OPF + LMP sensitivities, SOCWR, balanced AC power flow. **Excluded:** multiconductor PF, persistent Studies, capacity planning |
| **MCP surface** | **PowerMCP #67 keeps it, with its CLI tool vocabulary.** The wheel ships no MCP server in v1 |
| Convergence | Once the wheel is on PyPI, #67's subprocess transport can be replaced in place, keeping its tool names — see §7.3 |
| v1 definition of done | Release-*ready*, not released: wheels build and smoke-test on all platforms, publish job wired to TestPyPI only |

The rejected alternatives were never hypothetical, and the plan owes that an explicit
note. `docs/src/studies.md:87-92` published the CLI-vocabulary PowerMCP contract on
**2026-09-07** (commit `df8b3cc`, samtalki), nine days before #67 implemented it. A
reviewer weighing "in-process vs subprocess" is weighing a design that is already
specified and built, not one that was merely considered.

---

## 2. Goals and non-goals

**Goals**

- `import tellegen` gives a researcher a solver in a notebook: load a case, solve, edit
  demand or ratings, read LMP sensitivity columns. **This is the half of the original
  ask that nothing currently delivers.**
- The wheel is structured so that #67 can later call it in-process instead of spawning a
  binary, removing #67's hardest prerequisite (§7.2).

**Non-goals for v1**

- An MCP server in the wheel. #67 owns that surface.
- Multiconductor / distribution power flow (`mc-pf`) — excluded from the compiled feature
  set, not merely hidden.
- Persistent Studies and capacity planning. #67 already exposes both over the CLI; the
  wheel need not race it.
- Publishing to PyPI. Wiring is built and pointed at TestPyPI.
- Progress streaming for the **solve path**. Note the narrower claim than revision 1
  made: `study_ops::execute_study` (`crates/tellegen/src/study_ops.rs:278`) *does* take a
  public checkpoint-and-cancel closure, and the CLI already emits
  `{"event":"study_checkpoint","index":n}` on stderr under `--progress`. It is the solve
  path and the planning search that have no hook.

---

## 3. What exists today

**3.1 The FFI seam exists and is proven twice over.** Every function the CLI and the wasm
adapter call is `pub` in the engine and speaks JSON strings: `solve_module_json`
(`api.rs:465`), `capabilities_json` (`api.rs:965`), plus the retained `Study` whose
methods take JSON for edits and sensitivities (`study.rs:812-1005`).
`crates/tellegen-wasm/src/lib.rs` is a near-mechanical wrapper of exactly this surface,
and CI runs its bodies natively (`cargo test -p tellegen-wasm --features conic`) — the
existing proof that this surface works headlessly. The Python binding is the same wrapper
with `PyErr` where `JsError` is.

This is forced, not merely convenient. Many engine enums are `#[non_exhaustive]`
(`NetworkEdit`, `Operand`, `Parameter`, `SensitivityMatrix`, `ElementId`, `Mode`), so an
out-of-crate binding cannot construct them with struct literals even from inside the same
workspace; and `SolveRequest` / `Edits` / `SensRequest` derive `Deserialize` only
(`api.rs:157-215`). Serde-from-JSON is the only available seam.

**3.2 `powerio` is a complete template.** The sibling repo ships a maturin mixed-layout
wheel: `powerio-py` cdylib with `pyo3 0.29` + abi3, `extension-module` as an opt-in
feature so `cargo test` never links libpython, a pure-Python tree under
`python/powerio/`, hand-written `.pyi` stubs gated by `mypy.stubtest`, a five-leg wheel
matrix, and PyPI trusted publishing. Copy the skeleton.

The local checkout is 216 commits behind at 0.9.0, whose Python API was removed in 0.11.
Read templates from the tag: `git show v0.11.1:<path>`.

**3.3 The Tellegen MCP server exists — as PowerMCP #67.** *(Revision 1 asserted the
opposite. That was wrong.)*

`powermcp/tellegen.py`, 525 lines, spawns the native `tellegen` CLI once per tool call and
exchanges PowerIO IR only. 11 tools in the CLI vocabulary: `capabilities`, `contract`,
`solve`, `solve_module`, `plan`, and a six-tool Study family. Open, **draft**, mergeable,
both CI legs green (py3.10 and py3.12), and **zero human review** — no reviews, no inline
comments, no issue comments; pushed once on 2026-09-16 and untouched since. Of its
+3286/−1344 against `main`, only about **1212 lines across 13 files** are
tellegen-specific; the rest is base drift.

It is genuinely good work. Three things in it are better than what revision 1 proposed:

- **`apply` refusal.** `OPERATIONS` is an eight-kind frozenset that simply omits `apply`;
  `study_run` raises before spawning anything. This answers revision 1's hardest deferred
  question — how a headless server obtains human approval — by declining to have a tool at
  all. Nothing to talk an agent through.
- **Element-based bounding.** `_bounded()` turns any array over `DEFAULT_MAX_ELEMENTS`
  into `{"truncated": true, "count": n, "head": [...]}`, which is a better caller
  experience than WebMCP's flat `OUTPUT_TOO_LARGE` refusal.
- **`_module_ir()`** — exactly-one-of validation, path containment, parse-or-deserialize,
  diagnostics gating before *and* after collection selection, multiconductor refusal
  naming `to_balanced` as the remedy. Transport-agnostic; see §6.3.

And `docs/src/studies.md:87-92` is not incorrect prose to fix — it is the spec #67
implements, down to the apply refusal.

### 3.4 The two blockers, re-scoped

**Blocker A — the published crate — is one merge away, and smaller than revision 1 said.**
crates.io `tellegen` 0.2.0 depends on `powerio ^0.9` plus the retired `powerio-pkg`, and
HEAD is 125 commits ahead on powerio 0.11.1. But tellegen **PR #103** (`release-plz-main`)
is open and already sets `crates/tellegen/Cargo.toml` to `version = "0.3.0"` with a
2026-09-11 changelog; its body instructs a merge commit, not a squash. Separately, **a
maturin wheel built from a path dependency publishes to PyPI without crates.io
involvement at all** — only the *sdist* carries the workspace. So 0.3 gates the PowerMCP
pip extra and the sdist decision (§12 risk 3), not the wheel.

**Blocker B — `Study` is `!Send` — is fixed by one line, now verified by compilation.**
`Study` holds `solved: Box<dyn SolvedState>` (`study.rs:377`) and `SolvedState`
(`study.rs:142`) declares no `Send`/`Sync` supertrait. The trait is **private** with
exactly three implementors — `DcState`:193, `AcPfState`:236, `ConicState`:263, each
holding only a network and a solution — so adding the bound is not a public API change and
`cargo-semver-checks` is unaffected.

> **Verified, not assumed.** `trait SolvedState: Send + Sync` plus a
> `assert_send_sync::<Study>()` probe compiles clean:
> `cargo check -p tellegen --features conic --all-targets` finished in 36s with no errors.
> Revision 1 listed this as the one unverified assumption in the critical path; it is
> now closed, and the `#[pyclass(unsendable)]` fallback is not needed. GIL release via
> `py.allow_threads` is available.

---

## 4. Architecture

```
┌───────────────────────────────────────────────────────────┐
│ python/tellegen/         Pythonic API (zero runtime deps)  │  tellegen
│   load() · Case.solve/update/preview/sensitivity/reset    │
│   dense-index translation · units · typed exceptions      │
├───────────────────────────────────────────────────────────┤
│ crates/tellegen-py/      PyO3 seam  →  tellegen._tellegen │
│   JSON in / JSON out · mirrors tellegen-wasm 1:1          │
├───────────────────────────────────────────────────────────┤
│ crates/tellegen/         engine (unchanged except §9)      │
└───────────────────────────────────────────────────────────┘

   PowerMCP #67  ──spawns──►  tellegen CLI  ──►  same engine
   (the MCP surface; §7)         later: calls the wheel in-process
```

The library enforces only the engine's invariants — demand ≥ 0, ratings > 0
(`study.rs:1342`) — and stops there. Slider-style bounds are a *host* policy that belongs
in an MCP layer, not in a programmatic API where clamping would surprise a caller.

---

## 5. Layer 1 — `crates/tellegen-py` (the PyO3 seam)

A mechanical mirror of `crates/tellegen-wasm/src/lib.rs` minus the wasm glue.

| Export | Engine call |
| --- | --- |
| `solve_module(module_json, request_json) -> str` | `tellegen::solve_module_json` |
| `capabilities_json() -> str` | `tellegen::capabilities_json` |
| `parse_case(bytes, format) -> str` | `powerio::parse` → `ir::balanced_module` → `ir::serialize_module` + a summary |
| `classify_json(bytes) -> str` | `powerio::classify_json_bytes` |
| `resolve_format(token) -> str \| None` | `powerio::resolve_format` |

`parse_case` is why the wheel is self-contained: case parsing is not in tellegen (`ir.rs`
only moves IR text; `model::parse_matpower` is crate-private), it is in `powerio`, whose
Rust crates this extension already links. Binding them directly keeps the base package
dependency-free. (#67 demonstrates the alternative working — parsing in Python via
`powerio.parse`/`deserialize`/`serialize` — so both are viable; see Q6.)

For v1, `parse_case` returns only `{name, base_mva, n_bus, n_branch, n_gen, load_mw,
module_json}`. The rich ingest payload in `tellegen-wasm` (topology, coordinates, map
view) is built by `pub(crate)` helpers there; relocating them into the engine so both
hosts share them is **deferred** — needed only if the Python package ever feeds a viewer.

**The retained handle: `_tellegen.Case`**, a `#[pyclass]` wrapping `tellegen::Study`.
Named `Case` deliberately: `Study` is reserved for the persistent-document layer, and the
two are different things.

```
Case(module_json, formulation)          Study::new
  .replace_edits(edits_json, sens_json) Study::replace_edits_with   → absolute edit set
  .commit(edits_json, sens_json)        Study::commit_with          → append to the log
  .preview_replacement(edits, watched)  Study::preview_replacement  → first-order, no solve
  .preview(edits, watched)              Study::preview
  .solution() / .formulation() / .fork()
  .save_module() / .save_instance_module() / .save_solution_module()
  .export(format)                       Study::export
```

`fork()`, `preview_replacement()` and `solve_instance_cancellable` are the three things
**no subprocess transport can reach** — see §7.1. They are the reason the library is worth
building rather than shelling out.

**Errors.** Mirror powerio's hierarchy with `.code` on the instance:

```
TellegenError(ValueError)          .code: str
├── TellegenInputError             malformed request, unknown element, out-of-range edit
└── TellegenSolveError             infeasible, unbounded, not converged, cancelled
```

The engine returns free-form `Result<_, String>`, so one `classify(msg) -> &'static str`
table in Rust maps the documented stable substrings onto `SOLVE.INFEASIBLE`,
`REQUEST.MALFORMED`, `EDIT.UNKNOWN_ELEMENT`, `EDIT.OUT_OF_RANGE`, `SENS.UNSUPPORTED`. See
§12 risk 4 and §9.2.

Panics: the engine has non-test `unwrap`/`unreachable!` sites on internal invariants and
no `catch_unwind`. Wrap every public callable in a `_guard` decorator converting
`pyo3_runtime.PanicException` — a `BaseException` that escapes `except Exception` — into
`TellegenError(code="BIND.PY.PANIC")`, which is why powerio added
`python/powerio/_guard.py`. Mark a `Case` that panicked mid-solve dead.

---

## 6. Layer 2 — `python/tellegen/` (the Pythonic API)

Zero runtime dependencies. `numpy`/`scipy` behind a `matrix` extra, imported lazily.

```python
import tellegen

case = tellegen.load("case30.m")
case = tellegen.load_ir(module_json)
case.summary

sol = case.solve()
sol.objective, sol.lmp, sol.flows, sol.dispatch

sol  = case.update(demand={2: 50.0})      # absolute deltas from base, exact re-solve
pred = case.preview(demand={2: 60.0})     # first-order, no re-solve
col  = case.sensitivity(bus=5)            # ∂LMP/∂demand column at bus 5
case.reset()

case.formulation = "socwr"
tellegen.capabilities(); tellegen.versions()
```

**6.1 Absolute vs append.** `update()` maps to `replace_edits` (absolute), matching what
the browser protocol uses exclusively; `append()` maps to `commit`.

**6.2 Dense-index translation.** `SensRequest.indices` are dense 0-based indices over the
in-service reindexed axis, while edit keys and response `ElementId`s are source ids or
uids, with no public helper between them (`api.rs:1895` pins this: "sens_bus 2 is dense
index 1"). The browser translates from `lmp[]`/`flows[]` order at
`packages/engine/src/index.ts:1069-1089`. **The most error-prone piece of the port; it
gets dedicated parity tests (§10.3).**

**6.3 Reuse #67's module resolution rather than reinventing it.**
`powermcp/tellegen.py`'s `_module_ir()` (~55 lines) and its `_module_summary()` /
`_over_input()` / `_counts()` output shape are entirely transport-agnostic and already
tested: exactly-one-of validation, diagnostics gating before and after collection
selection, `OperatingPoint` unwrapping, an accepted-value-type whitelist, per-severity
diagnostic counts. Port these into `tellegen.load`/`load_ir` rather than writing them
again. `_json_argument()` likewise.

**6.4 Units and identity, documented on every accessor.** Angles are **radians** in
`SolveResponse` but **degrees** in the portable `powerio::DcOpfSolution` (`emit.rs:75`).
`lmp` is `objective_unit/MW` and present *only* when the instance objective is
`network_generator_cost`. `loading` is dimensionless. Edit keys may be integers or uid
strings, and **aliases for the same element are summed** before bounds are checked.

**6.5 Per-formulation capability matrix,** surfaced rather than failing late: `acpf` has no
LMP, no sensitivities, no preview; `socwr` needs `conic`; rating edits are rejected by
`dcpf` and `acpf`; `acopf` is wire-stable but permanently unavailable.

Typing: hand-written `.pyi`, empty `py.typed`, `mypy.stubtest` in CI with an allowlist,
run from an empty cwd so the repo tree does not shadow the installed package.

---

## 7. The MCP surface — PowerMCP #67

#67 keeps this surface. This section records what that means for the wheel.

### 7.1 What the CLI transport cannot do

Three capabilities have **no CLI surface at any price**, which bounds what #67 can ever
offer and is the strongest technical argument for the library:

- **`Study::fork()`** — no CLI command. Fork-isolated sensitivity analysis is unbuildable
  over a subprocess.
- **`Study::preview_replacement()`** — no CLI command. First-order preview without a
  re-solve cannot be exposed.
- **Graceful solve cancellation.** `crates/tellegen-cli/Cargo.toml:23` enables ctrlc's
  `termination` feature, but `ctrlc::set_handler` is called **only inside the `study run`
  arm** (`crates/tellegen-cli/src/main.rs:162`, within `study_command()`). Verified: it is
  the sole `set_handler` call in the file. So `solve`, `solve-module` and `plan` have no
  handler, and #67's SIGTERM-then-grace-then-kill path is a plain kill for them.
  `solve_instance_cancellable` (`api.rs:545`) is the only real solve-side hook and it is
  reachable **only in-process**.

### 7.2 Defects in #67 worth fixing, with or without the wheel

Each verified directly, not relayed:

1. **Cancellation is over-documented.** `powermcp/TELLEGEN.md` and the PR body describe
   the SIGTERM grace period as letting "the current exact trial finish and its evidence be
   saved." True for `study_run`; false for `solve`, `solve_module` and `plan`, which have
   no signal handler (§7.1). The prose needs to scope the claim.
2. **The solve request travels on argv, unbounded.** `solve` does
   `_call([json.dumps(request)], raw_stdin=ir_text)` while `_call` length-checks only
   stdin (`len(data) > MAX_BUNDLE_BYTES`). A large `sensitivities` list hits `ARG_MAX`
   (~256 KiB on macOS) as an opaque `OSError`. The CLI's own tests note that requests go
   on stdin *because* of the Windows 32,767-character argv limit.
3. **The empty extra misleads.** `pyproject.toml` has `tellegen = []  # The native CLI
   installs separately.`, and `opensource = ["powermcp[tellegen]", …]`. So
   `pip install powermcp[opensource]` advertises tellegen support it cannot provide.
4. **`probe="mcp"` starts a server with no engine.** `powermcp/runner.py`'s preflight does
   `find_spec(probe)`, so `powermcp run tellegen` succeeds on a machine with no `tellegen`
   binary and fails only mid-conversation on the first tool call.
5. **Coverage debt.** `study_export` and `study_import` appear in
   `tests/test_tellegen_server.py` only in the two tool-name set assertions;
   `study_export`'s concurrent-mutation check and sha256, `study_import`'s 512 MiB guard,
   and every non-summary branch of `study_inspect` including the 8192-character pager are
   unexercised. `tests/data/fake_tellegen.py` implements export but not import, and
   `tests/test_doctor.py` contains no occurrence of "tellegen".
6. **It does not follow the error convention its own stack introduces.** See §7.4.

### 7.3 The convergence path

Everything subprocess-specific in #67 lives inside one function, `_call()`. Tool names,
signatures, docstrings, sandbox chokepoints, bounding and all of its tests sit above it.
Replacing `_call()` is the whole integration once the wheel exists.

Recommended end state is **split by capability, not wholesale replacement**: in-process
for the fast stateless calls (`capabilities`, `contract`, `solve`, `solve_module`), where
in-process also buys the only real solve cancellation; subprocess retained for `study_run`
and `plan`, where process isolation matters because the engine has non-test panic sites
and no `catch_unwind`, so an in-process panic would take the MCP server down with it.

### 7.4 Error convention — a decided carve-out, no longer an open question

PR #70 adds `powermcp/errors.py` exposing `run_tool`, `tool_error`, `tool_success`, and
documents `{"status": "success"|"error", "message": …}` in the top-level README as the
shape for **every** tool in the repository. Verified: `powermcp/tellegen.py` on #70's own
branch contains no reference to `run_tool`, `tool_error` or `tool_success` — so the claim
is already false for a server inside #70's own stack.

**This is not a style question. Measured against a live server, the raise style as
written silently destroys the message.** In the mcp 2.x SDK,
`mcp/server/mcpserver/tools/base.py:205-210` handles a tool body's exception in two ways:
a `ToolError` becomes `is_error=True` *with your message* in `content`, logged at INFO;
**anything else** — including `ValueError` — becomes `UnexpectedToolError` whose message
is replaced by the generic `Error executing tool <name>`, with a traceback logged at
ERROR. The SDK's own docstring says so: "the model sees only `Error executing tool
<name>`."

Demonstrated end to end against #67 (see the transcript in §15): calling `study_run` with
`operation.kind = "apply"` returns

```
is_error: True
content:  [{"type": "text", "text": "Error executing tool study_run"}]
```

The refusal *works* — nothing is applied — but the remedy #67 carefully wrote, "Apply the
reviewed proposal through an explicit native CLI user action", never reaches the agent,
which therefore cannot learn from it and will plausibly retry. Every deliberate refusal in
#67 has this shape, and each logs a server-side traceback for expected behaviour.

The scope is wider than #67: `powerio/mcp/server.py` in the published 0.11.3 wheel has
**11 `raise ValueError` sites and no `ToolError` usage at all**, so powerio's carefully
coded `f"{code}: {message}"` messages are discarded the same way. #70's returned-dict
shape sidesteps the problem entirely, because a returned dict is a successful result
carrying an error payload.

Corrected recommendation, superseding revision 2's first draft of this section: a
wheel-resident server should raise **`ToolError(f"{code}: {message}")`**, not
`ValueError`. That keeps the coded-message style *and* delivers it. Returning #70's dict
is the other correct option; raising `ValueError` is simply wrong under mcp 2.x.

### 7.5 The binary-distribution gap

#67's hardest prerequisite is that the user must supply a `tellegen` binary. Verified:
`tellegen-cli` is `publish = false` (`crates/tellegen-cli/Cargo.toml:8`) and appears in
`.github/workflows/` only as `cargo build -p tellegen-cli --features conic` in
`gates-js.yml:41` and in the clippy lists. There is **no** artifact upload, no
`gh release upload`, no binary release workflow anywhere. So `cargo install` is impossible
and no release artifact exists: the only path is cloning the repo and building, which
#67's own error message states outright.

A release workflow attaching CLI binaries to the GitHub release would fix this for every
#67 user and is independent of the wheel. It belongs in the tellegen repo. See §14.

---

## 8. Packaging, repo layout, CI

### 8.1 Layout

```
pyproject.toml                      maturin backend, dynamic version
mypy.ini  ruff.toml
crates/tellegen-py/{Cargo.toml,src/lib.rs}
python/tellegen/{__init__.py,__init__.pyi,_tellegen.pyi,py.typed}
python/tests/
python/stubtest_allowlist.txt
```

### 8.2 Manifest details that are load-bearing

Each verified; getting one wrong breaks an existing gate.

- `crates/tellegen-py` in `members` but **not** `default-members`, so bare `cargo build`
  and the wasm gate never link libpython.
- `tellegen = { path = "../tellegen", default-features = false, features = ["conic"] }`
  **with no `version` field**. `validate-version-changes.py:94-110` requires the
  release-plz diff to touch *exactly* `{Cargo.lock, crates/tellegen/CHANGELOG.md,
  crates/tellegen/Cargo.toml}` and fails closed otherwise; a `version` field would make
  release-plz rewrite this manifest too.
- `publish = false` in `Cargo.toml` **and** a `release-plz.toml` `[[package]]` stanza with
  `release = false, publish = false`, or release-plz refuses to run.
- `[features] extension-module = ["pyo3/extension-module"]` opt-in, so
  `cargo test --workspace` does not link libpython.
- `pyo3 = { version = "0.29", features = ["abi3-py39"] }`. **Do not enable
  `multiple-pymethods`** — it is incompatible with abi3.
- `features = ["conic"]` with `default-features = false`; `conic` implies `sensitivity`,
  giving dcpf/dcopf/acpf plus KKT sensitivities. Omitting `mc-pf` keeps the compiled
  surface honest about v1 scope; adding it back is one word.
- `[tool.maturin]`: `manifest-path`, `module-name = "tellegen._tellegen"`,
  `python-source = "python"`, `features = ["extension-module"]`, `strip = true`,
  `include = [{ path = "LICENSE", format = "sdist" }]` (PyPI rejected powerio's sdist
  without it).
- A `[profile.wheel]` inheriting release at `opt-level = 3, lto = "thin"`, selected with
  `maturin build --profile wheel`. The workspace root sets `opt-level = "s", lto = true`
  for **wasm size**, and native throughput at `"s"` versus `3` has never been benchmarked.

**Version sourcing.** The workspace has no `[workspace.package] version` (verified), and
release-plz will not bump a `release = false` crate, so `crates/tellegen-py` needs its own.
Recommend tracking the engine version so `pip install tellegen==X` and
`cargo add tellegen@X` mean the same engine, at the cost of one allowlist edit (Q2).

### 8.3 CI

- **`gates-rust.yml`**: add `-p tellegen-py --features extension-module` to the clippy
  step — it names crates in an explicit hardcoded list (`gates-rust.yml:34`, mirrored in
  `justfile:28` and `powerio-candidate.yml:56`), so a new crate is silently unlinted
  otherwise. Add `check_no_pounce tellegen-py -p tellegen-py` to `scripts/epl-guard.sh`.
  cargo-deny picks up pyo3's tree automatically; verify no SPDX identifier outside
  `deny.toml`'s allowlist.
- **New `gates-python.yml`** (`workflow_call`, from `ci.yml` beside rust and js):
  `maturin build --release --out dist`, install the **built wheel** rather than
  `maturin develop` (the repo's own `python/tellegen` shadows the installed package on
  `sys.path`; powerio documents hitting exactly this), then pytest, ruff, mypy, stubtest.
- **Wheel matrix**: `PyO3/maturin-action@v1`, `manylinux: auto`, five legs — ubuntu
  x86_64 and aarch64, macos-14 arm64 and x86_64, windows x64. Plus an sdist job and a
  `wheel` job in `package-inspect.yml`.
- **Publish**: jobs **inside `release-crate.yml`**, not `on: release:` or
  `on: push: tags:`. release-plz creates the tag and release with `github.token`, and
  events generated by a workflow's `GITHUB_TOKEN` **do not trigger other workflows** — a
  tag-triggered wheel workflow would silently never fire. v1 targets **TestPyPI** via
  `repository-url` in a new `testpypi` environment with `id-token: write`.
- Housekeeping: a `pip` Dependabot ecosystem for `/`; `.dockerignore` gains `python/`,
  `dist/`, `*.whl`, `.venv`, `__pycache__`; `.gitignore` gains `dist/`, `*.whl`,
  `python/tellegen/*.so`, `.venv/`.

---

## 9. Prerequisite engine changes

**9.1 `SolvedState: Send + Sync`** (`study.rs:142`) — **verified by compilation**, see
§3.4 Blocker B. One line; private trait; not a public API change. Enables GIL release.

**9.2 Recommended before 0.3: a typed error enum at the api edge.** `SensError` is already
typed internally (`sens/contract.rs:351`) then stringified at `api.rs:912`. Substring
classification means a reworded message silently reclassifies an error, and
`cargo-semver-checks` runs on release PRs, so adding this after 0.3 costs more. Not a v1
blocker. Note this interacts with §3.4: if 0.3 is merged first to unblock the wheel, this
lands in 0.4.

---

## 10. Testing

1. **Rust seam** — unit tests for the `classify(msg) -> code` table and JSON round trips,
   kept to pure functions so they need no libpython.
2. **Python API** — pytest over the MATPOWER fixtures in `crates/tellegen/tests/data`
   (see §12 risk 3 on fixture licensing). Reuse
   `PowerMCP/tests/data/opendss/geometry_unresolved.dss`, a 9-line fixture that reliably
   produces an error-severity diagnostic.
3. **Parity with the shipped engine — the highest-value tests.** For a fixed case and
   request, assert the Python `solve_module` output matches the engine's, and assert the
   dense-index translation agrees with the browser by porting the cases behind
   `packages/engine/src/index.ts:1069-1089`.
4. **Ported from #67** — roughly 18 of its 24 assertions are about input validation and
   semantics rather than subprocess mechanics and transfer verbatim: multiconductor
   refusal before any engine work, error-severity diagnostic refusal, input-side
   diagnostics travelling with every response, collection selection, staged write plus
   overwrite refusal, path containment on every filesystem argument.
5. **Protocol knowledge from `tests/data/fake_tellegen.py`** — it encodes exactly what each
   entry point accepts (stdin must be `{"schema":"pio-ir","version":2,…}`; `plan` takes
   `{module, spec}`; `study run` takes `{expected_revision, operation}`) and is the
   reference for the seam's JSON signatures. The subprocess double itself disappears.
6. **Zero-dependency check** — `"numpy" not in sys.modules` after a solve.
7. **Per-platform wheel smoke** in a clean venv before any publish, following powerio's
   `scripts/wheel-smoke.py`.

---

## 11. Phases

> **Status.** The two commits that follow this document in its pull request land
> Phase 0.1, most of 0.2 and 0.3, and Phases 1 and 2. Each item below is marked. The
> one thing that looks done and is not is **0.2's actual gate**: wheels have been built
> and tested on macOS arm64 only. That is the premise the whole package rests on, and
> it can only be settled by CI.

**Phase 0 — prerequisites and de-risking (no Python yet)**
- 0.1 **done.** `Study` verified `Send` by compilation; `SolvedState: Send + Sync` landed.
- 0.2 **partial.** `crates/tellegen-py` exists and builds a real wheel, and
  `gates-python.yml` runs lint, build, install-the-wheel, mypy and pytest on 3.9 and
  3.13. But that gate is `ubuntu-latest` only: the **five-leg wheel matrix (ubuntu
  x86_64 + aarch64, macos-14 arm64 + x86_64, windows x64) is NOT built**, so the stated
  gate — "all five legs green before further work" — has not been met. Risk 1 is open.
- 0.3 **done.** Workspace member, release-plz stanza, a scoped cargo-deny exception for
  `target-lexicon`, an isolated clippy invocation for the extension, `.gitignore` and
  `.dockerignore`. Dependabot `pip` ecosystem still outstanding.

**Phase 1 — the seam. done.** `solve_module`, `capabilities_json`, `parse_case`,
`resolve_format`, the three-level exception hierarchy with `.code`, `_guard` over
`PanicException`, and GIL release via `Python::detach` (pyo3 0.29's name for
`allow_threads`). The error-classification table is asserted from pytest rather than
`cargo test`, because a cdylib-only pyo3 crate has no portable test harness — see the
commit for why.

**Phase 2 — the Pythonic API. mostly done.** `load`, `load_ir`, `Case`
(solve/update/preview-free sensitivity/reset/formulation), `capabilities`, `versions`,
`_tellegen.pyi`, `py.typed`, ruff and mypy green, 41 tests against the installed wheel.
**Outstanding:** `docs/src/python-package.md` and its `SUMMARY.md` entry, the README pip
line, `__init__.pyi` plus the stubtest gate, and porting `_module_ir` from #67. The
`matrix` extra currently declares numpy that nothing uses.

**Phase 3 — release-ready. not started.** Wheel smoke on all legs, sdist,
package-inspect job, the TestPyPI publish job in `release-crate.yml`, install docs.

**Definition of done for v1:** wheels build and smoke-test green on all five legs;
`import tellegen` solves a case from a clean venv on each platform; the publish job is
wired to TestPyPI only; PyPI trusted publishing documented but not enabled.

**Deferred:** an MCP server in the wheel; converging #67's `_call()` onto it (§7.3);
persistent Studies and capacity planning in the Python API; multiconductor PF; relocating
the rich ingest payload out of `tellegen-wasm`; the PowerMCP registry row for the wheel.

---

## 12. Risks

1. **Cross-platform wheels are unproven in this repo.** Every tellegen workflow job runs
   on `ubuntu-latest`; there is no macOS or Windows runner anywhere. The dependency tree
   is pure Rust — clarabel on QDLDL+amd, faer with default features off, sha2, no
   BLAS/LAPACK/C — which is *why* this should work, but aarch64-linux cross, macos-14
   x86_64 cross and windows MSVC are untested. **Mitigated by making the matrix task
   0.2.** Note the *interpreter* axis is already de-risked: PowerMCP #69 runs 3.10, 3.12,
   3.13 and 3.14 green against powerio's single cp39-abi3 wheel.
2. **Two names, one word.** `tellegen::Study` (retained solver session) and "persistent
   Study" (the document) are different things; the Python layer calls the former `Case`.
3. **sdist contents and fixture licensing.** maturin's sdist includes path dependencies,
   so it would carry the whole workspace including `crates/tellegen/tests/data/mc_pf`
   (BSD-3-Clause) and OpenDSS-derived oracle JSON. `cargo package --list` already shows
   test data in the published crate tarball. Decide before publishing any sdist — or
   publish wheels only (Q7).
4. **Substring error classification is brittle.** A reworded engine message silently
   reclassifies. Mitigated by tests asserting on the substrings; fixed by §9.2.
5. **powerio's path policy reverses under the feet of a long-lived server.** In powerio
   0.11.3, `powerio/mcp/sandbox.py` sets `_DEFAULT_ROOT = Path.cwd().resolve(strict=True)`
   as a **module-level constant captured at import**, and `allowed_roots()` returns
   `(_DEFAULT_ROOT,)` when no env var is set; an env var that is set but names no
   directory raises `PathNotAllowed`. A stdio server that changes directory after import
   therefore confines paths to the *import-time* cwd. Relevant to #67 today and to any
   future wheel-resident server. `tests/conftest.py`'s session-scoped autouse
   `mcp_allowed_roots` fixture exists for exactly this reason.

---

## 13. Questions for the reviewer

**Still open**

*(Numbers are kept from revision 1 so review notes against it still line up.)*

**Q1 — License of record.** Root `LICENSE` and the workspace say MIT;
`crates/tellegen/README.md:86-89` — shipped as the crates.io README — claims
"Apache-2.0 OR MIT" and references `LICENSE-APACHE`, `LICENSE-MIT` and `NOTICE` that do
not exist in that crate; `crates/tellegen-wasm` ships dual-license files. The wheel's
`license` metadata and PyPI long description depend on the answer.

**Q2 — Wheel version coupling.** Track the engine version (needs the
`validate-version-changes.py` allowlist edit) or version independently?
*Recommend: track the engine.*

**Q7 — sdist and fixture redistribution.** See risk 3. Prune the sdist, or ship wheels
only?

**Q9 — Native release profile.** Add `[profile.wheel]` at `opt-level = 3`, or keep the
wasm-oriented `opt-level = "s"`? Never benchmarked.

**Settled since revision 1**

- **Q3 Python floor — settled empirically.** PowerMCP #69 runs 3.10/3.12/3.13/3.14 green
  against powerio's single cp39-abi3 wheel. The contrast is demonstrable: `surge-py`
  builds pyo3 *without* abi3, declares `requires-python ">=3.12,<3.15"`, and is the sole
  cause of the hand-duplicated `_surge_supported()` gate in `powermcp/doctor.py:49` and
  `powermcp/wizard.py:34`. Use abi3 and tellegen needs no such gate.
- **Q4 output budget — moot.** The wheel ships no MCP server in v1. #67's element-based
  `{truncated, count, head}` is the better shape if one is ever needed.
- **Q5 error style — now a carve-out, not a choice.** See §7.4.
- **Q6 case parsing — both viable; recommendation unchanged.** #67 proves parsing in
  Python works cleanly; binding `powerio::parse` in-extension is still preferred for a
  dependency-free base package.
- **Q8 — the premise was wrong and the question inverts.** `docs/src/studies.md:87-92` is
  not incorrect prose; it is a spec published 2026-09-07 (`df8b3cc`) that #67 implements
  faithfully. The real question — *do we honour or override the published CLI contract?* —
  is answered by §1: honour it, and #67 keeps that surface.
- **Risk 5 of revision 1 (mcp 2.x unexercisable) — settled.** `MCPServer` from
  `mcp.server.mcpserver` is exercised in PowerMCP CI today against `mcp>=2,<3`; only the
  local default interpreter is stuck on 1.29.0.
- **powerio floor — settled by #71** at `powerio[mcp,matrix]>=0.11.3,<0.12`, which also
  deletes PowerMCP's local `diagnostic_record`/`value_type_name` helpers in favour of
  public `powerio.diagnostic_records(...)` and `PioModule.type_name`. `PioModule::type_name`
  already exists in the 0.11.1 Rust crate, so the PyO3 seam can emit the same canonical
  string without the powerio Python package.

---

## 14. The PR stack — a blocking problem, independent of this plan

Verified by ancestry, not by PR description:

```
main ──► (#64's content, at d1a8d91) ──► #67 ──┬──► #69
                                               └──► #70 ──► #71
```

- **#67 is an ancestor of #69, #70 and #71.** All three carry `powermcp/tellegen.py`;
  #64 does not. So the Python 3.13/3.14 matrix (#69), the repo-wide error-shape
  unification (#70) and the powerio 0.11.3 adoption (#71) **cannot be merged without also
  merging an unreviewed draft CLI server.**
- **#64 is an ancestor of nothing,** including #67. `git diff d1a8d91 pr/64` is empty and
  both trees are `7cc43178` — its tip is a merge that discarded its second parent. Its
  content reaches `main` only via #67's lineage.
- **#67 is stale against #64**: `git merge-base --is-ancestor pr/64 pr/67` is false.

Suggested order, highest value first:

1. **Unstack #69 and #70.** Neither has tellegen content of its own; both are held draft
   behind #67 for no technical reason. Rebase each onto `origin/main` and land it
   separately. This is the highest-value, lowest-risk action available.
2. **Resolve #64 as bookkeeping.** Close it noting its content ships via the others, or
   merge it immediately — but do not leave it open.
3. **Land #71's own delta** (the powerio 0.11.3 floor and three helper deletions) rebased
   onto main-after-#70. Its body still says "Draft until powerio 0.11.3 is on PyPI" on a
   non-draft PR whose blocker is resolved.
4. **Merge tellegen #103** with a merge commit as its body instructs, publishing 0.3.0 and
   taking Blocker A off the critical path before the Python work needs it.
5. **Then treat #67** on its merits: fix §7.2's defects, and decide whether it rebases or
   is lifted onto a fresh branch off current main.
6. **Phase 0 starts now, in parallel** — it touches nothing in PowerMCP.

Two additions to the tellegen repo fall out of this and are worth doing regardless of the
wheel:

- **A CLI binary release workflow** (§7.5). Every #67 user needs it and none exists.
- **Scoping the cancellation prose** in `powermcp/TELLEGEN.md` to `study_run` (§7.2 item 1),
  and bounding the argv payload in `solve` (item 2). Small and uncontroversial.

---

## 15. Testing the MCP server from a Claude client — verified recipe

Everything below was executed on 2026-09-18 against PowerMCP #67 (ref `pr67-review`) and a
`cargo build -p tellegen-cli --features conic` build of tellegen at `3130e92`. The
observed transcript is at the end.

### 15.1 Environment

`pip install "powermcp[tellegen]"` installs **no solver** — the extra is literally
`tellegen = []`. Two separate pieces are needed.

```sh
# 1. the CLI. `publish = false` blocks `cargo publish`, NOT a source install, so
#    this works today and lands a release binary on PATH (~/.cargo/bin) — which
#    satisfies PowerMCP's third resolution step and skips the config entirely.
cargo install --git https://github.com/eigenergy/tellegen tellegen-cli --features conic
#    (verified locally in the --path form: 2m05s, 13 MB release binary. A plain
#     `cargo build -p tellegen-cli --features conic` also works but yields an
#     87 MB debug binary at target/debug/tellegen that you must then configure.)

# 2. a Python env; the default interpreter is not enough
python3 -m venv .venv                            # 3.10+ (PowerMCP's floor)
./.venv/bin/python -m pip install 'mcp>=2,<3' 'powerio[mcp,matrix]>=0.11.3,<0.12'
./.venv/bin/python -m pip install -e /path/to/PowerMCP
```

The `mcp` pin matters: `mcp 2.x` moved the server class to
`mcp.server.mcpserver.MCPServer`, which `mcp 1.x` does not have.

### 15.2 Point the server at the binary

Resolution order is env var → `~/.powermcp/config.toml` → `shutil.which("tellegen")`.
Prefer the config file for GUI clients, because **PowerMCP never writes an `env` block**
into a client config and Claude Desktop does not inherit your shell:

```sh
powermcp config set tellegen.binary /ABS/PATH/tellegen/target/debug/tellegen
powermcp doctor tellegen        # actually runs `tellegen capabilities`, 15s timeout
```

`powermcp doctor` is the only check that tests the binary. The launcher's preflight does
**not**: the registry entry uses `probe="mcp"`, so it re-tests the SDK and nothing else.
A server with no binary connects, advertises all 11 tools, and fails per call.

### 15.3 Client configuration

Claude Desktop — `~/Library/Application Support/Claude/claude_desktop_config.json`
(no `type` key). Claude Code — the root `mcpServers` of `~/.claude.json` (adds
`"type": "stdio"`). The managed key is `powermcp_tellegen`:

```json
{
  "mcpServers": {
    "powermcp_tellegen": {
      "command": "/ABS/PATH/.venv/bin/python",
      "args": ["-m", "powermcp", "run", "tellegen"],
      "env": {
        "POWERMCP_TELLEGEN_BINARY": "/ABS/PATH/tellegen/target/debug/tellegen",
        "POWERIO_MCP_ALLOWED_ROOTS": "/ABS/PATH/cases"
      }
    }
  }
}
```

The `env` block is an addition — the installer emits only `command` and `args`, frozen to
the `sys.executable` that ran it. Two consequences worth stating plainly:

- **Re-running `powermcp install` silently destroys hand edits.** It rewrites every
  `powermcp_*` key with an env-less entry, and `_common.py:80-81` writes the
  `*.powermcp.bak` backup **only if one does not already exist** — so a second install
  drops your `env` block with no recoverable copy.
- `powermcp install --tools tellegen` **cannot** configure tellegen alone: the wizard
  unions the selection with `CORE`, so pandapower, pypsa and powerio are written too.

Quit and reopen Claude Desktop fully; for Claude Code, `claude mcp list` and `/mcp`.

**`POWERIO_MCP_ALLOWED_ROOTS` is a security setting, not a convenience.** The
often-repeated summary — "unset means paths are confined to the startup directory" — is
true but misleading about which way it fails. `powerio/mcp/sandbox.py:57` is
`_DEFAULT_ROOT = Path.cwd().resolve(strict=True)`, a module-level constant captured at
import and never recomputed. So with no variable set, the single allowed root is whatever
cwd the launching client happened to hand the process:

- if that cwd is `/` — a common value for a macOS GUI-launched child — then `/` is the
  only root and **every absolute path on the machine is admitted**, including the whole
  home directory, to a *model-supplied* argument. Containment is effectively off.
- if it is any narrower directory, a case file outside it is refused with
  `` `path` is outside allowed MCP roots ``.

Which of the two you get is a property of the client, not of your project, so it is
unpredictable rather than merely restrictive. Set the variable explicitly.

Two further traps in the same gate:

- **Relative arguments are rebased onto the live cwd, not onto the configured root**
  (`sandbox.py:172-175`). A tool argument `"case30.m"` becomes
  `<server cwd>/case30.m` and is refused *even when `POWERIO_MCP_ALLOWED_ROOTS` correctly
  names your project*. Always pass absolute paths from a GUI client.
- Roots are compared as **real** paths. On macOS `/tmp` is a symlink, so a root given as
  `/tmp/cases` will refuse files reached as `/private/tmp/cases`. PowerMCP's own
  `tests/conftest.py` wraps every root in `os.path.realpath` for exactly this reason.

A misconfigured root does not announce itself either: a nonexistent root resolves with
`strict=False`, so a path spelled *under* a typo'd root passes the gate and fails later as
an ordinary file-not-found from the parser. Do not read "case not found" as "the path is
fine" — have the server report `allowed_roots()` instead.

### 15.4 Driving it headlessly instead

Faster to iterate than a GUI, and what CI does. A minimal stdio client is
`StdioServerParameters(command=<venv python>, args=["-m","powermcp","run","tellegen"], env=…)`
under `stdio_client` + `ClientSession`, then `list_tools()` / `call_tool(...)`, reading
`result.content[0].text` as JSON. **Check `result.is_error`** — the SDK returns tool
failures as a result, it does not raise client-side.

For a host with no Rust toolchain, `_command()` runs a `.py` path under `sys.executable`,
so `POWERMCP_TELLEGEN_BINARY=tests/data/fake_tellegen.py` exercises the whole protocol
against a stand-in. It is not a solver; do not read its numbers as results.

### 15.5 Observed transcript

```
connected — 11 tools advertised
  capabilities -> binary: tellegen | available: dcpf,dcopf,acpf,socwr
  contract     -> tellegen.cli/1 tellegen 0.2.0 powerio 0.11.1
  solve(path)  -> optimal objective 626.9487 | LMPs [11.4872, 11.4872, 11.4872]
  solve(+30MW) -> optimal objective 1074.0   | LMPs [18.2, 25.0, 11.4]
  dLMP/dd      -> (objective_unit/MW)/MW shape 3x3 row0 [0.0959, 0.0959, 0.0959]
  solve(socwr) -> optimal objective 631.5334
  study_run(apply) -> is_error=True, text="Error executing tool study_run"
```

`solve(path)` matches a direct `tellegen '{}' < case3.pio.json` run exactly. The +30 MW
edit splits the LMPs, so congestion and the edit path are both real. The last line is the
§7.4 defect: correct refusal, destroyed message.

### 15.6 What this run did *not* establish

- **The `env` block is the one step not executed.** The transcript drove the server with
  the Python SDK's own `StdioServerParameters(env=…)`, which is not Claude Desktop. No
  file in PowerMCP emits an `env` key, and the two installer-written entries in this
  machine's `~/.claude.json` carry only `args`, `command` and `type`, so there is no local
  precedent either way. If a client turns out to ignore `env`, the failure-proof route is
  a two-line wrapper script that exports both variables and `exec`s the interpreter, with
  `command` pointing at the script. `powermcp config set tellegen.binary` already solves
  the binary half independently of `env`, because it is read from
  `~/.powermcp/config.toml` inside the process.
- **The run used a relative path (`case3.m`) and it worked** — because the server was
  launched with its cwd equal to the allowed root. That coincidence is exactly what a GUI
  client removes, so the transcript does not exercise the rebasing trap in §15.3.
- **The fresh venv was load-bearing, not hygiene.** This machine's default interpreter
  resolves `powerio` to a local editable checkout reporting 0.1.1, which has no
  `staged_file_write`; since `powermcp/sandbox.py` imports that name at module scope,
  `powermcp run tellegen` would die of `ImportError` before the server ever connected. The
  `>=0.11.3,<0.12` pin in §15.1 is the fix.
- Nothing here exercised `plan` or the six Study tools against a real bundle, and #67's own
  coverage of `study_export`, `study_import` and `study_inspect`'s pager is thin (§7.2
  item 5).
