//! Draws the block diagrams at every build, from the `@block` annotations in
//! the code (`tellegen-blocks`): `/docs/blocks/rms`, the RMS engine
//! (`crates/tellegen/src/rms`) and, when the build has it, the app's dynamics
//! code (`packages/svelte`); `/docs/blocks/protection`, the relay engine
//! (`crates/tellegen/src/protection`). An annotation problem is a build
//! warning naming the file and line.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    // Canonical, so file paths show relative to the workspace root.
    let workspace = manifest
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../.."));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    for name in ["rms", "protection"] {
        draw(&workspace, &out_dir, name);
    }
}

/// Write `<name>-blocks.html` of preset `name` to `out_dir`.
fn draw(workspace: &Path, out_dir: &Path, name: &str) {
    let (roots, headings, intro, title) =
        tellegen_blocks::preset(name, workspace).expect("a known preset");
    // Directories: cargo reruns when any file under them changes.
    for root in &roots {
        println!("cargo:rerun-if-changed={}", root.dir.display());
    }
    let meta = tellegen_blocks::Meta {
        intro: intro.to_owned(),
        ..tellegen_blocks::Meta::default()
    };
    let out = out_dir.join(format!("{name}-blocks.html"));
    let doc =
        tellegen_blocks::generate_roots(&roots, &headings, workspace, title, &meta, &out, None)
            .unwrap_or_else(|e| panic!("draw the {name} block diagram: {e}"));
    for issue in &doc.issues {
        println!("cargo:warning=block docs ({name}): {issue}");
    }
}
