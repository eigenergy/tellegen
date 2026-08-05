//! Error wording for `.pio.json` package reads.
//!
//! Every surface that accepts a dropped package — the study restore, the study
//! export, and the multiconductor viewer — calls
//! `powerio_pkg::NetworkPackage::from_json` and wraps its failure for the user.
//! This module is the one place that wording lives, so the three call sites
//! cannot drift.
//!
//! The distinction that matters to a user is *why* a package will not load. A
//! malformed or truncated file is a bad file. A file whose `schema_version` is
//! outside the reader's lineage is a **good file written by a different version
//! of powerio**, and the fix is not to repair it but to regenerate it: the
//! `.pio.json` document is a snapshot of a case plus an edit log, so re-opening
//! the source case and saving the study again produces a current one. powerio's
//! own message says to regenerate from the source case; this restates it in
//! terms of the thing a tellegen user actually did (saved a study).
//!
//! Deliberately free of a `powerio_pkg` dependency so it compiles in every
//! feature combination — `powerio-pkg` is optional in this crate and gated on
//! `sensitivity`, while the multiconductor viewer needs this wording without it.

use std::fmt::Display;

/// The substring powerio uses when it rejects a document whose `schema_version`
/// is outside the lineage this reader supports. Stable across the versions that
/// changed the rest of the sentence: 0.7.x says "this reader supports major
/// version N", later readers name a `major.minor` lineage and add their own
/// regenerate hint. Matching the shared prefix keeps this working across a
/// powerio bump.
const UNSUPPORTED_SCHEMA_VERSION: &str = "unsupported .pio.json schema_version";

/// Whether `message` is powerio reporting a `.pio.json` written to a format
/// version it does not read.
#[must_use]
pub fn is_unsupported_schema_version(message: &str) -> bool {
    message.contains(UNSUPPORTED_SCHEMA_VERSION)
}

/// Wrap a `NetworkPackage::from_json` failure for display.
///
/// A version mismatch gets the regenerate instruction; everything else keeps
/// the generic wording, which already carries powerio's parse diagnostic.
pub fn read_error(err: &impl Display) -> String {
    let message = err.to_string();
    if is_unsupported_schema_version(&message) {
        return format!(
            "this .pio.json was written by a different version of the package format and \
             cannot be read: {message}. Open the source case again and re-save the study to \
             write a current package."
        );
    }
    format!("invalid .pio.json package: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 0.7.x wording. Pinned so a powerio bump that changes the tail of the
    /// sentence still routes to the regenerate message.
    const V07_MESSAGE: &str =
        "unsupported .pio.json schema_version 9.0.0; this reader supports major version 0";

    /// The wording that replaces it once the envelope collapses to one version
    /// number and the reader names a major.minor lineage.
    const LINEAGE_MESSAGE: &str = "unsupported .pio.json schema_version 0.1.1; this reader \
         supports 0.2.x; regenerate the package from its source case";

    #[test]
    fn a_version_mismatch_tells_the_user_to_re_save() {
        for message in [V07_MESSAGE, LINEAGE_MESSAGE] {
            assert!(
                is_unsupported_schema_version(message),
                "not matched: {message}"
            );
            let wrapped = read_error(&message);
            assert!(
                wrapped.contains("re-save the study"),
                "expected the regenerate instruction, got: {wrapped}"
            );
            // The upstream diagnostic rides along: it names the version that was
            // read and the one this build wants.
            assert!(
                wrapped.contains(message),
                "upstream detail dropped: {wrapped}"
            );
        }
    }

    #[test]
    fn other_failures_keep_the_generic_wording() {
        let wrapped = read_error(&"expected value at line 1 column 1");
        assert!(
            wrapped.starts_with("invalid .pio.json package: "),
            "got: {wrapped}"
        );
        assert!(!wrapped.contains("re-save the study"), "got: {wrapped}");
    }

    #[test]
    fn a_model_kind_mismatch_is_not_a_version_mismatch() {
        assert!(!is_unsupported_schema_version(
            "model_kind does not match model.kind"
        ));
    }
}
