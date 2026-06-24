# Desktop and mobile (roadmap)

This chapter records the design for native desktop and mobile builds of tellegen, built
on Tauri. It is a roadmap; the native apps are not yet implemented. The choices are fixed
here so that adding them later does not require restructuring the workspace.

## Why native apps

The web app already runs the full engine in the browser, privately, with no server. A
native build is not about a capability the browser lacks in principle — it is about reach,
distribution, and the faster solver:

- **Cross-platform, including mobile.** Tauri 2 targets desktop (macOS, Windows, Linux)
  and mobile (iOS, Android) from the same Rust and web codebase. Phones and tablets get a
  real installable app, not a browser tab.
- **The faster solver.** The native build runs the engine with the multithreaded `pounce`
  AC OPF backend — the one the browser cannot use, because WebAssembly has no threads (see
  [Architecture](architecture.md)). So the desktop and mobile apps solve the full nonlinear
  AC OPF faster than the browser does.
- **Offline and local.** No network at all: cases stay on the device and solve in process —
  the browser's local-file privacy property, extended to every solve.
- **OS integration and distribution.** File associations, native menus, notifications, and
  installer / app-store distribution.
- **A home for local case libraries**, and later the opt-in research data collection.

## Shape

[Tauri](https://tauri.app) pairs a Rust backend with the platform webview. tellegen already
has both halves — the SvelteKit app (built as a static single-page app) is the UI, and the
engine is a Rust crate — so the native build wires them together without forking either.

- `apps/desktop` — the Tauri shell (desktop and mobile). It loads the same `apps/web`
  build; the UI is shared, not reimplemented.
- `crates/tellegen-tauri` — a workspace member that holds a native `Study` and exposes it
  to the UI through Tauri commands. Built natively, it uses the `pounce` backend (EPL-2.0,
  multithreaded) for the full AC OPF.

The existing `apps/` + `packages/` + `crates/` layout already accommodates this; no move is
required when the native targets land.

## One UI, three transports

The web app's solve path is a transport: the in-browser `Study`, or the HTTP server for
opt-in and shared workflows. The native build adds a third — the Tauri command bridge —
selected at runtime by detecting the Tauri context. The UI and the `Study` contract are
identical across all three; only the transport and the backend (interiors in the browser,
pounce natively) differ. Keeping the transport behind one interface is what makes the
native target additive rather than a fork.

## Status

Not started. This chapter fixes the layout (`apps/desktop`, `crates/tellegen-tauri`), the
shared-UI decision, the transport interface, the native `pounce` backend, and the licensing
boundary, so the desktop and mobile targets can be added later as an additive step.
