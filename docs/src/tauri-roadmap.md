# Desktop and mobile (roadmap)

Not built yet. This records the plan so the native targets land without restructuring.

tellegen runs in the browser today. A native build adds three things the browser cannot:

- **Cross-platform reach.** Tauri 2 builds desktop (macOS, Windows, Linux) and mobile (iOS, Android) from one Rust and web codebase. Phones and tablets get an installable app, not a tab.
- **The full nonlinear AC OPF.** The browser solves DC OPF, AC power flow, and the SOCWR relaxation. The interior-point AC OPF parallelizes across threads, which the browser does not have, so it runs natively — locally, with no server.
- **Offline and local.** Cases stay on the device and solve in process.

The pieces already fit. The SvelteKit app is a static single-page app and the engine is a Rust crate, so the native build reuses both:

- `apps/desktop` — the Tauri shell. It loads the same `apps/web` build.
- `crates/tellegen-tauri` — a workspace member that holds a native `Study` and exposes it through `#[tauri::command]`. Built natively, it adds the interior-point AC OPF backend.

One UI, three transports: the in-browser `Study`, the HTTP server, and the Tauri bridge, chosen at runtime. The `Study` contract is the same across all three; only the transport differs.
