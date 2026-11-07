# Settings

Open **Settings** in the header or panel toolbar. The Network and Studies power-flow settings buttons open this same panel.

Calculation settings belong to the selected case. Edit the draft, choose **Apply settings**, then run explicitly. Applying, resetting, or selecting another calculation does not solve or replace the displayed result. Until a new run succeeds, the result retains its preceding calculation and settings. A failed or cancelled rerun retains that result. Load edits and sensitivity calculations use the accepted Study/session settings, independently of pending choices.

The panel groups load-model and constraint behavior separately from numerical controls. Advanced multiconductor tolerances are collapsed. The load envelope changes equations, and angle/thermal selections change the DC OPF optimization model. These are distinct from engineering limit assessments. Source-less-island exclusion remains an explicit **Solve energized network** action, with exclusions visible before execution. Once an energized solve succeeds, or an input carrying the selection is opened, that case solves only its energized network; re-import it to solve the full network. Clearing a numeric field restores its case default.

## Scope and defaults

| Scope | Storage | Examples |
| --- | --- | --- |
| Calculation | Case and saved result | Multiconductor PF options, AC PF initial voltages, DC OPF limit enforcement |
| Display preferences | This browser | Theme, bus sizing, preferred coloring |
| View state | Existing view controls | Camera, selection, map/diagram mode and panel positions |

Hosted-case manifests accept all ten multiconductor options and validate them through the common Rust resolver.

Calculation settings resolve in this order: engine defaults, hosted-case or restored-study baseline, explicit overrides. **Restore case defaults** removes the overrides and restores that baseline, including hosted-case overrides. It does not reset every case. Display defaults are restored separately. If browser preference storage is unavailable, changes remain usable for the current visit and a message identifies the persistence failure.

All ten multiconductor PF options are described in the [power-flow guide](multiconductor-pf.md). Balanced AC PF supports `start: "flat" | "case"`. DC OPF supports `angle_difference`, `thermal` (`"all"`, `"rated"`, or `{ "branches": [ELEMENT_KEYS] }`) and optional `lazy` (`null` or `{ "max_rounds": 20, "near_binding": 0.98 }`). Other calculations describe their availability and any public options; this interface adds no numerical parameters to those solvers. The isolated browser AC OPF backend remains separately available according to the host configuration.

The Rust resolver owns calculation defaults and validation. Generated TypeScript descriptors expose labels, units, types, defaults and constraints. The running engine's catalog reports its build capabilities. Unknown or unsupported settings and invalid combinations are rejected before selected state changes. A partial nested option object is replaced as a unit and its omitted fields receive that option's typed defaults.

## TypeScript

```ts
import {
  describeSettings,
  resolveSettings,
  calculateWithSettings,
  createSettingsHttpClient
} from '@tellegen/engine';

const catalog = await describeSettings();
const settings = { calculation: 'mc_ac_pf', overrides: { damping: 0.8 } };
const resolved = await resolveSettings(settings);
const result = await calculateWithSettings({ module_json, settings }, signal);

// Same request and result contracts, through a server with compute enabled.
const http = createSettingsHttpClient('/api');
const remoteResult = await http.calculate({ module_json, settings }, signal);
```

A resolved response carries `version`, `calculation`, `engine_defaults`, the full `baseline`, the supplied `overrides`, and full `effective` settings. The calculation response pairs these settings with its existing solver result. Multiconductor results retain load-model qualification and excluded-island provenance.

For an interactive Svelte host, `controller.inspectSettings(case)` reads next-run and accepted-result settings. `controller.updateSettings(case, calculation, patch, expectedSettingsRevision)` validates and applies an atomic patch without solving; pass `true` as its final argument to restore the case baseline. `controller.runSettings(case, signal)` explicitly calculates. Mutations reject changes during a solve and recheck revisions after asynchronous validation. Display preferences are available through `app.preferences.inspect()`, `update(patch)` and `reset()`.

`createStudy(input, formulation, { settings })` and Rust `Study::new_with_settings` retain accepted options for later edits and sensitivities. PowerIO saves of such a Study carry a `tellegen.calculation_settings` extension: `{ "version": 1, "calculation": "dcopf", "overrides": { … } }`, where `overrides` holds only the options whose values differ from the engine defaults. A later engine therefore applies its own defaults to everything the user never changed. A saved DC OPF instance states the actual active angle/thermal constraints; lazy enforcement does not remove its original enforceable limits.

Reopening restores the recorded options over the current engine defaults. A recorded option this engine does not know, such as one written by a newer version, is dropped with a warning rather than refusing the file: Rust `Study::new` reports it as a `settings_ignored` diagnostic on every solve, and the Svelte app logs it when the file opens. `Study::new` also ignores, with the same diagnostic, a record of another format version and settings recorded for a different calculation than the one it opens. Known options keep their full validation. Legacy inputs without this extension retain their existing defaults and do not acquire invented historical provenance. Existing versioned multiconductor snapshots keep their own effective options and replay checks.

## Native and CLI

```rust,ignore
use tellegen::settings::{Calculation, CalculationRequest, SettingsRequest};

let settings = SettingsRequest::new(Calculation::McAcPf).with_overrides(overrides);
let resolved = tellegen::settings::resolve(&settings)?;
let request = CalculationRequest::new(module_json, settings).with_energized_only(true);
let result = tellegen::settings::calculate(&request, cancel_flag)?;
let catalog = tellegen::settings::describe();
```

`Calculation` names a calculation; its JSON form is the wire name (`"mc_ac_pf"`, `"dcopf"`, `"acpf"`, `"dcpf"`, `"socwr"`, `"acopf"`), which `as_str`, `Display` and `FromStr` also use. The request and result types are `#[non_exhaustive]`, so build requests with `new` and the `with_*` methods.

```sh
tellegen settings describe
printf '%s' '{"calculation":"mc_ac_pf","overrides":{"damping":0.8}}' | tellegen settings resolve
tellegen calculate < calculation-request.json
tellegen solve-mc '@pf-options.json' < case.pio.json
```

`tellegen solve-mc` resolves its options through the same resolver, so an unknown option key is an error rather than being ignored.

`tellegen describe` includes the settings catalog and calculation request schema. Existing solve commands remain available. A calculation request contains a `module_json` string, `settings`, optional balanced-network `edits`, and optional `energized_only` (multiconductor PF only). Multiconductor load edits belong in the input module.

## HTTP and WebMCP

- `GET /api/settings` describes settings supported by this server.
- `POST /api/settings/resolve` accepts a settings request and returns resolved settings without running a solver.
- `POST /api/calculate` accepts a calculation request. It uses the existing compute gate, rate limit, concurrency permits and timeout. The request body is limited to 64 MiB; a larger declared body is refused with 413 before it waits for a solver permit, and the body is read only once a permit is held, within the solve timeout. Requests never modify shared hosted-case state. Legacy solve-stream and sensitivity fallbacks that cannot carry custom settings are refused rather than silently using defaults.

WebMCP exposes `inspect_settings`, `update_settings`, `reset_settings`, and `run_calculation`. Inspection defaults to calculation settings; use `scope: "display"` for browser preferences or `field` to inspect one calculation-setting descriptor. Calculation mutations require `case_id`, `expected_revision`, and `settings_revision`; display mutations require the preference revision. `update_settings` accepts `patch` and optional `calculation`. Updates and resets do not solve. `run_calculation` explicitly uses the selected settings and requires a fresh case/settings binding. Saved Study views remain read-only. The existing `solve_multiconductor_pf` tool and its `max_iterations` argument remain available.

Backend URLs, asset locations and application branding remain embedding configuration rather than user preferences.
