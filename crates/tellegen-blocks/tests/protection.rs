//! The protection block annotations are complete: every module of the relay
//! engine is a block, every block is documented, every tag is known. A
//! failure here names the file and line to fix.

use std::path::Path;

#[test]
fn protection_blocks_are_complete() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (roots, headings) = tellegen_blocks::protection_preset(&workspace);
    if roots.is_empty() {
        return; // a checkout without the relay engine
    }
    let doc = tellegen_blocks::scan_roots(&roots, &headings, &workspace, "protection")
        .expect("scan the relay engine");
    let issues: Vec<String> = doc.issues.iter().map(ToString::to_string).collect();
    assert!(
        issues.is_empty(),
        "block annotation issues:\n{}",
        issues.join("\n")
    );
    assert!(doc.files.len() >= 6, "found {} files", doc.files.len());
    let roots: Vec<&str> = doc
        .roots()
        .into_iter()
        .map(|r| doc.blocks[r].id.as_str())
        .collect();
    assert_eq!(roots, ["protection"]);
    // The diagram has something to draw.
    assert!(!doc.edges.is_empty());
}
