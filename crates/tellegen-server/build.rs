//! Draws the RMS dynamics block diagram at every build, from the `@block`
//! annotations in the code (`tellegen-blocks`), for `/docs/blocks/rms`: the
//! engine (`crates/tellegen/src/rms`) and, when the build has it, the app's
//! dynamics code (`packages/svelte`). An annotation problem is a build
//! warning naming the file and line.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    // Canonical, so file paths show relative to the workspace root.
    let workspace = manifest
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../.."));
    let (roots, headings) = tellegen_blocks::rms_preset(&workspace);
    // Directories: cargo reruns when any file under them changes.
    for root in &roots {
        println!("cargo:rerun-if-changed={}", root.dir.display());
    }
    let meta = tellegen_blocks::Meta {
        intro: tellegen_blocks::RMS_INTRO.to_owned(),
        ..tellegen_blocks::Meta::default()
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("rms-blocks.html");
    let doc = tellegen_blocks::generate_roots(
        &roots,
        &headings,
        &workspace,
        "tellegen RMS dynamics",
        &meta,
        &out,
        None,
    )
    .expect("draw the RMS block diagram");
    for issue in &doc.issues {
        println!("cargo:warning=block docs: {issue}");
    }
}
