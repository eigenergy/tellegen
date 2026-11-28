//! Block diagrams that the code draws of itself.
//!
//! The code is divided into blocks where it is written, by one tag in the
//! documentation it already has:
//!
//! ```text
//! //! Time integration of `M ẋ = f(x)` with the implicit trapezoidal rule, …   (module docs)
//! //!
//! //! @block Time integration
//!
//! /// Take one step from `x` …                                                  (item docs)
//! // @block One step
//! pub fn step(…) {
//!     // @block Predict                                                          (a section)
//!     // Start Newton from the extrapolated history …
//!     …
//!     // @block Newton iterations
//!     // …
//! }
//! ```
//!
//! [`scan`] reads a module tree into blocks (module → item → section, nested
//! by where they are), takes each block's id from its Rust path and its edges
//! from the code (a name one block uses and another defines), and
//! [`to_html`] draws them as one self-contained, interactive page: the blocks
//! of one level with what uses what, a summary on hover, the sub-blocks on a
//! click, the full documentation and the code beside it.
//!
//! Nothing is written twice: the text is the rustdoc the code carries anyway,
//! the structure is where the code is, and the arrows are what the code
//! calls. A build script or a test turns annotation problems (a module
//! without a block, an undocumented block, an unknown tag) into warnings or
//! failures, so the diagram cannot drift from the code.
//!
//! @block Block docs generator

pub mod lex;
pub mod render;
pub mod scan;

use std::path::Path;

pub use lex::Lang;
pub use render::{to_html, to_json, Meta};
pub use scan::{build, scan, scan_roots, Block, Doc, Edge, Heading, Issue, Kind, Root};

/// Scan `dir` as module `module` and write the page to `html` (and the data
/// to `json`, if given).
///
/// # Errors
/// When a file cannot be read or written.
pub fn generate(
    dir: &Path,
    module: &str,
    base: &Path,
    title: &str,
    meta: &Meta,
    html: &Path,
    json: Option<&Path>,
) -> std::io::Result<Doc> {
    let doc = scan(dir, module, base, title)?;
    if let Some(parent) = html.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(html, to_html(&doc, meta))?;
    if let Some(json) = json {
        std::fs::write(json, to_json(&doc, meta))?;
    }
    Ok(doc)
}

/// The roots and headings of this repository's RMS dynamics diagram: the
/// engine (`crates/tellegen/src/rms`), its browser entry points (the RMS and
/// protection files of `crates/tellegen-wasm/src` and `packages/engine/src`)
/// and the app's dynamics code (`packages/svelte/src/lib/dynamics`,
/// `…/components/dynamics`), each included when it is there.
#[must_use]
pub fn rms_preset(workspace: &Path) -> (Vec<Root>, Vec<Heading>) {
    let candidates: [(&str, &str, &[&str]); 6] = [
        ("crates/tellegen/src/rms", "rms", &[]),
        (
            "crates/tellegen-wasm/src",
            "browser::wasm",
            &["rms.rs", "rms/threads.rs", "rms_tools.rs", "protection.rs"],
        ),
        (
            "packages/engine/src",
            "browser::engine",
            &[
                "rms.ts",
                "rms-protocol.ts",
                "rms-worker.ts",
                "rms-thread.ts",
                "rms-tools.ts",
                "rms-tools-worker.ts",
                "protection.ts",
            ],
        ),
        ("packages/svelte/src/lib/dynamics", "app::logic", &[]),
        (
            "packages/svelte/src/lib/components/dynamics",
            "app::ui",
            &[],
        ),
        ("packages/svelte/src/lib/charts", "app::charts", &[]),
    ];
    let roots = candidates
        .iter()
        .map(|(dir, module, only)| Root {
            dir: workspace.join(dir),
            module: (*module).to_owned(),
            only: only.iter().map(|f| (*f).to_owned()).collect(),
        })
        .filter(|r| r.dir.is_dir())
        .collect();
    let heading = |module: &str, title: &str, doc: &str| Heading {
        module: module.to_owned(),
        title: title.to_owned(),
        doc: doc.to_owned(),
    };
    let headings = vec![
        heading(
            "browser",
            "RMS in the browser",
            "The browser entry points of the RMS engine: the WebAssembly adapter that exports it \
             (`crates/tellegen-wasm`) and the `@tellegen/engine` workers and classes that call it \
             (`packages/engine`).",
        ),
        heading(
            "browser::wasm",
            "WebAssembly adapter",
            "`crates/tellegen-wasm/src`: the RMS case, the live session, the model tools, the \
             protection calls and the threads of the threaded build, exported to JavaScript.",
        ),
        heading(
            "browser::engine",
            "Engine package",
            "`packages/engine/src`: the RMS worker and its protocol, the threads of the threaded \
             build, the model tools and the protection calls, as the app uses them.",
        ),
        heading(
            "app",
            "Dynamics in the app",
            "The browser side of RMS dynamics (`packages/svelte`): the state and logic that drive \
             the engine in its web worker (attaching dynamic data, runs, the live view, playback, \
             the unit editor) and the components that show them (the Dynamics panel, the live \
             workbench, charts, the block-diagram editor).",
        ),
        heading(
            "app::logic",
            "State and logic",
            "`packages/svelte/src/lib/dynamics`: controllers holding the dynamics state, and pure \
             helpers (events, series, axes, verdicts, pacing) tested without a browser.",
        ),
        heading(
            "app::ui",
            "Components",
            "`packages/svelte/src/lib/components/dynamics`: the Svelte components of the Dynamics \
             panel, the live workbench and the editors.",
        ),
        heading(
            "app::charts",
            "Chart helpers",
            "`packages/svelte/src/lib/charts`: axis ticks and decimation shared by the live \
             traces, the Bode plots and the app's other charts.",
        ),
    ];
    (roots, headings)
}

/// The intro of the RMS dynamics diagram's top level.
pub const RMS_INTRO: &str = "RMS (phasor-domain) dynamic simulation in tellegen, from the \
    engine to the app. **RMS dynamic simulation** is the Rust engine: building the system from a \
    case, the device models, the solver, runs and their analysis. **RMS in the browser** is the \
    WebAssembly adapter and the engine package's workers that call it. **Dynamics in the app** is \
    the browser side that drives it and shows the results. Every block is drawn from `@block` tags in \
    the code and its own documentation; arrows are what the code uses.";

/// The roots of this repository's protection diagram: the relay engine
/// (`crates/tellegen/src/protection`), when it is there. Its link to RMS
/// runs (`rms::protect`) and the app's protection views are blocks of the
/// RMS dynamics diagram.
#[must_use]
pub fn protection_preset(workspace: &Path) -> (Vec<Root>, Vec<Heading>) {
    let root = Root {
        dir: workspace.join("crates/tellegen/src/protection"),
        module: "protection".to_owned(),
        only: Vec::new(),
    };
    (
        std::iter::once(root).filter(|r| r.dir.is_dir()).collect(),
        Vec::new(),
    )
}

/// The intro of the protection diagram's top level.
pub const PROTECTION_INTRO: &str = "Protection relays in tellegen: the measuring-relay functions, \
    their settings derived from the network with an audit, the standard test cases run in RMS, \
    static fault studies, and the interface the app calls. The relays act in RMS runs and live \
    sessions through `rms::protect` (see the RMS dynamics diagram). Every block is drawn from \
    `@block` tags in the code and its own documentation; arrows are what the code uses.";

/// A named diagram of this repository: its roots and headings, intro and
/// title (`rms` or `protection`).
#[must_use]
pub fn preset(
    name: &str,
    workspace: &Path,
) -> Option<(Vec<Root>, Vec<Heading>, &'static str, &'static str)> {
    match name {
        "rms" => {
            let (roots, headings) = rms_preset(workspace);
            Some((roots, headings, RMS_INTRO, "tellegen RMS dynamics"))
        }
        "protection" => {
            let (roots, headings) = protection_preset(workspace);
            Some((roots, headings, PROTECTION_INTRO, "tellegen protection"))
        }
        _ => None,
    }
}

/// Scan the roots and write the page to `html` (and the data to `json`).
///
/// # Errors
/// When a file cannot be read or written.
pub fn generate_roots(
    roots: &[Root],
    headings: &[Heading],
    base: &Path,
    title: &str,
    meta: &Meta,
    html: &Path,
    json: Option<&Path>,
) -> std::io::Result<Doc> {
    let doc = scan_roots(roots, headings, base, title)?;
    if let Some(parent) = html.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(html, to_html(&doc, meta))?;
    if let Some(json) = json {
        std::fs::write(json, to_json(&doc, meta))?;
    }
    Ok(doc)
}
