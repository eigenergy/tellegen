//! The RMS dynamics block annotations are complete: every module (Rust file,
//! TypeScript file, Svelte component) is a block, every block is
//! documented, every tag is known. A failure here names the file and line
//! to fix.

use std::path::Path;

#[test]
fn rms_blocks_are_complete() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (roots, headings) = tellegen_blocks::rms_preset(&workspace);
    let doc = tellegen_blocks::scan_roots(&roots, &headings, &workspace, "RMS dynamics")
        .expect("scan the RMS engine and the app's dynamics code");
    let issues: Vec<String> = doc.issues.iter().map(ToString::to_string).collect();
    assert!(
        issues.is_empty(),
        "block annotation issues:\n{}",
        issues.join("\n")
    );
    assert!(doc.files.len() >= 15, "found {} files", doc.files.len());
    // The engine, and the app when its sources are there, at the top.
    let roots: Vec<&str> = doc
        .roots()
        .into_iter()
        .map(|r| doc.blocks[r].id.as_str())
        .collect();
    assert!(roots.contains(&"rms"), "{roots:?}");
    assert!(
        roots.iter().all(|r| ["rms", "app"].contains(r)),
        "{roots:?}"
    );
    // The diagram has something to draw.
    assert!(!doc.edges.is_empty());
}
